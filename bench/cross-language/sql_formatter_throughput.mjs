#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//
// Warm end-to-end throughput: `sql-formatter` vs our wasm parser, over the shared
// conformance corpus, per dialect. The Node sibling of `sqlglot_throughput.py`,
// `calcite_throughput.java`, and `jsqlparser_throughput.java`; it follows the same
// caveat framework (`docs/performance.md`) and prints the same
// self-describing caption so a raw log is interpretable on its own.
//
// WHAT THIS MEASURES (read before reading any number it prints): warm, single-thread,
// end-to-end throughput — statements per wall-clock second — of FOUR workloads over
// the measured subset of one dialect's corpus:
//   (a) THEIR  sql-formatter `format(sql, { language })`  -> re-laid-out SQL string.
//   (b) OURS   `parse(sql, { dialect })`                  -> typed Document (wasm parse
//              + serde-wasm-bindgen marshalling of the AST into JS objects).
//   (c) OURS   `render(sql, { dialect })`                 -> canonical SQL string (wasm
//              parse + render, NO JS-AST materialization; this is the parse+render
//              ROUND-TRIP and the closest analogue of their string-in/string-out call).
//   (d) OURS   `format(sql, { dialect })`                 -> layout-IR pretty-printer behind
//              the document-render/full wasm variant; string in, formatted string out.
//
// WHY (a) vs (c) is the honest comparison, and (b) is a different thing:
//   sql-formatter is a tokenizer + re-layout formatter: string in, formatted string
//   out, no grammar validation. Our (c) is string in, canonical string out, but via a
//   REAL parse (typed AST, dialect feature gates, error reporting) that theirs never
//   does. (b) additionally pays to marshal the whole AST across the wasm boundary into
//   JS objects — a richer deliverable (AST access) that has no counterpart in
//   sql-formatter — so (b) is expected to be SLOWER than (c), not a like-for-like cell.
//   See the notes doc for the full approach framing.
//
// NOT apples-to-apples with the Rust in-process numbers: this pays the JS<->wasm
// boundary on every call and V8's own warm-up. It is a JS-consumer positioning number
// ("which library is faster to reach for from Node"), captioned as such. Memory is
// excluded for the same cross-runtime reasons as the other runners (see notes §4).
//
// BOTH-ACCEPT SUBSET degenerates to OUR accepts. sql-formatter's `format()` is
// permissive by design and (for well-formed SQL) does not reject, so the "statements
// both tools handle" rule reduces to "statements OUR parser accepts under this
// dialect". The runner still probes their side under try/catch and drops any id their
// `format()` throws on, so the measured set is truly `ours ∩ theirs`; in practice the
// intersection equals our accept set, and the caption reports if theirs ever dropped one.
//
// sql-formatter API surface this is coded against (pinned in package.json):
//   `import { format } from "sql-formatter";`
//   `format(query: string, cfg?: { language?: string, ... }): string`
//   `language` values used here: "postgresql" | "mysql" | "sqlite" (the dialect knob;
//   sql-formatter also ships bigquery, db2, hive, mariadb, n1ql, plsql, redshift,
//   spark, sql, tidb, trino, transactsql/tsql). `format` throws only on an UNKNOWN
//   language or a genuine tokenizer error, never as grammar validation. This API
//   (`format(sql, { language })`) has been stable across sql-formatter v4–v15.
//   RUN-DAY: verify the pinned version resolves and the signature is unchanged.
//
// OFFLINE: this file `node --check`s and runs `--dry-run` with NO network, NO wasm
// build, and NO sql-formatter installed — dry-run exercises the corpus-read +
// argument-parsing + JSON-shape paths and exits before importing either engine.
//
// USAGE
//   node sql_formatter_throughput.mjs --dialect postgres [--subset both_accept.txt]
//   node sql_formatter_throughput.mjs --dialect mysql --emit-accepts mysql.ids
//   node sql_formatter_throughput.mjs --dialect postgres --json report.postgres.json
//   node sql_formatter_throughput.mjs --dialect sqlite  --dry-run
//
// Phases mirror the Python/Java runners: accept-probe (untimed) -> optional subset
// intersection -> warm-up (excludes V8/wasm-instantiation warm-up) -> timed passes.

import { writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";

import { CORPORA, defaultCorpusRoot, loadCandidates } from "./corpus_loader.mjs";

// Dialect -> (our wasm subpath variant, sql-formatter `language`). We use the
// per-dialect SERIALIZE-ONLY variant (smallest blob that compiles this dialect and
// still exposes `parse` + string `render`); the notes doc names the blob size per
// variant. `render(string)` funnels to the wasm `render_sql` export, which every
// serialize-only variant provides (a `*-full` build is only needed to render a
// JS-mutated or recovered AST object, which we never do here).
const DIALECTS = {
  postgres: { variant: "postgres", fullVariant: "postgres-full", language: "postgresql" },
  mysql: { variant: "mysql", fullVariant: "mysql-full", language: "mysql" },
  sqlite: { variant: "sqlite", fullVariant: "sqlite-full", language: "sqlite" },
};

function parseArgs(argv) {
  const args = {
    dialect: "postgres",
    corpusRoot: null,
    subset: null,
    emitAccepts: null,
    warmupSecs: 2.0,
    reps: 7,
    minPassSecs: 0.2,
    json: null,
    dryRun: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => argv[(i += 1)];
    switch (a) {
      case "--dialect": args.dialect = next(); break;
      case "--corpus-root": args.corpusRoot = next(); break;
      case "--subset": args.subset = next(); break;
      case "--emit-accepts": args.emitAccepts = next(); break;
      case "--warmup-secs": args.warmupSecs = Number(next()); break;
      case "--reps": args.reps = Number(next()); break;
      case "--min-pass-secs": args.minPassSecs = Number(next()); break;
      case "--json": args.json = next(); break;
      case "--dry-run": args.dryRun = true; break;
      case "-h": case "--help": args.help = true; break;
      default:
        throw new Error(`unknown argument: ${a}`);
    }
  }
  if (!(args.dialect in DIALECTS)) {
    throw new Error(`--dialect must be one of ${Object.keys(DIALECTS).join(", ")} (got ${args.dialect})`);
  }
  return args;
}

function loadSubsetIds(text) {
  const ids = new Set();
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (line !== "" && !line.startsWith("#")) ids.add(line);
  }
  return ids;
}

// One measurement: `passes` full sweeps of `sqls`, applying `fn` to each. Returns
// statements/sec. `sink` folds each result so V8 cannot elide the call; foreign wasm
// calls are not DCE-able anyway, but this keeps parity with the JVM runners' blackhole
// and forces the returned string/object to be produced.
function timedRate(fn, sqls, passes) {
  let sink = 0;
  const t0 = performance.now();
  for (let p = 0; p < passes; p += 1) {
    for (const sql of sqls) {
      const r = fn(sql);
      sink = (sink + (typeof r === "string" ? r.length : r === undefined ? 0 : 1)) | 0;
    }
  }
  const dt = (performance.now() - t0) / 1000;
  if (sink === 0x1234567) process.stderr.write("");
  return dt > 0 ? (passes * sqls.length) / dt : Infinity;
}

// Inner-loop count so one timed pass spans >= minPassSecs (clock-noise floor),
// calibrated from one timed pass — mirrors the Python runner's `calibrate_passes`.
function calibratePasses(fn, sqls, minPassSecs) {
  let sink = 0;
  const t0 = performance.now();
  for (const sql of sqls) {
    const r = fn(sql);
    sink = (sink + (typeof r === "string" ? r.length : 1)) | 0;
  }
  const onePass = (performance.now() - t0) / 1000;
  if (sink === 0x1234567) process.stderr.write("");
  if (onePass <= 0) return 1024;
  return Math.max(1, Math.floor(minPassSecs / onePass) + 1);
}

// Warm-up: loop the subset until warmupSecs elapses (>= one full pass). This leaves
// V8 tier-up, the one-time wasm instantiation, and sql-formatter's module import
// entirely OUTSIDE the measured window.
function warmUp(fn, sqls, warmupSecs) {
  const end = performance.now() + warmupSecs * 1000;
  let passes = 0;
  let sink = 0;
  while (performance.now() < end) {
    for (const sql of sqls) {
      const r = fn(sql);
      sink = (sink + (typeof r === "string" ? r.length : 1)) | 0;
    }
    passes += 1;
  }
  if (passes === 0) {
    for (const sql of sqls) fn(sql);
  }
  if (sink === 0x1234567) process.stderr.write("");
}

