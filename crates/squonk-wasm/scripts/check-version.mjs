#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const workspaceRoot = join(crateDir, "..", "..");
const cargo = readFileSync(join(workspaceRoot, "Cargo.toml"), "utf8");
const workspaceSection = cargo.match(/\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/)?.[1];
const workspaceVersion = workspaceSection?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (!workspaceVersion) throw new Error("could not read workspace.package.version");

const buildVersion = JSON.parse(readFileSync(join(crateDir, "package.json"), "utf8")).version;
if (buildVersion !== workspaceVersion) {
  throw new Error(`npm build version ${buildVersion} does not match Cargo workspace ${workspaceVersion}`);
}

const stageRoot = join(crateDir, "dist", "npm");
for (const label of readdirSync(stageRoot)) {
  const manifest = JSON.parse(readFileSync(join(stageRoot, label, "package.json"), "utf8"));
  if (manifest.version !== workspaceVersion) {
    throw new Error(`${manifest.name} has version ${manifest.version}, expected ${workspaceVersion}`);
  }
  for (const [dependency, version] of Object.entries(manifest.optionalDependencies ?? {})) {
    if (version !== workspaceVersion) {
      throw new Error(`${manifest.name} pins ${dependency}@${version}, expected ${workspaceVersion}`);
    }
  }
}

console.log(`version consistency OK (${workspaceVersion})`);
