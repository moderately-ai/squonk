// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { initSync } from "../pkg-all/squonk_wasm.js";
import * as wasm from "../pkg-all/squonk_wasm.js";
import { createNodeSquonk } from "./node.js";

const api = createNodeSquonk(
  initSync,
  wasm,
  new URL("../pkg-all/squonk_wasm_bg.wasm", import.meta.url),
  {
    defaultDialect: "ansi",
    supportedDialects: [
      "ansi", "postgres", "mysql", "sqlite", "duckdb", "quiltdb", "bigquery", "hive",
      "clickhouse", "databricks", "mssql", "snowflake", "redshift", "lenient",
    ] as const,
  },
);

export type * from "./runtime.js";
export type * from "../js/ast.generated.js";
export { Diagnostic, Document, Ident, Node, ObjectName, RecoveredDocument, SqlParseError } from "./runtime.js";
/** Return true when a dynamic string is a dialect accepted by this entrypoint. */
export const isDialectName = api.isDialectName;
/** Throw `SqlParseError` unless a dynamic string is a dialect accepted by this entrypoint. */
export const assertDialectName = api.assertDialectName;
/** Canonicalize a dynamic dialect string for this entrypoint, or return null. */
export const canonicalDialectName = api.canonicalDialectName;
/** Parse SQL and throw on the first parser diagnostic. */
export const parse = api.parse;
/** Parse SQL and return the raw JSON payload. */
export const parseJson = api.parseJson;
/** Parse SQL with an explicit recursion-depth limit. */
export const parseWithLimit = api.parseWithLimit;
/** Parse SQL while recovering statement-level syntax errors. */
export const parseRecovering = api.parseRecovering;
/** Recovering parse that returns the raw JSON payload. */
export const parseRecoveringJson = api.parseRecoveringJson;
/** Return dialect metadata compiled into this entrypoint. */
export const supportedDialects = api.supportedDialects;
/** Tokenize SQL under a supported dialect. */
export const tokenize = api.tokenize;
/** Render a SQL string or parsed document. */
export const render = api.render;
/** Render a SQL string or parsed document in redaction mode. */
export const redact = api.redact;
/** Pretty-print SQL. */
export const format = api.format;
/** Parse SQL under one dialect and render it under another. */
export const transpile = api.transpile;
/** Return the crate/package version string. */
export const version = api.version;
/** Return the serialized AST wire-schema version. */
export const schemaVersion = api.schemaVersion;
/** Describe the selected native or WebAssembly runtime backend. */
export const runtimeInfo = api.runtimeInfo;
