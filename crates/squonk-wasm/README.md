# Squonk for TypeScript and JavaScript

Typed WebAssembly bindings for Squonk's SQL parser, renderer, formatter, tokenizer, and transpiler. Packages are ESM-only, require Node 22 or newer for their synchronous Node entrypoint, and include TypeScript declarations.

## Choose a package

Install only the dialect surface you need:

```sh
npm install @squonk/postgres
```

| Package | Default dialect | Accepted dialects |
|---|---|---|
| `@squonk/ansi` | ANSI | ANSI |
| `@squonk/postgres` | PostgreSQL | ANSI, PostgreSQL |
| `@squonk/mysql` | MySQL | ANSI, MySQL |
| `@squonk/sqlite` | SQLite | ANSI, SQLite |
| `@squonk/duckdb` | DuckDB | ANSI, DuckDB |
| `@squonk/lenient` | Lenient | ANSI, Lenient |
| `squonk` | ANSI | Every built-in dialect |

The `squonk` umbrella additionally includes BigQuery, Hive, ClickHouse, Databricks, MSSQL, Snowflake, and Redshift. Every package has the complete document API; there are no `full` variants.

## Node

Node initializes its colocated WASM synchronously when the module loads. Consumers do not call `init()`:

```ts
import { format, parse, parseRecovering, tokenize } from "@squonk/postgres";

const document = parse("select $1");
document.dialect; // typed as "postgres"
console.log(document.toSQL());

const expression = [...parse("select a + 1").findAll("BinaryOp")][0];
if (expression.isRenderable) console.log(expression.toSQL());

const recovered = parseRecovering("select 1; from broken; select 2");
console.log(recovered.errors);
console.log(format("select a,b from t"));
console.log(tokenize("select $1").tokens);
```

A focused package defaults to its named dialect. Pass `{ dialect: "ansi" }` when the ANSI baseline is desired.

## Browser

Browsers load WASM asynchronously through the explicit `browser` entrypoint:

```ts
import { createSquonk } from "@squonk/postgres/browser";

const sql = await createSquonk();
const document = sql.parse("select $1");
console.log(document.toSQL());
```

The default resolves the package's colocated WASM URL. Custom hosting, byte loading, and precompiled modules are supported without exposing an initialization lifecycle on the loaded API:

```ts
const sql = await createSquonk({ wasm: new URL("/assets/postgres.wasm", location.href) });
```

Concurrent calls share one initialization promise. A failed load can be retried; after success, subsequent calls return the same runtime and the first successful WASM source wins.

## Typed API

The API includes:

- `parse`, `parseJson`, `parseWithLimit`
- `parseRecovering`, `parseRecoveringJson`
- `render`, `redact`, `format`, `transpile`
- `tokenize`, `supportedDialects`, `version`
- `isDialectName`, `assertDialectName`, `canonicalDialectName`
- `Document`, `RecoveredDocument`, `Node`, `Ident`, `ObjectName`, `Diagnostic`, and `SqlParseError`

Wrapper instances are created by parse operations and remain bound to their WASM
runtime; their constructors are not a supported public factory. `raw` and `toJSON()`
are live mutable views, so edits are observed by subsequent rendering. Node fragment
rendering supports complete statements, queries, expressions, and data types;
context-dependent nodes report `unsupported_node_render`.

Dialect aliases infer canonical result types:

```ts
import { parse } from "squonk";

parse("select 1", { dialect: "bq" }).dialect; // "bigquery"
parse("select 1", { dialect: "sf" }).dialect; // "snowflake"
```

Plain dynamic strings must be validated before use:

```ts
import { isDialectName, parse } from "squonk";

if (isDialectName(configuredDialect)) {
  const document = parse(source, { dialect: configuredDialect });
}
```

Parse options are `dialect`, `recursionLimit`, `captureTrivia`, and `parseFloatAsDecimal`. Format options are `dialect`, `indentWidth`, `maxWidth`, and `keywordCase`.

## Development

From this directory:

```sh
npm install --ignore-scripts --no-package-lock
npm run build
npm run typecheck
npm run smoke:variants
npm run size:check:tarball
npm run smoke:install
```

`npm run build` creates seven optimized WASM artifacts, compiles the shared TypeScript facade, and stages exact publish trees under `dist/npm/`. The checked-in build manifest remains private; publishing is performed only from verified staged artifacts by the protected release workflow.