function median(xs) {
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

function measure(fn, sqls, { warmupSecs, reps, minPassSecs }) {
  warmUp(fn, sqls, warmupSecs);
  const passes = calibratePasses(fn, sqls, minPassSecs);
  const rates = [];
  for (let r = 0; r < reps; r += 1) rates.push(timedRate(fn, sqls, passes));
  return { best: Math.max(...rates), median: median(rates), passes };
}

// Coverage of OUR parser over the full corpus (accept-probe, untimed, broad catch).
// A reject is "did not build an AST", however the parser signals it.
function acceptProbe(parse, candidates, dialect) {
  const accepted = new Set();
  const coverage = Object.fromEntries(CORPORA.map(([k]) => [k, [0, 0]]));
  for (const c of candidates) {
    coverage[c.corpus][1] += 1;
    try {
      parse(c.sql, { dialect });
    } catch {
      continue;
    }
    accepted.add(c.id);
    coverage[c.corpus][0] += 1;
  }
  return { accepted, coverage };
}

// Probe THEIR side too, so the measured set is genuinely `ours ∩ theirs`. Returns the
// ids (from `ids`) their `format()` throws on — expected to be empty for well-formed
// SQL (their formatter is permissive), but measured rather than assumed.
function theirRejects(format, byId, ids, language) {
  const rejects = new Set();
  for (const id of ids) {
    const c = byId.get(id);
    if (!c) continue;
    try {
      format(c.sql, { language });
    } catch {
      rejects.add(id);
    }
  }
  return rejects;
}

function fmt(n) {
  return n.toLocaleString("en-US", { maximumFractionDigits: 0 });
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write("see the header of this file, or docs/performance.md\n");
    return 0;
  }
  const { variant, fullVariant, language } = DIALECTS[args.dialect];
  const corpusRoot = args.corpusRoot || process.env.SQUONK_CORPUS_ROOT || defaultCorpusRoot();
  const candidates = loadCandidates(corpusRoot);
  const byId = new Map(candidates.map((c) => [c.id, c]));

  // DRY-RUN: prove the offline paths (corpus read + args + JSON shape) without importing
  // the wasm package (needs a build) or sql-formatter (needs an install).
  if (args.dryRun) {
    const totals = Object.fromEntries(CORPORA.map(([k]) => [k, 0]));
    for (const c of candidates) totals[c.corpus] += 1;
    const skeleton = {
      tool_ours_variant: variant,
      tool_theirs: `sql-formatter (language=${language})`,
      dialect: args.dialect,
      corpus_root: corpusRoot,
      candidates_total: candidates.length,
      candidates_per_corpus: totals,
      measured_subset: null,
      workloads: {
        theirs_format: { best: null, median: null },
        ours_parse: { best: null, median: null },
        ours_render_roundtrip: { best: null, median: null },
        ours_format_pretty: { best: null, median: null },
      },
      note: "dry-run: no engines loaded, no timing performed",
    };
    process.stdout.write("# DRY-RUN (offline): corpus + args + JSON shape only; no wasm build, no sql-formatter\n");
    for (const [k] of CORPORA) process.stdout.write(`#   ${k.padEnd(28)} ${String(totals[k]).padStart(5)}\n`);
    process.stdout.write(`#   ${"TOTAL".padEnd(28)} ${String(candidates.length).padStart(5)}\n`);
    process.stdout.write(JSON.stringify(skeleton, null, 2) + "\n");
    if (args.json) {
      writeFileSync(args.json, JSON.stringify(skeleton, null, 2) + "\n");
      process.stdout.write(`# wrote dry-run skeleton to ${args.json}\n`);
    }
    return 0;
  }

  // Import our wasm variants. `defaultWasmUrl` points into the (gitignored) pkg dir,
  // so `crates/squonk-wasm` must have been built (`npm run build`) first. The
  // serialize-only variant is used for parse/render; the full variant is needed for
  // the document-render-gated `format()` export.
  let ours;
  let oursFull;
  try {
    ours = await import(`../../crates/squonk-wasm/js/${variant}.js`);
    oursFull = await import(`../../crates/squonk-wasm/js/${fullVariant}.js`);
  } catch (err) {
    process.stderr.write(
      `error: could not import our wasm variants '${variant}'/'${fullVariant}'.\n` +
      "       Build them first:  (cd crates/squonk-wasm && npm install && npm run build)\n" +
      `       cause: ${err.message}\n`,
    );
    return 2;
  }
  await ours.init(await readFile(ours.defaultWasmUrl));
  await oursFull.init(await readFile(oursFull.defaultWasmUrl));
  const parse = ours.parse;
  const render = ours.render;
  const oursPrettyFormat = oursFull.format;
  const oursVersion = (() => {
    try { return ours.version(); } catch { return "unknown"; }
  })();

  // Import sql-formatter (a devDependency; see package.json). Absent in the sandboxed
  // worktree — mirror the Python runner's ImportError message and exit cleanly.
  let format;
  let theirsVersion = "unknown";
  try {
    const mod = await import("sql-formatter");
    format = mod.format;
    try {
      const pkg = await import("sql-formatter/package.json", { with: { type: "json" } });
      theirsVersion = pkg.default?.version ?? "unknown";
    } catch { /* version is best-effort */ }
  } catch (err) {
    process.stderr.write(
      "error: sql-formatter is not installed. From bench/cross-language/ run:\n" +
      "       npm install   (installs the pinned devDependency; see package.json)\n" +
      `       cause: ${err.message}\n`,
    );
    return 2;
  }

  // Accept-probe (untimed) -> our accept set = the both-accept gate.
  const { accepted, coverage } = acceptProbe(parse, candidates, args.dialect);

  if (args.emitAccepts) {
    const ids = [...accepted].sort();
    writeFileSync(args.emitAccepts, ids.join("\n") + (ids.length ? "\n" : ""));
    process.stdout.write(
      `wrote ${ids.length} accepted ids to ${args.emitAccepts} ` +
      `(ours ${oursVersion}, dialect=${args.dialect})\n`,
    );
    return 0;
  }

  // Subset selection: requested ids ∩ our accepts ∩ their accepts.
  let requested = null;
  let measuredIds;
  let missingOurs = [];
  if (args.subset) {
    requested = loadSubsetIds(await readFile(args.subset, "utf8"));
    measuredIds = [...requested].filter((id) => accepted.has(id)).sort();
    missingOurs = [...requested].filter((id) => !accepted.has(id)).sort();
  } else {
    measuredIds = [...accepted].sort();
  }
  const theirDropped = theirRejects(format, byId, measuredIds, language);
  if (theirDropped.size) measuredIds = measuredIds.filter((id) => !theirDropped.has(id));

  const measuredSqls = measuredIds.map((id) => byId.get(id).sql);
  if (measuredSqls.length === 0) {
    process.stderr.write("error: measured subset is empty (no accepted ids to time)\n");
    return 1;
  }

  const opts = { warmupSecs: args.warmupSecs, reps: args.reps, minPassSecs: args.minPassSecs };
  const theirs = measure((sql) => format(sql, { language }), measuredSqls, opts);
  const oursParse = measure((sql) => parse(sql, { dialect: args.dialect }), measuredSqls, opts);
  const oursRender = measure((sql) => render(sql, { dialect: args.dialect }), measuredSqls, opts);
  const oursFormat = measure((sql) => oursPrettyFormat(sql, { dialect: args.dialect }), measuredSqls, opts);

  const totalCandidates = Object.values(coverage).reduce((s, [, t]) => s + t, 0);
  const totalAccepted = Object.values(coverage).reduce((s, [a]) => s + a, 0);

  const report = {
    schema: "squonk.cross-language.js-sql-formatter/1",
    dialect: args.dialect,
    ours: {
      variant,
      full_variant: fullVariant,
      version: oursVersion,
      note: "per-dialect serialize-only wasm variant for parse/render; per-dialect full variant for format()",
    },
    theirs: { tool: "sql-formatter", version: theirsVersion, language },
    runtime: `node ${process.version}`,
    corpus_root: corpusRoot,
    metric: "statements/sec = statements / wall_seconds (warm, 1 thread, END-TO-END, JS<->wasm boundary included)",
    method: {
      warmup_secs: args.warmupSecs,
      reps: args.reps,
      min_pass_secs: args.minPassSecs,
      passes_theirs: theirs.passes,
      passes_ours_parse: oursParse.passes,
      passes_ours_render: oursRender.passes,
      passes_ours_format: oursFormat.passes,
    },
    coverage: Object.fromEntries(CORPORA.map(([k]) => [k, { accepted: coverage[k][0], total: coverage[k][1] }])),
    coverage_total: { accepted: totalAccepted, total: totalCandidates },
    subset: {
      file: args.subset,
      requested: requested ? requested.size : null,
      missing_ours: missingOurs.length,
      dropped_theirs: theirDropped.size,
      measured: measuredSqls.length,
      both_accept_degenerates_to_ours: theirDropped.size === 0,
    },
    throughput: {
      theirs_format: { best: theirs.best, median: theirs.median },
      ours_parse: { best: oursParse.best, median: oursParse.median },
      ours_render_roundtrip: { best: oursRender.best, median: oursRender.median },
      ours_format_pretty: { best: oursFormat.best, median: oursFormat.median },
    },
  };

  // Human-readable captioned block (self-describing, like the Python/Java runners).
  const L = process.stdout;
  L.write("# cross-language throughput: sql-formatter vs squonk (wasm)\n");
  L.write(`#   runtime         : Node ${process.version}  (V8; JS<->wasm boundary paid per call; single-thread)\n`);
  L.write(`#   ours            : squonk ${oursVersion}  variant=${variant} (per-dialect, serialize-only)\n`);
  L.write(`#   theirs          : sql-formatter ${theirsVersion}  language=${language}\n`);
  L.write(`#   dialect         : ${args.dialect}\n`);
  L.write(`#   corpus root     : ${corpusRoot}\n`);
  L.write("#   metric          : statements/sec = statements / wall_seconds (warm, 1 thread, END-TO-END)\n");
  L.write(`#   method          : warm-up >= ${args.warmupSecs}s (excl. V8/wasm-instantiation warm-up), ${args.reps} timed passes\n`);
  if (args.subset) {
    L.write(`#   subset          : ${args.subset}  (${requested.size} requested ids)\n`);
    if (missingOurs.length) {
      L.write(`#   WARNING         : ${missingOurs.length} requested id(s) NOT accepted by OUR parser -> excluded\n`);
    }
  } else {
    L.write("#   subset          : OUR-ACCEPTS (both-accept degenerates to ours; see notes)\n");
  }
  if (theirDropped.size) {
    L.write(`#   note            : sql-formatter threw on ${theirDropped.size} id(s) -> excluded (intersection is ours ∩ theirs)\n`);
  } else {
    L.write("#   note            : sql-formatter rejected 0 ids -> both-accept == our accept set\n");
  }
  L.write("#\n");
  L.write("# coverage (OUR parser accepts / candidates), per corpus:\n");
  for (const [k] of CORPORA) L.write(`#   ${k.padEnd(28)} ${String(coverage[k][0]).padStart(5)}/${coverage[k][1]}\n`);
  L.write(`#   ${"TOTAL".padEnd(28)} ${String(totalAccepted).padStart(5)}/${totalCandidates}\n`);
  L.write("#\n");
  L.write(`# throughput over the measured subset (${measuredSqls.length} statements), statements/sec:\n`);
  L.write(`#   ${"workload".padEnd(34)} ${"best".padStart(14)} ${"median".padStart(14)}\n`);
  const row = (label, m) => L.write(`#   ${label.padEnd(34)} ${fmt(m.best).padStart(14)} ${fmt(m.median).padStart(14)}\n`);
  row("(a) sql-formatter format()", theirs);
  row("(b) ours parse() -> typed AST", oursParse);
  row("(c) ours render() round-trip", oursRender);
  L.write("#\n");
  L.write("#   (a) vs (c) is the comparable string->string cell; (b) also marshals the AST to JS (richer, slower).\n");

  if (args.json) {
    writeFileSync(args.json, JSON.stringify(report, null, 2) + "\n");
    L.write(`# wrote JSON report to ${args.json}\n`);
  }
  return 0;
}

main().then((code) => process.exit(code)).catch((err) => {
  process.stderr.write(String(err.stack || err) + "\n");
  process.exit(1);
});
