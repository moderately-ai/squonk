#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { packages } from "./package-matrix.mjs";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
for (const variant of packages) {
  const args = ["pack", ...(process.argv.includes("--dry-run") ? ["--dry-run"] : []), "--json"];
  const result = spawnSync("npm", args, { cwd: join(crateDir, "dist", "npm", variant.label), encoding: "utf8" });
  if (result.status !== 0) {
    process.stderr.write(result.stderr);
    process.exit(result.status ?? 1);
  }
  process.stdout.write(`${variant.packageName}: ${result.stdout}`);
}
