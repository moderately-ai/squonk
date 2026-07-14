#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { currentNativePackage } from "./native-package-matrix.mjs";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const requestedRuntime = process.argv[2];
if (requestedRuntime && !["bun", "deno"].includes(requestedRuntime)) {
  throw new Error(`unknown runtime ${JSON.stringify(requestedRuntime)}`);
}
const packageDir = join(crateDir, "dist", "npm", "sqlite");
const facadeTarball = pack(packageDir);
const native = currentNativePackage();
const nativeDir = native === null ? null : join(
  crateDir,
  "dist",
  "npm",
  native.packageName.slice("@squonk-sql/".length),
);
const nativeTarball = nativeDir !== null && existsSync(nativeDir) ? pack(nativeDir) : null;
const workDir = mkdtempSync(join(tmpdir(), "squonk-runtimes-"));

try {
  run("npm", ["init", "-y"], workDir);
  run("npm", ["install", "--no-save", ...(nativeTarball ? [nativeTarball] : []), facadeTarball], workDir);
  writeFileSync(join(workDir, "smoke.mjs"), `
import { strict as assert } from "node:assert";
import { parse, runtimeInfo } from "@squonk-sql/sqlite";
assert.equal(parse("select 1").toSQL(), "SELECT 1");
const expected = globalThis.Bun ? ${JSON.stringify(nativeTarball ? "native" : "wasm")} : "wasm";
assert.equal(runtimeInfo().backend, expected);
console.log(JSON.stringify(runtimeInfo()));
`);

  if (!requestedRuntime || requestedRuntime === "bun") {
    if (available("bun")) run("bun", ["run", "smoke.mjs"], workDir);
    else if (requestedRuntime) throw new Error("Bun is not installed");
    else console.log("skip Bun (not installed)");
  }
  if (!requestedRuntime || requestedRuntime === "deno") {
    if (available("deno")) run("deno", ["run", "--node-modules-dir=manual", "smoke.mjs"], workDir);
    else if (requestedRuntime) throw new Error("Deno is not installed");
    else console.log("skip Deno (not installed)");
  }
} finally {
  rmSync(workDir, { force: true, recursive: true });
  rmSync(facadeTarball, { force: true });
  if (nativeTarball) rmSync(nativeTarball, { force: true });
}

function pack(cwd) {
  const result = capture("npm", ["pack", "--json"], cwd);
  return join(cwd, JSON.parse(result)[0].filename);
}

function available(command) {
  return spawnSync(command, ["--version"], { stdio: "ignore" }).status === 0;
}

function capture(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr || `${command} failed`);
  return result.stdout;
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed\n${result.stderr || result.stdout}`);
  }
  if (result.stdout.trim()) console.log(result.stdout.trim());
}
