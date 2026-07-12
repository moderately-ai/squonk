# squonk

An extensible, fast, multi-dialect SQL tokenizer and parser for Rust that hands back an owned, `'static` syntax tree you can inspect, rewrite, and render back to SQL.

## Quickstart

```sh
cargo add squonk
```

```rust
use squonk::parse;

// Parse: ANSI SQL becomes an owned, `'static` syntax tree.
let parsed = parse("select 1 +  2").expect("well-formed SQL parses");

// Inspect: statements come back in source order.
assert_eq!(parsed.statements().len(), 1);

// Render: canonical output normalizes keyword case and spacing.
assert_eq!(parsed.to_string(), "SELECT 1 + 2");
```

The tree is fully owned (it moves the source in, so it never borrows your input) and walks the whole statement: match a `Statement` to inspect it, edit it in place with the generated `Visit` / `VisitMut` walks, or render it back out — canonical, fully parenthesized, or PII-redacted. The [crate docs][docs] carry worked parse, inspect, render, rewrite, transpile, and error-recovery examples.

## Dialects

Several dialects ship (`BuiltinDialect::ALL` is the selectable list). `Ansi` — the SQL-standard baseline that `parse` defaults to — is always compiled; every other dialect is an opt-in cargo feature, so a default build stays lean:

```toml
[dependencies]
squonk = { version = "1.0", features = ["postgres"] }
```

`postgres`, `mysql`, `sqlite`, `duckdb`, the permissive `lenient` "parse-anything" union, and the conservative `bigquery`, `clickhouse`, `databricks`, `hive`, `mssql`, `redshift`, and `snowflake` presets each gate one dialect; `full` turns on all of them. Select one at runtime with `parse_with` (or `parse_with_builtin`), render *for* a target dialect with the `Renderer`, or `transpile` between two in one call. Each engine-backed dialect is held to its real engine by a differential accept/reject oracle, so its surface is engine-verified rather than merely self-consistent; the conservative presets are ANSI-derived and ship without an oracle (excluded from the oracle conformance sets, they reject unmodelled syntax cleanly). The per-dialect 100%-conformance programmes are ongoing, so this is verified breadth, not a finished spec-level audit.

## Status

Stable (1.0) — the public API is frozen and covered by the SemVer contract in [docs/stable-api.md](https://github.com/moderately-ai/squonk/blob/main/docs/stable-api.md); no breaking change lands without a major bump. The engine-backed dialects (ANSI, PostgreSQL, MySQL, SQLite, DuckDB) are the strongest surface, each held to its real engine by a differential oracle; the conservative presets reject unmodelled syntax cleanly. The per-dialect conformance work continues additively under `1.x`, with design rationale recorded in `docs/adr/`.

`squonk` parses roughly 3x faster than the prior art it learns from (`apache/datafusion-sqlparser-rs`) while building an AST many times lighter in heap; full numbers, methodology, and fairness caveats live in the repository's benchmark sources.

## Documentation

- API reference and worked examples: [docs.rs/squonk][docs]
- Internals and architecture: [the workspace README][repo]

The AST itself lives in the near-dependency-free [`squonk-ast`][ast] crate (its only non-optional dependency is the `thin-vec` micro-leaf; serde is opt-in) and is re-exported here as `squonk::ast`, so most users only need this crate.

## License

Licensed under the [MIT License](https://github.com/moderately-ai/squonk/blob/main/LICENSE).

[docs]: https://docs.rs/squonk
[ast]: https://docs.rs/squonk-ast
[repo]: https://github.com/moderately-ai/squonk
