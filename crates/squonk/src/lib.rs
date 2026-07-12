// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

// docs.rs feature-gate banners (ticket docs-rs-feature-gate-banners): turn on rustdoc's `doc_cfg` for the nightly docs.rs build only — gated by the `docsrs` cfg docs.rs sets — so feature-gated items render an "Available on crate feature X" banner; auto_cfg is on by default at crate level once `doc_cfg` is enabled (the old `doc_auto_cfg` gate was merged into `doc_cfg` and removed in Rust 1.92), and the whole thing is inert on the pinned stable toolchain (the cfg is never set there).
#![cfg_attr(docsrs, feature(doc_cfg))]
// Crate-level (not the shared `[workspace.lints]` table): `missing_docs` is opted into
// per crate. The two published crates — this one and `squonk-ast` — each deny it, but the
// bindings and dev tooling (`squonk-python`, `squonk-wasm`, `squonk-sourcegen`) carry
// undocumented public surface, so a workspace-wide deny would break them.
#![deny(missing_docs)]
//! `squonk` — an extensible, fast, multi-dialect SQL tokenizer and parser.
//!
//! The AST lives in the `squonk-ast` crate and is re-exported here as [`ast`],
//! so most users only need to depend on this crate. This crate adds the zero-copy
//! [`tokenizer`], the identifier [`interner`], structured [`error`]s, the
//! monomorphized [`Parser`] engine (recursive descent + a Pratt expression core),
//! the [`Dialect`] system, and the owned [`Parsed`] root. Several dialects ship
//! (`BuiltinDialect::ALL` is the selectable list) — `Ansi`, the always-compiled
//! SQL-standard baseline `parse` defaults to, plus the feature-gated presets
//! `Postgres`, `MySql`, `Sqlite`, `DuckDb`, `BigQuery`, `Hive`, `ClickHouse`,
//! `Databricks`, `Mssql`, `Snowflake`, `Redshift`, and the permissive `Lenient`,
//! each carrying a release-contract support tier (`docs/support-tiers.md`);
//! `full` turns them all on — over a query surface spanning the SELECT
//! family (CTEs, set operations, joins, window functions, and expression forms such
//! as `IS [NOT] DISTINCT FROM`) plus DDL, DML, DCL, TCL, and utility statements
//! (including `TRUNCATE` and `COMMENT ON`). Parse via [`parse`], [`parse_with`], or
//! [`Parser`].
//!
//! # Design records
//!
//! The architectural decisions behind these choices — owned tree, interned
//! identifiers, dialect-as-data, render modes, and the rest — are recorded as ADRs in
//! the repository's [`docs/adr`](https://github.com/moderately-ai/squonk/tree/main/docs/adr)
//! directory.
//!
//! # Entry points
//!
//! The rule behind the parse entry points: a caller-tunable knob that leaves the return type alone is a field on [`ParseOptions`], reached through the combining `_with_options` form — [`parse`]/[`parse_with`]/[`parse_with_rc`]/[`parse_with_options`] all return a [`Parsed`] tree, and [`parse_with_trivia`] is simply documented sugar over [`parse_with_options`] for one common combination. A genuinely different result shape earns its own verb instead: [`Recovered`] (partial AST plus diagnostics, from [`parse_recovering`]/[`parse_recovering_with_options`]) and the streaming [`Statements`] iterator (from [`statements`]/[`statements_with_options`]) are not options on [`ParseOptions`], because forcing either behind a flag would need an enum return — worse than a second name. A new knob follows the first path unless it genuinely changes what the function returns.
//!
//! # Examples
//!
//! ## Parse
//!
//! [`parse`] defaults to the [`Ansi`](dialect::Ansi) dialect;
//! [`parse_with`] selects another, such as `Postgres`. Both
//! return an owned, `'static` [`Parsed`] tree: the source is moved into
//! the root, so the tree never borrows the input string.
//!
//! ```
//! use squonk::dialect::Ansi;
//! use squonk::{parse, parse_with};
//!
//! // ANSI by default.
//! let parsed = parse("SELECT 1").expect("a well-formed query parses");
//! assert_eq!(parsed.statements().len(), 1);
//!
//! // ...or name a dialect explicitly. Non-default dialects (e.g. `Postgres`)
//! // are available under their cargo feature.
//! let parsed = parse_with("SELECT 1", Ansi).expect("parses under ANSI");
//! assert_eq!(parsed.statements().len(), 1);
//! ```
//!
//! The default [`parse_with`] root is `Parsed<Arc<str>>` — `Send + Sync`, so it can
//! cross threads. The tree's ownership tiers let a caller trade that reach for a
//! cheaper one: [`parse_with_rc`] roots the tree in a non-atomic `Rc<str>` for
//! single-thread use, and [`Parsed::into_statements`] drops the source and resolver
//! entirely for callers that only inspect statement *shape*.
//!
//! ```
//! use squonk::dialect::Ansi;
//! use squonk::{parse_with, parse_with_rc};
//!
//! // Default tier: an `Arc<str>` root, `Send + Sync` (can cross threads).
//! let arc = parse_with("SELECT 1", Ansi).expect("Arc<str> root");
//! assert_eq!(arc.statements().len(), 1);
//!
//! // Single-thread tier: a non-atomic `Rc<str>` root, the cheapest refcount.
//! let rc = parse_with_rc("SELECT 1", Ansi).expect("Rc<str> root");
//! assert_eq!(rc.statements().len(), 1);
//!
//! // Structure-only tier: drop the source and resolver, keep the statements.
//! let statements = arc.into_statements();
//! assert_eq!(statements.len(), 1);
//! ```
//!
//! ## Inspect
//!
//! [`Parsed::statements`] yields the statements in source order. Match a node to
//! walk into it; pair any [`Symbol`](ast::Symbol) it holds with the tree's
//! [`resolver`](Parsed::resolver) to recover the interned identifier text.
//!
//! ```
//! use squonk::ast::Resolver as _;
//! use squonk::ast::{Expr, SelectItem, SetExpr, Statement};
//! use squonk::parse;
//!
//! let parsed = parse("SELECT name FROM users").expect("parses");
//! let Statement::Query { query, .. } = &parsed.statements()[0] else {
//!     panic!("expected a query statement");
//! };
//! let SetExpr::Select { select, .. } = &query.body else {
//!     panic!("expected a SELECT body");
//! };
//! let SelectItem::Expr { expr: Expr::Column { name, .. }, .. } = &select.projection[0] else {
//!     panic!("expected a column projection");
//! };
//! // A node stores interned `Symbol`s; the resolver gives them their text back.
//! assert_eq!(parsed.resolver().resolve(name.0[0].sym), "name");
//! ```
//!
//! ## Render canonical SQL
//!
//! The simplest path is the [`Display`](std::fmt::Display) impl on the [`Parsed`]
//! root, which the root can offer because it owns the source and resolver a render
//! needs. To render a *detached* node — one not behind a `Parsed` — pair
//! it with a [`RenderCtx`](ast::render::RenderCtx) via
//! [`displayed`](ast::render::RenderExt::displayed).
//!
//! ```
//! use squonk::ast::render::{RenderConfig, RenderCtx, RenderExt};
//! use squonk::parse;
//!
//! // Canonical rendering normalizes keyword case and spacing, round-tripping SQL.
//! let parsed = parse("select 1 +  2").expect("parses");
//! assert_eq!(parsed.to_string(), "SELECT 1 + 2");
//!
//! // The same text for one statement, threaded explicitly through a RenderCtx.
//! let config = RenderConfig::default();
//! let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
//! assert_eq!(parsed.statements()[0].displayed(&ctx).to_string(), "SELECT 1 + 2");
//! ```
//!
//! ## Pre-sized render
//!
//! [`Parsed`]'s `Display` is the convenient path, but `.to_string()` starts from an
//! empty buffer and pays the reallocation-doubling chain up to the output size
//! (a render-perf audit measured 7 reallocations for a 270-byte statement).
//! On a render-dominant path — transpile-many, or rewrite-then-render — prefer
//! [`Parsed::to_sql`], which reserves `source().len()` up front and renders in one
//! allocation, or [`Parsed::render_into`] with a buffer reused across trees. All
//! three produce byte-identical canonical SQL.
//!
//! ```
//! use squonk::parse;
//!
//! let parsed = parse("select 1, 2, 3").expect("parses");
//!
//! // Pre-sized: reserves `source().len()` and renders in one allocation.
//! assert_eq!(parsed.to_sql(), "SELECT 1, 2, 3");
//!
//! // Or render into a caller-owned buffer, reusable across many trees.
//! let mut buf = String::new();
//! parsed.render_into(&mut buf).expect("rendering into a String is infallible");
//! assert_eq!(buf, "SELECT 1, 2, 3");
//! ```
//!
//! ## Rewriting the AST (`Visit` / `VisitMut`)
//!
//! The retained, rewritable AST is the crate's differentiator. The
//! generated [`Visit`](ast::generated::visit::Visit) /
//! [`VisitMut`](ast::generated::visit::VisitMut) traits expose a `visit_*` hook per
//! node type over the whole tree; override the few you care about and call the
//! matching `walk_*` to recurse. The [`Parsed`] root is shared, so clone its
//! statements to rewrite them ([`Statement`](ast::Statement) is `Clone`) while the
//! root keeps the source and resolver a render needs. A rewrite may only reuse
//! [`Symbol`](ast::Symbol)s already interned in the tree — the root's resolver is
//! frozen, so brand-new identifier text has no symbol to point at (the graft-safety
//! rule). Fuller walkthroughs live in `examples/`: `rewrite_qualify`,
//! `rewrite_redact`, and `analyze_tables` (run e.g. `cargo run --example
//! rewrite_qualify`).
//!
//! ```
//! use squonk::ast::generated::visit::{VisitMut, walk_expr_mut};
//! use squonk::ast::render::{RenderConfig, RenderCtx, RenderExt as _};
//! use squonk::ast::Expr;
//! use squonk::parse;
//!
//! // Strip the qualifier from every column (`t.id` -> `id`): a mutable walk reusing
//! // the tree's own interned symbols (the frozen resolver mints no new ones).
//! struct Unqualify;
//! impl VisitMut for Unqualify {
//!     fn visit_expr_mut(&mut self, node: &mut Expr) {
//!         if let Expr::Column { name, .. } = node {
//!             while name.0.len() > 1 {
//!                 name.0.remove(0);
//!             }
//!         }
//!         walk_expr_mut(self, node);
//!     }
//! }
//!
//! let parsed = parse("SELECT t.id, t.name FROM t").expect("parses");
//! // Clone the statements to rewrite them while the root keeps source + resolver.
//! let mut statements = parsed.statements().to_vec();
//! for statement in &mut statements {
//!     Unqualify.visit_statement_mut(statement);
//! }
//! let config = RenderConfig::default();
//! let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
//! assert_eq!(statements[0].displayed(&ctx).to_string(), "SELECT id, name FROM t");
//! ```
//!
//! ## Debug a detached node
//!
//! Prefer the canonical path above whenever you can: [`Parsed`]'s `Display` and
//! [`displayed`](ast::render::RenderExt::displayed) render exact SQL because they
//! travel with the matched source and resolver, so they cannot mismatch. Reach for
//! [`debug_sql`](ast::render::RenderExt::debug_sql) only to debug a *detached* or
//! *synthesized* node — one lifted out of its [`Parsed`] root, whose symbols may not
//! belong to the resolver you have on hand.
//!
//! It takes the resolver as an **explicit argument** — never a hidden thread-local
//! or global — so it cannot *silently* render with the wrong resolver; an unknown
//! symbol becomes a visible `<unresolved>` placeholder instead of panicking; and
//! literals are spelled by kind (debug never slices source), so it cannot emit
//! misleading bytes from a mismatched context.
//!
//! ```
//! use squonk::ast::render::RenderExt;
//! use squonk::interner::Interner;
//! use squonk::parse;
//!
//! let parsed = parse("SELECT amount FROM ledger").expect("parses");
//! let stmt = &parsed.statements()[0];
//!
//! // With the node's own resolver, identifiers resolve and the shape is exact.
//! assert_eq!(stmt.debug_sql(parsed.resolver()).to_string(), "SELECT amount FROM ledger");
//!
//! // A foreign resolver (here an empty one) knows none of these symbols, so debug
//! // rendering marks each with a placeholder rather than panicking — where the
//! // canonical path would instead panic on the mismatched resolver.
//! let foreign = Interner::new().freeze();
//! assert_eq!(
//!     stmt.debug_sql(&foreign).to_string(),
//!     "SELECT <unresolved> FROM <unresolved>",
//! );
//! ```
//!
//! ## Redacted render
//!
//! [`RenderMode::Redacted`](ast::render::RenderMode::Redacted) masks identifier and
//! literal *content* — `id` for every identifier, `?` for every literal — while
//! keeping query *shape*, yielding a stable, PII-free fingerprint.
//! Set it on a [`RenderConfig`](ast::render::RenderConfig).
//!
//! ```
//! use squonk::ast::render::{RenderConfig, RenderCtx, RenderExt, RenderMode};
//! use squonk::parse;
//!
//! let parsed = parse("SELECT name, 42 FROM users WHERE id = 7").expect("parses");
//! let config = RenderConfig { mode: RenderMode::Redacted, ..RenderConfig::default() };
//! let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
//! assert_eq!(
//!     parsed.statements()[0].displayed(&ctx).to_string(),
//!     "SELECT id, ? FROM id WHERE id = ?",
//! );
//! ```
//!
//! ## Target-dialect render
//!
//! Tier-1 rendering above is infallible for the neutral SQL surface. The Tier-2
//! [`Renderer`](render::Renderer) renders *for a specific dialect target*: it
//! validates that the target can express each statement before spelling it, prefers
//! the target's type spellings, and *rejects* — rather than mis-renders — a
//! construct the target lacks.
//!
//! ```
//! use squonk::dialect::Ansi;
//! use squonk::parse;
//! use squonk::render::Renderer;
//!
//! // The ANSI target spells the standard type name for a CAST.
//! let parsed = parse("SELECT CAST(a AS VARCHAR(5))").expect("parses");
//! assert_eq!(
//!     Renderer::new(Ansi).render_parsed(&parsed).expect("ANSI can spell this"),
//!     "SELECT CAST(a AS CHARACTER VARYING(5))",
//! );
//!
//! // A PostgreSQL `$1` placeholder has no ANSI spelling, so the ANSI target rejects
//! // it with a span-carrying diagnostic instead of emitting invalid SQL. (Parsing
//! // PostgreSQL requires the `postgres` feature.)
//! # #[cfg(feature = "postgres")] {
//! # use squonk::dialect::Postgres;
//! # use squonk::parse_with;
//! # use squonk::render::RenderErrorKind;
//! let pg = parse_with("SELECT $1", Postgres).expect("parses under PostgreSQL");
//! let error = Renderer::new(Ansi).render_parsed(&pg).expect_err("ANSI has no $n");
//! assert_eq!(error.kind(), RenderErrorKind::Unsupported);
//! assert!(error.span().is_some());
//! # }
//! ```
//!
//! ## Transpile
//!
//! [`transpile`] packages the two-step parse-then-target-render into one call:
//! parse `sql` under a source dialect, render it for a target. Like the
//! [`Renderer`](render::Renderer) it wraps, it is *syntactic* transpilation with
//! rejection — a construct the target cannot spell fails with a
//! span-carrying [`TranspileError`], never invalid SQL — not sqlglot-style
//! semantic rewriting. It takes no options by design; a caller wanting a recursion
//! limit, trivia, a redacted mode, or a [`Renderer`](render::Renderer) reused
//! across inputs composes [`parse_with`] and [`Renderer`](render::Renderer)
//! directly.
//!
//! ```
//! # #[cfg(feature = "postgres")] {
//! use squonk::dialect::{Ansi, Postgres};
//! use squonk::render::RenderErrorKind;
//! use squonk::{transpile, TranspileError};
//!
//! // A PostgreSQL cast transpiles to ANSI, which prefers the standard type name.
//! let ansi = transpile("SELECT CAST(a AS VARCHAR(5))", Postgres, Ansi)
//!     .expect("ANSI can spell this cast");
//! assert_eq!(ansi, "SELECT CAST(a AS CHARACTER VARYING(5))");
//!
//! // A PostgreSQL `$1` placeholder has no ANSI spelling, so transpilation rejects
//! // it with a span-carrying diagnostic rather than emitting invalid SQL.
//! let error = transpile("SELECT $1", Postgres, Ansi).expect_err("ANSI has no $n");
//! let TranspileError::Render(rejection) = error else {
//!     panic!("expected a render rejection, not a parse error");
//! };
//! assert_eq!(rejection.kind(), RenderErrorKind::Unsupported);
//! assert!(rejection.span().is_some());
//! # }
//! ```
//!
//! ## Custom dialects
//!
//! A [`Dialect`] is *data*: its [`features`](Dialect::features) returns a
//! const [`FeatureSet`](ast::dialect::FeatureSet) the parser reads field by field.
//! Build one from a preset plus a [`FeatureDelta`](ast::dialect::FeatureDelta):
//! [`FeatureSet::with`](ast::dialect::FeatureSet::with) applies the delta unchecked
//! (the fast path the presets use), while
//! [`FeatureSet::try_with`](ast::dialect::FeatureSet::try_with) returns a
//! [`LexicalConflict`](ast::dialect::LexicalConflict) if the delta makes two features
//! fight over the same tokenizer trigger (e.g. `$1` as both a money literal and a
//! positional parameter). [`is_lexically_consistent`](ast::dialect::FeatureSet::is_lexically_consistent)
//! is the same check as a bool, usable in a `const` assertion.
//!
//! ```
//! use squonk::ast::NoExt;
//! use squonk::ast::dialect::{FeatureDelta, FeatureSet, ParameterSyntax};
//! use squonk::{Dialect, parse_with};
//!
//! // ANSI, plus anonymous `?` parameter placeholders.
//! const ANSI_WITH_PARAMS: FeatureSet = FeatureSet::ANSI.with(
//!     FeatureDelta::EMPTY.parameters(ParameterSyntax {
//!         anonymous_question: true,
//!         ..ParameterSyntax::ANSI
//!     }),
//! );
//! // The delta claims no trigger another feature already owns, so the set is
//! // consistent — checkable at compile time.
//! const _: () = assert!(ANSI_WITH_PARAMS.is_lexically_consistent());
//!
//! #[derive(Clone, Copy)]
//! struct AnsiWithParams;
//! impl Dialect for AnsiWithParams {
//!     type Ext = NoExt;
//!     fn features(&self) -> &FeatureSet {
//!         &ANSI_WITH_PARAMS
//!     }
//! }
//!
//! // `?` now parses where stock ANSI would reject it.
//! let parsed = parse_with("SELECT ?", AnsiWithParams).expect("the custom dialect parses `?`");
//! assert_eq!(parsed.statements().len(), 1);
//! ```
//!
//! ## Diagnostics
//!
//! A failed parse returns a [`ParseError`](error::ParseError): a byte
//! [`Span`](ast::Span) plus what the parser *expected* and *found*; an
//! input that ends mid-construct reports [`Found::EndOfInput`](error::Found::EndOfInput).
//! Spans are byte offsets by design; recover line/column from the source
//! through an [`ast::LineIndex`], or via [`Parsed::span_line_col`] when a tree is in
//! hand.
//!
//! ```
//! use squonk::ast::LineIndex;
//! use squonk::parse;
//!
//! // `FROM` cannot begin a statement, so the parse fails on the second line.
//! let src = "SELECT 1;\nFROM t";
//! let error = parse(src).expect_err("FROM is not a statement");
//!
//! assert_eq!(error.span.start(), 10);
//! assert!(error.to_string().contains("found FROM"), "{error}");
//!
//! // Map the error's byte span to a zero-based (line, column) for display.
//! let (line, column) = LineIndex::from_str(src).lookup(error.span.start());
//! assert_eq!((line, column), (1, 0)); // second line, first column
//! ```
//!
//! ## Recovering parse
//!
//! The default parse is fail-fast — the first error short-circuits. To
//! collect *every* diagnostic in a multi-statement script (compiler-style "all errors
//! in the file"), use [`parse_recovering`]: it records each broken statement's error,
//! resynchronizes at the next `;`, and resumes. The returned [`Recovered`] carries
//! both the well-formed statements — as ordinary AST — and the
//! [`errors`](Recovered::errors) for the broken ones (no error nodes enter
//! the tree).
//!
//! ```
//! use squonk::dialect::Ansi;
//! use squonk::parse_recovering;
//!
//! // The middle statement is malformed; the outer two are well-formed.
//! let recovered =
//!     parse_recovering("SELECT 1; SELECT FROM t; SELECT 2", Ansi).expect("recovers");
//! assert!(recovered.has_errors());
//! assert_eq!(recovered.errors().len(), 1);
//! // The two good statements are still parsed and usable.
//! assert_eq!(recovered.statements().len(), 2);
//! ```
//!
//! ## Trivia
//!
//! Comments and whitespace are skipped at zero cost by default. To recover them — for
//! a formatter, linter, or doc-comment extractor — set
//! [`ParseOptions::with_trivia_capture`] (or the [`parse_with_trivia`] sugar). The
//! captured runs hang off the [`Parsed`] root, queryable by offset via
//! [`Parsed::trivia`], [`Parsed::trivia_in`], and [`Parsed::trivia_before`]; the
//! statements themselves stay trivia-free.
//!
//! ```
//! use squonk::dialect::Ansi;
//! use squonk::parse_with_trivia;
//!
//! let parsed = parse_with_trivia("SELECT /* note */ 1", Ansi).expect("parses with trivia");
//! // Every skipped run is recoverable from the root and slices back out of source.
//! let comment = parsed.trivia().iter().find_map(|run| {
//!     let span = run.span();
//!     let text = &parsed.source()[span.start() as usize..span.end() as usize];
//!     text.starts_with("/*").then_some(text)
//! });
//! assert_eq!(comment, Some("/* note */"));
//! // `trivia_before` recovers a token's leading trivia by its start offset.
//! assert!(!parsed.trivia_before(18).is_empty()); // the `1` token starts at byte 18
//! ```
//!
//! ## Serialization (`serde` feature)
//!
//! With the `serde` feature on, a [`Parsed`] root round-trips through any serde
//! format. The document is self-contained — the source, the resolver's dynamic
//! string table, and the statement tree with its numeric symbols — so a reloaded tree
//! resolves and renders identically, including across processes. Deserialization runs
//! behind a format-agnostic depth cap
//! (`DEFAULT_DESERIALIZE_DEPTH`, in the `serde` feature's `ast::serde_depth` module) so
//! untrusted bytes cannot rebuild a hostile-deep tree that overflows the stack on its
//! first drop/render/visit (the deserialize counterpart of the parser's recursion
//! guard). Beyond depth, deserialization also *validates the loaded content*
//! in one walk before returning: every non-synthetic span is checked in bounds for the
//! `source` (`start <= end <= source.len()`), every numeric symbol is checked to resolve
//! in the rebuilt table, and a table carrying duplicate entries — which would silently
//! misresolve every later symbol, since re-interning dedupes — is rejected. A violation
//! is a clean deserialize error naming the first offending span/symbol, so a hand-crafted
//! document cannot smuggle in an out-of-table symbol that panics on the first
//! canonical render (`Resolver::resolve`) or an out-of-bounds span that slices the wrong
//! text — the untrusted-input hardening the depth cap began.
//!
//! ```
//! # #[cfg(feature = "serde")] {
//! use squonk::{Parsed, parse};
//!
//! let parsed = parse("SELECT a, b FROM t WHERE a > 1").expect("parses");
//! let json = serde_json::to_string(&parsed).expect("serializes");
//! let restored: Parsed = serde_json::from_str(&json).expect("round-trips");
//! // The reloaded tree renders byte-identically to the original.
//! assert_eq!(restored.to_sql(), parsed.to_sql());
//! # }
//! ```
//!
//! # Performance
//!
//! For high-throughput or allocation-heavy parsing, the cheapest win is the global allocator the *final binary* links: set a fast general-purpose allocator (e.g. `mimalloc` or `jemalloc`) for **~15-19% of parse time on alloc-heavy SQL** (measured by `bench/benches/alloc_probe.rs`, `--profile profiling`). A library cannot set this itself, and `squonk` deliberately takes no allocator dependency, so it is the consumer's choice — workload-dependent, and negligible on tiny statements or any workload that is not allocation-bound.
//!
//! ```ignore
//! #[global_allocator]
//! static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
//! ```
//!
//! ## Parallel parsing
//!
//! Each [`parse_with`] result is an owned, `Send + Sync` `Parsed<Arc<str>>`,
//! so parsing many *independent* SQL strings parallelizes with no feature, dependency,
//! or code from this crate — just the caller's thread pool (e.g.
//! `inputs.par_iter().map(|s| parse_with(s, Ansi))` under rayon). Aggregate throughput
//! scales near-linearly to the performance-core count: a measured 1.16M parses/sec on
//! one thread rises to ~7.3M/sec on 14 threads (Apple M4 Max, 10 performance + 4
//! efficiency cores), ~85-97% efficiency through 4 threads and hardware-bound
//! (memory-bandwidth/DVFS) beyond — and *not* allocator-bound (parse scales identically
//! to an allocation-light tokenize control at every thread count). Size the pool to the
//! **performance**-core count — efficiency cores add sub-proportional throughput — and
//! pair with a fast global allocator (above) for allocation-heavy inputs. Numbers:
//! `docs/performance.md` §3.
//!
//! BATCH parsing — one multi-statement input into a single [`Parsed`] — is deliberately
//! *not* parallelized: the deterministic interner merge is Amdahl-capped at ~1.1×, so it
//! earns no owned `parallel` feature. Parallelize across independent *inputs*, not within
//! one. The scoped-thread form below needs no dependency; a rayon `par_iter` is the same
//! idea with a managed pool.
//!
//! ```
//! use std::thread;
//!
//! use squonk::dialect::Ansi;
//! use squonk::{Parsed, parse_with};
//!
//! let inputs = ["SELECT 1", "INSERT INTO t VALUES (1)", "UPDATE t SET a = 1"];
//!
//! // Fan the independent parses out across scoped threads; each `Parsed` is
//! // `Send + Sync`, so it crosses the thread boundary back to the caller.
//! let parsed: Vec<Parsed> = thread::scope(|scope| {
//!     let handles: Vec<_> = inputs
//!         .iter()
//!         .map(|&sql| scope.spawn(move || parse_with(sql, Ansi).expect("each input parses")))
//!         .collect();
//!     handles
//!         .into_iter()
//!         .map(|handle| handle.join().expect("no worker panics"))
//!         .collect()
//! });
//!
//! assert_eq!(parsed.len(), inputs.len());
//! assert!(parsed.iter().all(|tree| tree.statements().len() == 1));
//! ```
//!
//! # Other language bindings
//!
//! Two `publish = false` bindings crates in this workspace wrap the parser for other
//! runtimes: `squonk-wasm` exposes a tiny JS-value
//! `parse`/`parse_recovering`/`version` surface to the browser/edge, and
//! `squonk-python` packages the serde JSON surface as a maturin extension
//! module (`json.loads` in a thin Python wrapper). See each crate's README for
//! build, size, and untrusted-input notes.

