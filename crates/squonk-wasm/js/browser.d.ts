// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

export type * from "./runtime.js";
export type * from "../js/ast.generated.js";
export { Diagnostic, Document, Ident, Node, ObjectName, RecoveredDocument, SqlParseError } from "./runtime.js";
export declare const createSquonk: (createOptions?: import("./runtime.js").CreateSquonkOptions) => Promise<import("./runtime.js").SquonkApi<"ansi" | "postgres" | "mysql" | "sqlite" | "duckdb" | "quiltdb" | "bigquery" | "hive" | "clickhouse" | "databricks" | "mssql" | "snowflake" | "redshift" | "lenient", "ansi">>;
