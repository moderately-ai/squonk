#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { packages } from "./package-matrix.mjs";
import { currentNativePackage } from "./native-package-matrix.mjs";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const expectedVersion = JSON.parse(readFileSync(join(crateDir, "package.json"), "utf8")).version;
const platformPackage = currentNativePackage();
const nativeDir = platformPackage === null ? null : join(
  crateDir,
  "dist",
  "npm",
  platformPackage.packageName.slice("@squonk-sql/".length),
);
const nativeTarball = nativeDir === null || !existsSync(nativeDir) ? null : join(
  nativeDir,
  JSON.parse(runCapture("npm", ["pack", "--json"], nativeDir))[0].filename,
);
for (const variant of packages) {
  const packageDir = join(crateDir, "dist", "npm", variant.label);
  const packed = runCapture("npm", ["pack", "--json"], packageDir);
  const tarball = join(packageDir, JSON.parse(packed)[0].filename);
  const workDir = mkdtempSync(join(tmpdir(), `squonk-${variant.label}-`));
  try {
    run("npm", ["init", "-y"], workDir);
    const artifacts = nativeTarball === null ? [tarball] : [nativeTarball, tarball];
    run("npm", ["install", "--no-save", "--install-strategy=nested", ...artifacts], workDir);
    writeFileSync(join(workDir, "smoke.mjs"), consumerScript(variant, nativeTarball === null ? "wasm" : "native"));
    run("node", ["smoke.mjs"], workDir);
    writeFileSync(join(workDir, "smoke-no-addons.mjs"), consumerScript(variant, "wasm"));
    run("node", ["--no-addons", "smoke-no-addons.mjs"], workDir);
    writeFileSync(join(workDir, "smoke.cjs"), `
const assert = require("node:assert").strict;
const squonk = require(${JSON.stringify(variant.packageName)});
assert.equal(squonk.parse("select 1").toSQL(), "SELECT 1");
assert.equal(squonk.runtimeInfo().backend, ${JSON.stringify(nativeTarball === null ? "wasm" : "native")});
`);
    run("node", ["smoke.cjs"], workDir);
    console.log(`install ok ${variant.packageName}`);
  } finally {
    rmSync(workDir, { force: true, recursive: true });
    rmSync(tarball, { force: true });
  }
}
if (nativeTarball !== null) rmSync(nativeTarball, { force: true });

function consumerScript(variant, expectedBackend) {
  return `import { strict as assert } from "node:assert";
import { parse, format, parseRecovering, runtimeInfo, schemaVersion, supportedDialects, version } from ${JSON.stringify(variant.packageName)};
assert.equal(version(), ${JSON.stringify(expectedVersion)});
assert.equal(runtimeInfo().backend, ${JSON.stringify(expectedBackend)});
const document = parse("select 1");
assert.equal(document.dialect, ${JSON.stringify(variant.defaultDialect)});
assert.equal(document.toSQL(), "SELECT 1");
assert.ok(schemaVersion() >= 1);
assert.ok(format("select a,b from t").includes("SELECT a, b"));
assert.ok(parseRecovering("select 1; from broken; select 2").toSQL().includes("SELECT 2"));
assert.deepEqual(supportedDialects().map(x => x.name).sort(), ${JSON.stringify([...variant.supportedDialects].sort())});
console.log("consumer smoke OK");
`;
}

function runCapture(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr || `${command} failed`);
  return result.stdout;
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed\n${result.stderr || result.stdout}`);
  }
}