/// The SQL abstract syntax tree, re-exported from `squonk-ast`.
pub use squonk_ast as ast;

#[cfg(feature = "serde-serialize")]
pub mod bindings;
pub mod dialect;
pub mod error;
/// The pretty-printing formatter (layout-IR renderer + comment attachment). Gated
/// behind the non-default `document-render` feature so the serialize-only default and
/// wasm builds stay lean — the formatter compiles only when a consumer opts in.
#[cfg(feature = "document-render")]
pub mod format;
pub mod interner;
pub mod parser;
pub mod render;
pub mod tokenizer;

/// [`parse`] is the crate's default-dialect (`Ansi`) convenience, re-exported here so the crate's own leading example (`use squonk::parse;`) works from the root; [`BuiltinDialect`]/[`parse_with_builtin`] add runtime built-in dialect selection, the compile-time-free sibling of [`parse_with`].
pub use dialect::{
    BuiltinDialect, ParseBuiltinDialectError, parse, parse_recovering_with_builtin,
    parse_recovering_with_builtin_options, parse_with_builtin, parse_with_builtin_options,
    tokenize_with_builtin, tokenize_with_builtin_trivia,
};
/// The parser engine, dialect trait, owned root, and entry points.
pub use parser::{
    ClauseKw, ClauseMark, ClauseMarkIndex, DEFAULT_RECURSION_LIMIT, Dialect, ParseOptions, Parsed,
    Parser, Statements, StockParsed, parse_with, parse_with_options, parse_with_rc,
    parse_with_trivia, statements, statements_with_options,
};
/// Resilient multi-error parsing: collect every diagnostic and the partial AST in
/// one run, instead of stopping at the first error like the default [`parse_with`].
pub use parser::{Recovered, parse_recovering, parse_recovering_with_options};
/// The one-call [`transpile`] convenience — parse under a source dialect, render for a
/// target — and its [`TranspileError`], re-exported from [`render`] beside the
/// lower-level [`Renderer`](render::Renderer) building block it composes.
pub use render::{TranspileError, transpile};
