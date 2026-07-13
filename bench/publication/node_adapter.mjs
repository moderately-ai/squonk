// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import process from "node:process";
import readline from "node:readline/promises";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

import { createSquonkApi } from "../../crates/squonk-wasm/js/runtime.js";

const HERE = new URL("./", import.meta.url);
const DEFAULT_CORPUS = new URL("./corpus/portable.json", HERE);
const require = createRequire(import.meta.url);

function args() {
  const values = { mode: process.argv[2], tool: null, count: 0, corpus: DEFAULT_CORPUS };
  for (let i = 3; i < process.argv.length; i += 1) {
    if (process.argv[i] === "--tool") values.tool = process.argv[++i];
    else if (process.argv[i] === "--count") values.count = Number(process.argv[++i]);
    else if (process.argv[i] === "--corpus") values.corpus = new URL(`file://${process.argv[++i]}`);
    else throw new Error(`unknown argument: ${process.argv[i]}`);
  }
  if (!["qualify", "throughput", "retain", "cold"].includes(values.mode)) throw new Error("invalid mode");
  if (!["squonk", "node-sql-parser"].includes(values.tool)) throw new Error("invalid --tool");
  return values;
}

async function loadCorpus(url) {
  return JSON.parse(await readFile(url, "utf8"));
}

async function adapter(tool) {
  if (tool === "squonk") {
    // Exercise the exact checked-out generated facade over the freshly built
    // Node-API addon. This avoids requiring browser Wasm artifacts for a native
    // benchmark while preserving the public parse/document implementation.
    const bindings = require("../../crates/squonk-wasm/native/squonk.node");
    const module = createSquonkApi(bindings, {
      defaultDialect: "ansi",
      supportedDialects: ["ansi"],
      runtime: { backend: "native", host: "node" },
    });
    return {
      version: module.version(),
      parse: (sql) => module.parse(sql),
      serialize: (document) => JSON.stringify(document.raw),
    };
  }
  const module = await import("node-sql-parser");
  const parser = new module.default.Parser();
  const metadata = JSON.parse(await readFile(new URL("./node_modules/node-sql-parser/package.json", HERE), "utf8"));
  return {
    version: metadata.version,
    parse: (sql) => parser.astify(sql, { database: "MySQL" }),
    serialize: (ast) => JSON.stringify(ast),
  };
}

function digest(payloads) {
  const hash = createHash("sha256");
  for (const payload of payloads) {
    const bytes = Buffer.from(payload);
    const length = Buffer.alloc(8);
    length.writeBigUInt64BE(BigInt(bytes.length));
    hash.update(length);
    hash.update(bytes);
  }
  return hash.digest("hex");
}

function parseBatch(parse, sql) {
  let sink;
  for (const statement of sql) sink = parse(statement);
  return sink;
}

async function qualify(tool, corpus) {
  const implementation = await adapter(tool);
  const payloads = [];
  const failures = [];
  for (const item of corpus.statements) {
    try { payloads.push(implementation.serialize(implementation.parse(item.sql))); }
    catch (error) { failures.push({ id: item.id, error: String(error.message || error) }); }
  }
  return {
    schema: "squonk.publication-adapter/1", ecosystem: "node", tool,
    version: implementation.version, mode: "qualify", corpus_sha256: corpus.sha256,
    requested: corpus.statements.length, accepted: corpus.statements.length - failures.length,
    ast_digest: digest(payloads), failures,
  };
}

async function throughput(tool, corpus) {
  const implementation = await adapter(tool);
  const sql = corpus.statements.map((item) => item.sql);
  const bytes = sql.reduce((total, statement) => total + Buffer.byteLength(statement), 0);
  const warmupStarted = performance.now();
  let sink;
  while (performance.now() - warmupStarted < 2000) sink = parseBatch(implementation.parse, sql);
  const calibrationStarted = performance.now();
  sink = parseBatch(implementation.parse, sql);
  const passes = Math.max(1, Math.ceil(1000 / (performance.now() - calibrationStarted)));
  const samples = [];
  for (let sample = 0; sample < 7; sample += 1) {
    const started = performance.now();
    for (let pass = 0; pass < passes; pass += 1) sink = parseBatch(implementation.parse, sql);
    const seconds = (performance.now() - started) / 1000;
    samples.push({
      seconds,
      statements_per_second: sql.length * passes / seconds,
      mib_per_second: bytes * passes / seconds / (1024 * 1024),
    });
  }
  const values = samples.map((sample) => sample.mib_per_second).sort((a, b) => a - b);
  return {
    schema: "squonk.publication-adapter/1", ecosystem: "node", tool,
    version: implementation.version, mode: "throughput", corpus_sha256: corpus.sha256,
    passes_per_sample: passes, samples, median_mib_per_second: values[Math.floor(values.length / 2)],
    sink_type: typeof sink,
  };
}

async function retain(tool, corpus, count) {
  const implementation = await adapter(tool);
  const sql = corpus.statements.map((item) => item.sql);
  if (global.gc) global.gc();
  const retained = Array.from({ length: count }, (_, index) => implementation.parse(sql[index % sql.length]));
  if (global.gc) global.gc();
  process.stdout.write(`${JSON.stringify({
    schema: "squonk.publication-adapter/1", ecosystem: "node", tool,
    version: implementation.version, mode: "retain", corpus_sha256: corpus.sha256,
    retained_documents: retained.length, pid: process.pid, ready: true,
  })}\n`);
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  await rl.question("");
  rl.close();
  if (retained.length !== count) throw new Error("retained roots did not remain live");
}

const options = args();
const corpus = await loadCorpus(options.corpus);
if (options.mode === "qualify") console.log(JSON.stringify(await qualify(options.tool, corpus)));
else if (options.mode === "throughput") console.log(JSON.stringify(await throughput(options.tool, corpus)));
else if (options.mode === "cold") {
  const implementation = await adapter(options.tool);
  const result = implementation.parse(corpus.statements[0].sql);
  console.log(JSON.stringify({
    schema: "squonk.publication-adapter/1", ecosystem: "node", tool: options.tool,
    version: implementation.version, mode: "cold", corpus_sha256: corpus.sha256,
    sink_type: typeof result,
  }));
}
else await retain(options.tool, corpus, options.count);
