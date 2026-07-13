// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

export const packages = [
  { label: "ansi", packageName: "@squonk-sql/ansi", entry: "ansi", features: [], defaultDialect: "ansi", supportedDialects: ["ansi"] },
  { label: "postgres", packageName: "@squonk-sql/postgres", entry: "postgres", features: ["dialect-postgres"], defaultDialect: "postgres", supportedDialects: ["ansi", "postgres"] },
  { label: "mysql", packageName: "@squonk-sql/mysql", entry: "mysql", features: ["dialect-mysql"], defaultDialect: "mysql", supportedDialects: ["ansi", "mysql"] },
  { label: "sqlite", packageName: "@squonk-sql/sqlite", entry: "sqlite", features: ["dialect-sqlite"], defaultDialect: "sqlite", supportedDialects: ["ansi", "sqlite"] },
  { label: "duckdb", packageName: "@squonk-sql/duckdb", entry: "duckdb", features: ["dialect-duckdb"], defaultDialect: "duckdb", supportedDialects: ["ansi", "duckdb"] },
  { label: "lenient", packageName: "@squonk-sql/lenient", entry: "lenient", features: ["dialect-lenient"], defaultDialect: "lenient", supportedDialects: ["ansi", "lenient"] },
  { label: "all", packageName: "squonk", entry: "index", features: ["dialects-full"], defaultDialect: "ansi", supportedDialects: ["ansi", "postgres", "mysql", "sqlite", "duckdb", "bigquery", "hive", "clickhouse", "databricks", "mssql", "snowflake", "redshift", "lenient"] },
];
