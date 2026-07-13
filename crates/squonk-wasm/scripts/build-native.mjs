#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const workspaceRoot = join(crateDir, "..", "..");
const target = process.env.SQUONK_NATIVE_TARGET;
const packageLabel = process.env.SQUONK_NATIVE_PACKAGE;
const cargoSubcommand = process.env.SQUONK_CARGO_SUBCOMMAND ?? "build";
const args = [cargoSubcommand, "--release", "-p", "squonk-node"];
if (target) args.push("--target", target);

const result = spawnSync("cargo", args, { cwd: workspaceRoot, stdio: "inherit" });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);

const artifactTarget = target?.replace(/\.\d+\.\d+$/, "");
const targetDir = join(workspaceRoot, "target", ...(artifactTarget ? [artifactTarget] : []), "release");
const source = join(targetDir, nativeLibraryName());
const destinationDir = packageLabel
  ? join(crateDir, "native-artifacts", packageLabel)
  : join(crateDir, "native");
mkdirSync(destinationDir, { recursive: true });
copyFileSync(source, join(destinationDir, "squonk.node"));

function nativeLibraryName() {
  if (target?.includes("windows") || (!target && process.platform === "win32")) return "squonk_node.dll";
  if (target?.includes("apple") || (!target && process.platform === "darwin")) return "libsquonk_node.dylib";
  return "libsquonk_node.so";
}
