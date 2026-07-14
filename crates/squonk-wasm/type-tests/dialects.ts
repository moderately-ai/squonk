// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { expectTypeOf } from "expect-type";

import { parse as parseAnsi, runtimeInfo } from "../js/ansi.js";
import { createSquonk } from "../js/postgres-browser.js";
import * as postgresBrowser from "../js/postgres-browser.js";
import {
  isDialectName,
  parse as parsePostgres,
  schemaVersion,
  tokenize as tokenizePostgres,
  transpile as transpilePostgres,
  Document,
  type ParseResult,
} from "../js/postgres.js";
import { canonicalDialectName, parse as parseAll } from "../js/index.js";

expectTypeOf(parseAnsi("select 1").dialect).toEqualTypeOf<"ansi">();
expectTypeOf(runtimeInfo().backend).toEqualTypeOf<"native" | "wasm">();
expectTypeOf(runtimeInfo().host).toEqualTypeOf<
  "node" | "bun" | "deno" | "browser" | "workerd" | "edge-light" | "unknown"
>();
// @ts-expect-error ANSI package does not compile PostgreSQL.
parseAnsi("select $1", { dialect: "postgres" });

const postgres = parsePostgres("select $1");
expectTypeOf(postgres.statements[0]!.toSQL()).toEqualTypeOf<string>();
expectTypeOf(postgres.statements[0]!.isRenderable).toEqualTypeOf<boolean>();
expectTypeOf(schemaVersion()).toEqualTypeOf<number>();
expectTypeOf(postgres.dialect).toEqualTypeOf<"postgres">();
expectTypeOf(postgres).toEqualTypeOf<
  Document<ParseResult<"postgres">, "postgres", "ansi" | "postgres">
>();
expectTypeOf(parsePostgres("select 1", { dialect: "generic" }).dialect).toEqualTypeOf<"ansi">();
expectTypeOf(parsePostgres("select $1", { dialect: "pg" }).dialect).toEqualTypeOf<"postgres">();
// @ts-expect-error PostgreSQL package does not compile MySQL.
parsePostgres("select 1", { dialect: "mysql" });
// @ts-expect-error Documents are runtime-bound views created by parse().
new Document(postgres.raw);
// @ts-expect-error PostgreSQL package cannot tokenize as DuckDB.
tokenizePostgres("select 1", { dialect: "duckdb" });
// @ts-expect-error PostgreSQL package cannot transpile to SQLite.
transpilePostgres("select 1", { sourceDialect: "postgres", targetDialect: "sqlite" });

const dynamic = "PG" as string;
if (isDialectName(dynamic)) {
  expectTypeOf(parsePostgres("select 1", { dialect: dynamic }).dialect)
    .toEqualTypeOf<"ansi" | "postgres">();
}

expectTypeOf(parseAll("select 1").dialect).toEqualTypeOf<"ansi">();
expectTypeOf(parseAll("select 1", { dialect: "bq" }).dialect).toEqualTypeOf<"bigquery">();
expectTypeOf(parseAll("select 1", { dialect: "ch" }).dialect).toEqualTypeOf<"clickhouse">();
expectTypeOf(parseAll("select 1", { dialect: "dbx" }).dialect).toEqualTypeOf<"databricks">();
expectTypeOf(parseAll("select 1", { dialect: "sf" }).dialect).toEqualTypeOf<"snowflake">();
expectTypeOf(parseAll("select 1", { dialect: "amazonredshift" }).dialect).toEqualTypeOf<"redshift">();
expectTypeOf(canonicalDialectName("SQLSERVER")).toEqualTypeOf<
  | "ansi" | "postgres" | "mysql" | "sqlite" | "duckdb" | "quiltdb" | "bigquery" | "hive"
  | "clickhouse" | "databricks" | "mssql" | "snowflake" | "redshift" | "lenient" | null
>();

async function browserTypes(): Promise<void> {
  const sql = await createSquonk();
  expectTypeOf(sql.parse("select $1").dialect).toEqualTypeOf<"postgres">();
  expectTypeOf(sql.parse).toEqualTypeOf<typeof parsePostgres>();
  // @ts-expect-error initialization is not part of the loaded API.
  sql.init();
  // @ts-expect-error the browser module does not expose synchronous parse.
  postgresBrowser.parse;
  // @ts-expect-error the browser module does not expose manual initialization.
  postgresBrowser.init;
}
void browserTypes;
