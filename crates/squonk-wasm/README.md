# Squonk for TypeScript and JavaScript

Typed native and WebAssembly bindings for Squonk's SQL parser, renderer, formatter, tokenizer, and transpiler. Packages require Node 22 or newer for their synchronous Node entrypoint and include TypeScript declarations.

## Choose a package

Install only the dialect surface you need:

```sh
npm install @squonk-sql/postgres
```

| Package | Default dialect | Accepted dialects |
|---|---|---|
| `@squonk-sql/ansi` | ANSI | ANSI |
| `@squonk-sql/postgres` | PostgreSQL | ANSI, PostgreSQL |
| `@squonk-sql/mysql` | MySQL | ANSI, MySQL |
| `@squonk-sql/sqlite` | SQLite | ANSI, SQLite |
| `@squonk-sql/duckdb` | DuckDB | ANSI, DuckDB |
| `@squonk-sql/lenient` | Lenient | ANSI, Lenient |
| `squonk` | ANSI | Every built-in dialect |

The `squonk` umbrella additionally includes BigQuery, Hive, ClickHouse, Databricks, MSSQL, Snowflake, and Redshift. Every package has the complete document API; there are no `full` variants.

## Node and Bun

Node and Bun select a prebuilt Node-API engine for the current platform. Unsupported platforms and Node's `--no-addons` mode fall back to colocated WASM. Both paths are synchronous and consumers never call `init()`:

```ts
import { format, parse, parseRecovering, tokenize } from "@squonk-sql/postgres";

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

Use `runtimeInfo()` when diagnostics or telemetry need to distinguish `{ backend: "native" }` from `{ backend: "wasm" }`. Application behavior must not depend on which compatible backend was selected.

## Deno and edge runtimes

Bare imports in Deno use a permissionless WebAssembly-module entrypoint; `--allow-read` and `--allow-ffi` are not required. Cloudflare Wrangler selects the `workerd` entrypoint, and edge-light bundlers receive the corresponding static WASM-module entrypoint. All retain the synchronous parse API:

```ts
import { parse } from "npm:@squonk-sql/postgres";

console.log(parse("select $1").toSQL());
```

Explicit runtime entrypoints are available as `/node`, `/deno`, `/workerd`, `/edge-light`, `/browser`, and `/wasm`. Normal consumers should use the bare package import and let conditional exports select the backend.

## Browser

Browsers load WASM asynchronously through the explicit `browser` entrypoint:

```ts
import { createSquonk } from "@squonk-sql/postgres/browser";

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
- `tokenize`, `supportedDialects`, `version`, `schemaVersion`, `runtimeInfo`
- `isDialectName`, `assertDialectName`, `canonicalDialectName`
- `Document`, `RecoveredDocument`, `Node`, `Ident`, `ObjectName`, `Diagnostic`, and `SqlParseError`

Wrapper instances are created by parse operations and remain bound to their WASM
runtime; their constructors are not a supported public factory. Parse results stay in
WASM through source metadata access and parse → render. Reading `raw`, calling
`toJSON()`, or using `parseJson` explicitly materializes the tree into JavaScript;
that materialized view is live, so edits are observed by subsequent rendering. Node
fragment rendering supports complete statements, queries, expressions, and data
types; context-dependent nodes report `unsupported_node_render`.

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
npm run smoke:runtimes
npm run smoke:workerd
```

`npm run build` creates browser/edge and Deno WASM artifacts, builds the local Node-API addon, compiles the shared TypeScript facade, and stages exact publish trees under `dist/npm/`. The release workflow builds eight platform addons independently before staging. The checked-in build manifest remains private; publishing is performed only from verified staged artifacts by the protected release workflow.
