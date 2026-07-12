#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { packages } from "./package-matrix.mjs";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
for (const variant of packages) {
  const packageDir = join(crateDir, "dist", "npm", variant.label);
  const packed = runCapture("npm", ["pack", "--json"], packageDir);
  const tarball = join(packageDir, JSON.parse(packed)[0].filename);
  const workDir = mkdtempSync(join(tmpdir(), `squonk-${variant.label}-`));
  try {
    run("npm", ["init", "-y"], workDir);
    run("npm", ["install", "--no-save", "--install-strategy=nested", tarball], workDir);
    writeFileSync(join(workDir, "smoke.mjs"), consumerScript(variant));
    run("node", ["smoke.mjs"], workDir);
    console.log(`install ok ${variant.packageName}`);
  } finally {
    rmSync(workDir, { force: true, recursive: true });
    rmSync(tarball, { force: true });
  }
}

function consumerScript(variant) {
  return `import { strict as assert } from "node:assert";
import { parse, format, parseRecovering, schemaVersion, supportedDialects } from ${JSON.stringify(variant.packageName)};
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
  const result = spawnSync(command, args, { cwd, stdio: "ignore" });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed`);
}
