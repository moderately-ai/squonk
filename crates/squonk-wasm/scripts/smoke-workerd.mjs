#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const packageDir = join(crateDir, "dist", "npm", "sqlite");
const tarball = join(packageDir, JSON.parse(capture("npm", ["pack", "--json"], packageDir))[0].filename);
const workDir = mkdtempSync(join(tmpdir(), "squonk-workerd-"));
const outputDir = join(workDir, "bundle");
mkdirSync(outputDir);

try {
  run("npm", ["init", "-y"], workDir);
  run("npm", ["install", "--no-save", tarball], workDir);
  writeFileSync(join(workDir, "worker.mjs"), `
import { parse, runtimeInfo } from "@squonk-sql/sqlite";
export default {
  fetch() {
    return Response.json({ sql: parse("select 1").toSQL(), runtime: runtimeInfo() });
  },
};
`);
  writeFileSync(join(workDir, "wrangler.jsonc"), JSON.stringify({
    name: "squonk-runtime-smoke",
    main: "worker.mjs",
    compatibility_date: "2026-07-13",
  }, null, 2));
  run("npx", ["--yes", "wrangler@4.110.0", "deploy", "--dry-run", "--outdir", outputDir], workDir);

  const server = spawn("npx", ["--yes", "wrangler@4.110.0", "dev", "--port", "8799"], {
    cwd: workDir,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let diagnostics = "";
  server.stdout.on("data", (chunk) => { diagnostics += chunk; });
  server.stderr.on("data", (chunk) => { diagnostics += chunk; });
  try {
    const response = await waitForResponse("http://127.0.0.1:8799");
    const result = await response.json();
    if (result.sql !== "SELECT 1" || result.runtime?.backend !== "wasm" || result.runtime?.host !== "workerd") {
      throw new Error(`unexpected workerd result: ${JSON.stringify(result)}`);
    }
    console.log(JSON.stringify(result));
  } catch (error) {
    throw new Error(`workerd smoke failed\n${diagnostics}`, { cause: error });
  } finally {
    server.kill("SIGTERM");
  }
} finally {
  rmSync(workDir, { force: true, recursive: true });
  rmSync(tarball, { force: true });
}

async function waitForResponse(url) {
  let lastError;
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      const response = await fetch(url);
      if (response.ok) return response;
      lastError = new Error(`HTTP ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw lastError ?? new Error("workerd did not start");
}

function capture(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr || `${command} failed`);
  return result.stdout;
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed\n${result.stderr || result.stdout}`);
}
