#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { packages } from "./package-matrix.mjs";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const child = process.argv[2];
if (child) {
  const variant = packages.find((item) => item.label === child);
  if (!variant) throw new Error(`unknown package ${child}`);
  await smoke(variant, process.argv[3]);
} else {
  for (const variant of packages) {
    for (const mode of ["node", "browser"]) {
      const result = spawnSync(process.execPath, [fileURLToPath(import.meta.url), variant.label, mode], {
        cwd: crateDir,
        encoding: "utf8",
      });
      if (result.status !== 0) {
        process.stderr.write(result.stderr || result.stdout);
        process.exit(result.status ?? 1);
      }
      process.stdout.write(result.stdout);
    }
  }
}

async function smoke(variant, mode) {
  const stage = join(crateDir, "dist", "npm", variant.label);
  const browserEntry = variant.entry === "index" ? "browser" : `${variant.entry}-browser`;
  let api;
  if (mode === "browser") {
    const module = await import(pathToFileURL(join(stage, "js", `${browserEntry}.js`)));
    const bytes = await readFile(join(stage, `pkg-${variant.label}`, "squonk_wasm_bg.wasm"));
    try {
      await module.createSquonk({ wasm: new Uint8Array([0]) });
      throw new Error(`${variant.packageName}: invalid browser wasm unexpectedly initialized`);
    } catch (error) {
      if (String(error).includes("unexpectedly initialized")) throw error;
    }
    const first = module.createSquonk({ wasm: bytes });
    const second = module.createSquonk();
    if (first !== second) throw new Error(`${variant.packageName}: concurrent browser loads did not coalesce`);
    api = await first;
  } else {
    api = await import(pathToFileURL(join(stage, "js", `${variant.entry}.js`)));
    if ("init" in api || "defaultWasmUrl" in api) {
      throw new Error(`${variant.packageName}: public Node entrypoint leaked initialization controls`);
    }
  }

  const dialects = api.supportedDialects().map(({ name }) => name).sort();
  assertEqual(dialects, [...variant.supportedDialects].sort(), `${variant.packageName} dialects`);
  const document = api.parse("select 1");
  assertEqual(document.dialect, variant.defaultDialect, `${variant.packageName} default dialect`);
  assertEqual(document.toSQL(), "SELECT 1", `${variant.packageName} render`);
  const binary = [...api.parse("select a + 1 from t").findAll("BinaryOp")][0];
  if (!binary.isRenderable) throw new Error(`${variant.packageName}: expression not renderable`);
  assertEqual(binary.toSQL(), "a + 1", `${variant.packageName} fragment render`);
  const ident = [...api.parse("select a").findAll("Ident")][0];
  if (ident.isRenderable) throw new Error(`${variant.packageName}: identifier marked renderable`);
  try {
    ident.toSQL();
    throw new Error(`${variant.packageName}: context-dependent node rendered`);
  } catch (error) {
    if (error.kind !== "unsupported_node_render") throw error;
  }
  const mutated = api.parse("select 1; select 2");
  mutated.raw.statements.pop();
  assertEqual(mutated.toSQL(), "SELECT 1", `${variant.packageName} mutation-aware render`);
  if (!api.format("select a,b from t").includes("SELECT a, b")) {
    throw new Error(`${variant.packageName}: formatter unavailable`);
  }
  const recovered = api.parseRecovering("select 1; from broken; select 2");
  if (!recovered.toSQL().includes("SELECT 2")) {
    throw new Error(`${variant.packageName}: recovered render omitted survivor`);
  }
  if (api.tokenize("select 1").tokens.length === 0 || api.version().length === 0 || api.schemaVersion() < 1) {
    throw new Error(`${variant.packageName}: tokenizer/version smoke failed`);
  }
  console.log(`ok ${variant.packageName} ${mode}`);
}

function assertEqual(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${label}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}
