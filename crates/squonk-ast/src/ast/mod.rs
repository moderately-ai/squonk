// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! SQL AST node types (dialect-agnostic).
//!
//! Every struct node carries a `Meta` field, and every structural enum node
//! carries `meta: Meta` on each variant, so generated code can identify AST nodes
//! uniformly. `ObjectName` is the explicit exception: it is only a thin
//! qualified-name wrapper around `Ident` parts.
//!
//! # Node id and span policy
//!
//! Every node's `Meta` carries a `NodeId` that is unique and nonzero within its
//! tree; the `NonZeroU32` representation makes an unset (zero) id unrepresentable.
//! Because `Meta` is structural-equality-neutral, a fresh id per node never
//! perturbs structural equality.
//!
//! - **Parsed nodes** take their id from the parser's monotonic per-parse counter
//!   and their `span` from the source bytes they cover. Discarded speculation may
//!   leave gaps in the id sequence; gaps are harmless, since the contract is
//!   uniqueness, not density.
//! - **Synthesized / rewrite nodes** — built outside a parse, e.g. by a future
//!   formatter or optimizer — have no backing source text, so they take
//!   [`Span::SYNTHETIC`](crate::vocab::Span::SYNTHETIC) for their span and must
//!   still receive a fresh nonzero id that is unique across the tree they join
//!   (e.g. from a counter seeded past the originating parse's high-water mark).
//!
//! Per-node `Spanned`/`Visit`/`Render` impls are generated separately by
//! `squonk-sourcegen`.
//!
//! # Canonical shape and surface-syntax tags
//!
//! Where dialects spell the *same* semantics differently, the construct has **one**
//! canonical AST shape (the standard's model, or the common cross-dialect shape
//! where the standard is silent). Dialect *acceptance* is gated by [`FeatureSet`]
//! data, never by a distinct shape — there is no `PostgresCast`/`AnsiLimit` fork.
//! Two classes of spelling difference are handled two ways:
//!
//! - **Round-trip-significant spelling → a compact surface tag** (data on the one
//!   shape), so a formatter can reproduce the exact spelling and a transpiler can
//!   retarget it. The tag is a small `Copy` enum or `bool`; it is structural, so two
//!   spellings are the same *shape* but not value-equal.
//! - **Exact-synonym / noise spelling → canonicalized** to the standard form on
//!   render (like keyword casing), with no tag, because the shape is identical and
//!   structural round-trip is preserved.
//!
//! Audited inventory (prod-dialect-canonical-tags-audit); each row is one canonical
//! shape with the conformance proof in `squonk-conformance` `canonical_shapes.rs`:
//!
//! | Construct | Canonical shape | Surface spelling carried as |
//! |---|---|---|
//! | Cast `CAST(x AS t)` / `x::t` | [`Expr::Cast`] | tag [`CastSyntax`] |
//! | Row limit `LIMIT` / `FETCH FIRST … ONLY` | [`Limit`] | tag [`LimitSyntax`] |
//! | Named parameter `:name` / `@name` | [`ParameterKind::Named`] | tag [`ParameterSigil`] |
//! | Row constructor `ROW(…)` / `(…)` | [`RowExpr`] | tag `explicit: bool` |
//! | Tuple assignment `ROW(…)` / `(…)` | [`UpdateTupleSource::Row`] | tag `explicit: bool` |
//! | Inheritance `t` / `t *` / `ONLY t` / `ONLY (t)` | [`RelationInheritance`] | variant + [`OnlySyntax`] |
//! | Table/EXPLAIN options `opt …` / `(opt, …)` | [`TableStorageParameter`] / [`ExplainStatement`] | tag `parenthesized: bool` |
//! | Mutation output `RETURNING …` | [`Returning`] (shared by INSERT/UPDATE/DELETE) | acceptance via `mutation_syntax` |
//! | Set quantifier `ALL` / `DISTINCT` / `DISTINCT ON` | [`SelectDistinct`] | variant; explicit-`ALL` kept distinct from omitted |
//! | GROUP BY item `expr` / `ROLLUP (…)` / `CUBE (…)` / `GROUPING SETS (…)` / `()` | [`GroupByItem`] | variant per grammar position; acceptance via `grouping_sets` |
//! | Not-equal `<>` / `!=` | [`BinaryOperator::NotEq`] | tag [`NotEqSpelling`] |
//! | Join keywords `JOIN`/`INNER JOIN`, `LEFT JOIN`/`LEFT OUTER JOIN` | [`JoinOperator`] | tags `inner: bool` / `outer: bool` (explicit optional keyword) |
//! | `||` / `&&` meaning | [`BinaryOperator::StringConcat`] / [`BinaryOperator::And`] | *semantics* gated by `pipe_operator` / `double_ampersand` |
//! | Modulo `%` / `MOD` | [`BinaryOperator::Modulo`] | tag [`ModuloSpelling`] |
//! | Equality `=` / `==` | [`BinaryOperator::Eq`] | tag [`EqualsSpelling`] |
//! | Integer division `DIV` / `//` | [`BinaryOperator::IntegerDivide`] | tag [`IntegerDivideSpelling`]; MySQL `DIV` via `keyword_operators`, DuckDB `//` via `integer_divide_slash` |
//! | Logical xor `XOR` | [`BinaryOperator::Xor`] | distinct operator key (MySQL); acceptance via `keyword_operators` |
//! | Regex match `RLIKE` / `REGEXP` | [`BinaryOperator::Regexp`] | tag [`RegexpSpelling`] |
//! | PRAGMA value `= v` / `(v)` | [`PragmaStatement`] | tag `parenthesized: bool` |
//! | ATTACH/DETACH `[DATABASE]` keyword | [`AttachStatement`] / [`DetachStatement`] | tag `database_keyword: bool` |
//! | DETACH `IF EXISTS` guard | [`DetachStatement`] | tag `if_exists: bool` (DuckDB) |
//! | CHECKPOINT `FORCE` / `[db]` operands | [`CheckpointStatement`] | tags `force: bool`, `database: Option<Ident>` (DuckDB) |
//! | LOAD bare name / string path | [`LoadTarget`] | variant [`LoadTarget::Name`] / [`LoadTarget::Path`] |
//! | Array constructor `ARRAY[…]` / `[…]` | [`ArrayExpr::Elements`] | tag [`ArraySpelling`] |
//! | Struct field key `'k'` / `k` / `"k"` | [`StructField`] | tag [`StructKeySpelling`] |
//! | Struct constructor `STRUCT(…)` / `STRUCT<…>(…)` | [`StructConstructorExpr`] | acceptance via `struct_constructor`; empty `fields` = typeless |
//!
//! `<>`/`!=` and the optional `INNER`/`OUTER` join words are exact synonyms of the
//! canonical standard spelling universally accepted (no acceptance fork to gate), so
//! the operator key ([`BinaryOperator`]) / operator shape ([`JoinOperator`]) keeps one
//! canonical form and a compact fidelity tag ([`NotEqSpelling`]; `inner`/`outer` bools)
//! records which the source wrote — only a source-fidelity render replays it, a target
//! re-spell and the redacted fingerprint collapse to the canonical spelling. A
//! *dialect-specific* spelling that must round-trip is the same doctrine — the MySQL
//! `MOD` and `RLIKE`/`REGEXP` keywords and the SQLite `==` fold onto the canonical
//! modulo/regex/equality operator with a compact spelling tag ([`ModuloSpelling`] /
//! [`RegexpSpelling`] / [`EqualsSpelling`]), mirroring [`CastSyntax`]. `||`-as-OR and `&&`-as-AND are genuine *semantic*
//! differences (a different operator), so they are gated by `FeatureSet`, not a
//! second shape. The internal `StringLiteralBody::PostgresEscape` materialization
//! helper is likewise a semantic (escape-processing) difference, not a spelling fork.
//!
//! **Adding a construct:** map it to one shape; if a spelling must round-trip, add a
//! compact tag (mirror [`CastSyntax`]); express dialect acceptance via [`FeatureSet`].
//! A new dialect-named shape is caught by the `no_dialect_named_ast_shape_forks`
//! guard in this crate's `ast::tests`.
//!
//! [`FeatureSet`]: crate::dialect::FeatureSet

mod dcl;
mod ddl;
mod dml;
mod expr;
mod ext;
mod ident;
mod literal;
mod match_recognize;
mod pipe_ops;
mod pivot;
mod query;
mod replication;
mod stmt;
mod stored_program;
mod tcl;
mod ty;
mod util;
mod window;

pub use dcl::*;
pub use ddl::*;
pub use dml::*;
pub use expr::*;
pub use ext::*;
pub use ident::*;
pub use literal::*;
pub use match_recognize::*;
pub use pipe_ops::*;
pub use pivot::*;
pub use query::*;
pub use replication::*;
pub use stmt::*;
pub use stored_program::*;
pub use tcl::*;
pub use ty::*;
pub use util::*;
pub use window::*;

// Re-exported so the AST's public child-sequence container (ADR-0007) can be
// named by downstream consumers that match on node fields.
pub use thin_vec::ThinVec;

#[cfg(test)]
pub(crate) use crate::generated::size_asserts::{
    EXPR_SIZE_BUDGET, SET_EXPR_SIZE_BUDGET, STATEMENT_SIZE_BUDGET,
};

#[cfg(test)]
mod tests;
