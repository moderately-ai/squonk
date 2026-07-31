// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

// The one dialect list the playground renders its <select> options from AND the
// client script derives its DialectName type from, so the UI cannot drift from
// what the script sends to the engine. Must stay a subset of the names the
// `squonk` browser entrypoint compiles in (it currently bundles exactly these).
export const dialects = [
  "ansi",
  "postgres",
  "mysql",
  "sqlite",
  "duckdb",
  "bigquery",
  "snowflake",
  "redshift",
  "databricks",
  "clickhouse",
  "hive",
  "mssql",
  "quiltdb",
  "lenient",
] as const;

export type DialectName = (typeof dialects)[number];
