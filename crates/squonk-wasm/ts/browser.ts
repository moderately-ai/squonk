// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import initWasm, * as wasm from "../pkg-all/squonk_wasm.js";
import { createBrowserSquonk } from "./runtime.js";

export type * from "./runtime.js";
export type * from "../js/ast.generated.js";
export { Diagnostic, Document, Ident, Node, ObjectName, RecoveredDocument, SqlParseError } from "./runtime.js";
export const createSquonk = createBrowserSquonk(
  initWasm, wasm, new URL("../pkg-all/squonk_wasm_bg.wasm", import.meta.url),
  { defaultDialect: "ansi", supportedDialects: [
    "ansi", "postgres", "mysql", "sqlite", "duckdb", "quiltdb", "bigquery", "hive", "clickhouse",
    "databricks", "mssql", "snowflake", "redshift", "lenient",
  ] as const },
);
