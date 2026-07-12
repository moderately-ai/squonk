#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { packages } from "./package-matrix.mjs";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const budgets = JSON.parse(readFileSync(join(crateDir, "size-budget.json"), "utf8")).packages;
const checkTarballs = process.argv.includes("--tarball");
const breaches = [];

for (const variant of packages) {
  const limits = budgets[variant.label];
  if (!limits) {
    breaches.push(`${variant.label}: missing budget`);
    continue;
  }
  const report = JSON.parse(readFileSync(join(crateDir, "size-reports", `${variant.label}.json`), "utf8"));
  check(`${variant.label} raw`, report.final.raw, limits.rawBytes);
  check(`${variant.label} gzip`, report.final.gzip, limits.gzipBytes);
  if (checkTarballs) {
    const packed = pack(variant.label);
    check(`${variant.label} packed`, packed.size, limits.packedBytes);
    check(`${variant.label} unpacked`, packed.unpackedSize, limits.unpackedBytes);
    check(`${variant.label} entries`, packed.entryCount, limits.entryCount);
  }
}

if (breaches.length > 0) {
  console.error(`size budget FAILED:\n${breaches.map((item) => `  - ${item}`).join("\n")}`);
  process.exit(1);
}
console.log(`size budget OK (${packages.length} packages${checkTarballs ? " + tarballs" : ""})`);

function pack(label) {
  const result = spawnSync("npm", ["pack", "--dry-run", "--json"], {
    cwd: join(crateDir, "dist", "npm", label), encoding: "utf8",
  });
  if (result.status !== 0) throw new Error(result.stderr || `npm pack failed for ${label}`);
  return JSON.parse(result.stdout)[0];
}

function check(label, actual, ceiling) {
  if (actual > ceiling) breaches.push(`${label}: ${actual} exceeds ${ceiling}`);
}
