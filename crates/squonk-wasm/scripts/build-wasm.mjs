#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { spawn } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { gzipSync } from "node:zlib";
import { availableParallelism } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { packages } from "./package-matrix.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const crateDir = dirname(scriptDir);
const workspaceRoot = join(crateDir, "..", "..");
const metric = process.env.SQUONK_WASM_SIZE_METRIC === "raw" ? "raw" : "gzip";
// Size reports live outside the per-variant `pkg-*` dirs so the published tarball
// carries only runtime artifacts (glue + `.wasm` + `.d.ts`); `check-size-budget.mjs`
// reads them from here.
const reportsDir = join(crateDir, "size-reports");
mkdirSync(reportsDir, { recursive: true });
const allVariants = packages.map((variant) => ({
  ...variant,
  description: `${variant.packageName} full document API`,
  pkgDir: join(crateDir, `pkg-${variant.label}`),
}));
const variants = selectVariants();
const jobs = parallelism();

await runPool(variants, jobs, buildVariant);

function selectVariants() {
  const requested = process.env.SQUONK_WASM_VARIANTS
    ?.split(",")
    .map((value) => value.trim())
    .filter(Boolean);
  if (!requested || requested.length === 0) {
    return allVariants;
  }

  const byLabel = new Map(allVariants.map((variant) => [variant.label, variant]));
  return requested.map((label) => {
    const variant = byLabel.get(label);
    if (!variant) {
      throw new Error(
        `unknown wasm variant ${JSON.stringify(label)}; valid variants are ${allVariants
          .map((item) => item.label)
          .join(", ")}`,
      );
    }
    return variant;
  });
}

async function buildVariant({ label, description, pkgDir, features }) {
  const targetDir = join(workspaceRoot, "target", "wasm-variants", label);
  const wasmInput = join(
    targetDir,
    "wasm32-unknown-unknown",
    "release-wasm",
    "squonk_wasm.wasm",
  );
  const wasmOutput = join(pkgDir, "squonk_wasm_bg.wasm");
  const wasmOptimized = join(pkgDir, "squonk_wasm_bg.opt.wasm");
  const wasmStripped = join(pkgDir, "squonk_wasm_bg.strip.wasm");
  const wasmBaseline = join(pkgDir, "squonk_wasm_bg.base.wasm");

  rmSync(pkgDir, { force: true, recursive: true });
  mkdirSync(pkgDir, { recursive: true });

  const cargoArgs = [
    "build",
    "--profile",
    "release-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "-p",
    "squonk-wasm",
    "--target-dir",
    targetDir,
    "--no-default-features",
  ];
  if (features.length > 0) {
    cargoArgs.push("--features", features.join(","));
  }
  await run("cargo", cargoArgs, workspaceRoot);

  await run("wasm-bindgen", [
    "--target",
    "web",
    "--remove-name-section",
    "--remove-producers-section",
    "--out-dir",
    pkgDir,
    wasmInput,
  ], workspaceRoot);

  // Deno can import WebAssembly modules directly without filesystem permission.
  // The bundler target wires those module exports into wasm-bindgen's JS glue.
  const denoDir = join(pkgDir, "deno");
  mkdirSync(denoDir, { recursive: true });
  await run("wasm-bindgen", [
    "--target",
    "bundler",
    "--remove-name-section",
    "--remove-producers-section",
    "--out-dir",
    denoDir,
    wasmInput,
  ], workspaceRoot);
  const denoWasm = join(denoDir, "squonk_wasm_bg.wasm");
  const denoOptimized = join(denoDir, "squonk_wasm_bg.opt.wasm");
  await run("wasm-opt", [
    "-all",
    "-Oz",
    "--strip-debug",
    "--strip-producers",
    denoWasm,
    "-o",
    denoOptimized,
  ], crateDir);
  copyFileSync(denoOptimized, denoWasm);
  rmSync(denoOptimized, { force: true });

  copyFileSync(wasmOutput, wasmBaseline);
  await run("wasm-opt", [
    "-all",
    "--strip-debug",
    "--strip-dwarf",
    "--strip-producers",
    wasmOutput,
    "-o",
    wasmStripped,
  ], crateDir);
  await run("wasm-opt", [
    "-all",
    "-Oz",
    "--strip-debug",
    "--strip-producers",
    wasmOutput,
    "-o",
    wasmOptimized,
  ], crateDir);

  const candidates = [
    { label: "wasm-bindgen stripped", size: sizeOf(wasmBaseline) },
    { label: "wasm-opt strip-only", size: sizeOf(wasmStripped) },
    { label: "wasm-opt -all -Oz", size: sizeOf(wasmOptimized) },
  ];
  const selected = candidates.reduce((best, candidate) =>
    candidate.size[metric] < best.size[metric] ? candidate : best,
  );

  copyFileSync(selected.size.path, wasmOutput);
  rmSync(wasmBaseline, { force: true });
  rmSync(wasmStripped, { force: true });
  rmSync(wasmOptimized, { force: true });

  const report = {
    variant: label,
    description,
    features,
    metric,
    selected: selected.label,
    artifacts: Object.fromEntries(
      candidates.map((candidate) => [candidate.label, printable(candidate.size)]),
    ),
    final: printable(sizeOf(wasmOutput)),
    deno: printable(sizeOf(denoWasm)),
  };
  writeFileSync(join(reportsDir, `${label}.json`), JSON.stringify(report, null, 2) + "\n");

  console.log(
    `${label}: ${description}; selected ${selected.label}: ${formatBytes(report.final.raw)} raw, ` +
      `${formatBytes(report.final.gzip)} gzip (${metric} metric)`,
  );
}

function parallelism() {
  const requested = process.env.SQUONK_WASM_JOBS;
  const jobs = requested === undefined
    ? Math.min(4, availableParallelism())
    : Number.parseInt(requested, 10);
  if (!Number.isSafeInteger(jobs) || jobs < 1) {
    throw new Error(`SQUONK_WASM_JOBS must be a positive integer, got ${JSON.stringify(requested)}`);
  }
  return jobs;
}

async function runPool(items, concurrency, task) {
  let next = 0;
  async function worker() {
    while (next < items.length) {
      const item = items[next];
      next += 1;
      await task(item);
    }
  }
  await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, worker));
}

function run(command, args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, stdio: "inherit" });
    child.on("error", reject);
    child.on("exit", (code, signal) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(
          `${command} ${args.join(" ")} failed with ${signal ? `signal ${signal}` : `exit code ${code}`}`,
        ));
      }
    });
  });
}

function sizeOf(path) {
  if (!existsSync(path)) {
    throw new Error(`missing artifact: ${path}`);
  }
  const bytes = readFileSync(path);
  return {
    path,
    raw: statSync(path).size,
    gzip: gzipSync(bytes, { level: 9 }).byteLength,
  };
}

function printable(size) {
  return {
    raw: size.raw,
    gzip: size.gzip,
  };
}

function formatBytes(bytes) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`;
}
