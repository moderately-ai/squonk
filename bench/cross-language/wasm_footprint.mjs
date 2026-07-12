#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//
// Footprint companion for the sql-formatter comparison: measures OUR shipped bytes a
// JS consumer downloads — the wasm blob + the wasm-bindgen JS glue + the typed facade,
// raw and gzipped — per dialect variant.
//
// TWO SOURCES OF TRUTH, because the wasm blobs are BUILD ARTIFACTS, not committed:
//   * The `crates/squonk-wasm/pkg-*` dirs (which hold `squonk_wasm_bg.wasm`
//     and the generated `squonk_wasm.js` glue) are GITIGNORED
//     (`crates/squonk-wasm/pkg-*/`). They exist only AFTER `npm run build`. When
//     present, this script measures them directly (raw + gzip).
//   * The typed facade (`crates/squonk-wasm/js/*.js`, `runtime.js`,
//     `ast-metadata.generated.js`) IS committed, so its size is measurable OFFLINE and
//     is reported unconditionally.
//
// So: run with the pkg dirs built for the full wasm+glue+facade breakdown; run offline
// for the committed-facade numbers plus a pointer to the authoritative wasm blob table
// (the `crates/squonk-wasm/README.md` "Size" section, measured by
// `npm run build:wasm`, which writes a `size-report.json` per pkg dir).
//
// gzip is what a browser actually downloads; both raw and gzip are reported. Startup
// cost (wasm instantiation vs a pure-JS import) is discussed in the notes doc, not
// timed here (it is a one-time cost, excluded from the throughput warm-up by design).
//
// USAGE
//   node wasm_footprint.mjs                 # measure every built pkg-* + the facade
//   node wasm_footprint.mjs --json foot.json
//   node wasm_footprint.mjs postgres mysql  # only these variants
//   node wasm_footprint.mjs --facade-only   # committed facade only (fully offline)

import { gzipSync } from "node:zlib";
import { readFileSync, writeFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const WASM_CRATE = join(HERE, "..", "..", "crates", "squonk-wasm");
const JS_DIR = join(WASM_CRATE, "js");

// variant -> [pkg dir name, facade js basename]. The `.` default is ANSI-only.
const VARIANTS = [
  ["ansi", "pkg", "index.js"],
  ["postgres", "pkg-postgres", "postgres.js"],
  ["mysql", "pkg-mysql", "mysql.js"],
  ["sqlite", "pkg-sqlite", "sqlite.js"],
  ["duckdb", "pkg-duckdb", "duckdb.js"],
  ["lenient", "pkg-lenient", "lenient.js"],
  ["dialects", "pkg-dialects", "dialects.js"],
  ["ansi-full", "pkg-ansi-full", "ansi-full.js"],
  ["postgres-full", "pkg-postgres-full", "postgres-full.js"],
  ["mysql-full", "pkg-mysql-full", "mysql-full.js"],
  ["sqlite-full", "pkg-sqlite-full", "sqlite-full.js"],
  ["duckdb-full", "pkg-duckdb-full", "duckdb-full.js"],
  ["lenient-full", "pkg-lenient-full", "lenient-full.js"],
  ["full", "pkg-full", "full.js"],
];

// Shared JS every facade pulls in (the wrapper metadata a consumer also downloads).
const SHARED_FACADE = ["runtime.js", "ast-metadata.generated.js", "ast-metadata.generated.d.ts"];

function sizes(path) {
  const raw = readFileSync(path);
  return { raw: raw.length, gzip: gzipSync(raw, { level: 9 }).length };
}

function kib(n) {
  return (n / 1024).toFixed(1) + " KiB";
}

function measureVariant(pkgDir, facadeName) {
  const out = { pkg_present: false, wasm: null, glue: null, facade: null };
  const facadePath = join(JS_DIR, facadeName);
  if (existsSync(facadePath)) out.facade = sizes(facadePath);
  const pkgPath = join(WASM_CRATE, pkgDir);
  if (existsSync(pkgPath) && statSync(pkgPath).isDirectory()) {
    out.pkg_present = true;
    for (const f of readdirSync(pkgPath)) {
      const p = join(pkgPath, f);
      if (f.endsWith("_bg.wasm")) out.wasm = sizes(p);
      else if (f.endsWith(".js") && !f.endsWith("_bg.js")) out.glue = sizes(p);
    }
  }
  return out;
}

function main() {
  const argv = process.argv.slice(2);
  let jsonPath = null;
  let facadeOnly = false;
  const only = [];
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === "--json") jsonPath = argv[(i += 1)];
    else if (argv[i] === "--facade-only") facadeOnly = true;
    else only.push(argv[i]);
  }
  const wanted = VARIANTS.filter(([name]) => only.length === 0 || only.includes(name));

  const shared = {};
  let sharedGzip = 0;
  for (const f of SHARED_FACADE) {
    const p = join(JS_DIR, f);
    if (existsSync(p)) { shared[f] = sizes(p); if (f.endsWith(".js")) sharedGzip += shared[f].gzip; }
  }

  const rows = [];
  for (const [name, pkgDir, facadeName] of wanted) {
    rows.push({ name, ...(facadeOnly
      ? { facade: existsSync(join(JS_DIR, facadeName)) ? sizes(join(JS_DIR, facadeName)) : null, pkg_present: false }
      : measureVariant(pkgDir, facadeName)) });
  }

  const anyPkg = rows.some((r) => r.pkg_present);
  const report = { schema: "squonk.footprint/1", shared_facade_js_gzip: sharedGzip, shared, variants: rows };

  const L = process.stdout;
  L.write("# squonk-wasm footprint (bytes a JS consumer downloads)\n");
  L.write(`#   shared facade JS (runtime.js + ast-metadata.generated.js), gzip: ${kib(sharedGzip)} (loaded once, all variants)\n`);
  if (!anyPkg && !facadeOnly) {
    L.write("#   pkg-* dirs NOT built (they are gitignored build artifacts). Showing committed FACADE sizes only.\n");
    L.write("#   For wasm blob sizes, build first:  (cd crates/squonk-wasm && npm install && npm run build)\n");
    L.write("#   or read the authoritative table in crates/squonk-wasm/README.md (Size section).\n");
  }
  L.write("#\n");
  L.write(`#   ${"variant".padEnd(16)} ${"wasm raw".padStart(11)} ${"wasm gzip".padStart(11)} ${"glue gzip".padStart(11)} ${"facade gzip".padStart(12)}\n`);
  for (const r of rows) {
    const wasmRaw = r.wasm ? kib(r.wasm.raw) : "-";
    const wasmGz = r.wasm ? kib(r.wasm.gzip) : "(build)";
    const glueGz = r.glue ? kib(r.glue.gzip) : "(build)";
    const facGz = r.facade ? kib(r.facade.gzip) : "-";
    L.write(`#   ${r.name.padEnd(16)} ${wasmRaw.padStart(11)} ${wasmGz.padStart(11)} ${glueGz.padStart(11)} ${facGz.padStart(12)}\n`);
  }
  L.write("#\n");
  L.write("#   'facade gzip' is the committed thin per-variant entrypoint; add shared facade JS gzip once.\n");
  L.write("#   Over-the-wire total for a variant ~= wasm gzip + glue gzip + facade gzip + shared facade gzip.\n");

  if (jsonPath) {
    writeFileSync(jsonPath, JSON.stringify(report, null, 2) + "\n");
    L.write(`# wrote JSON footprint to ${jsonPath}\n`);
  }
  return 0;
}

process.exit(main());
