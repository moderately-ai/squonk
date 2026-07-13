// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! DuckDB SELECT-surface **structural** oracle via `json_serialize_sql`.
//!
//! DuckDB is the only non-PostgreSQL shipped dialect that can reach structural
//! (tree-shape) parity, not just accept/reject: `SELECT json_serialize_sql('SELECT …')`
//! returns DuckDB's own parse tree as JSON, in-process via the `duckdb` crate the M2
//! oracle already links. This module maps that JSON into the **same** neutral
//! [`QueryShape`](crate::shape) family [`pg`](crate::pg) maps the PostgreSQL protobuf into,
//! closing the "wrong-but-round-trippable AST" hole (ADR-0015) for DuckDB's SELECT
//! surface (`duckdb-structural-oracle-select`).
//!
//! # The `StructuralOracle` seam (the second structural source)
//!
//! [`oracle`](crate::oracle) deliberately left structural parity un-abstracted "until
//! that second structural source is actually built". This module *is* that source: it
//! prompted the [`StructuralOracle`] seam — a
//! `sql -> Vec<StatementShape>` mapping — implemented by both
//! [`PgStructuralOracle`](crate::pg::PgStructuralOracle) (wrapping the existing
//! [`pg_shape`](crate::pg::pg_shape)) and [`DuckDbStructuralOracle`], driven through one
//! [`structural_comparison`] against
//! [`squonk_shape`](crate::shape::squonk_shape). Per ADR-0011's one-canonical-shape
//! rule, both reuse the neutral shapes rather than forking a second shape type.
//!
//! # Bound — SELECT-family only (measured, cannot be widened)
//!
//! `json_serialize_sql` errors `Only SELECT statements can be serialized to json!` on
//! every non-SELECT (CREATE, INSERT, and even a top-level `PIVOT`), so this oracle
//! covers SELECT / FROM-first / set-ops / VALUES only. A non-SELECT is
//! [`OutsideSubset`](StructuralShape::OutsideSubset) — a counted skip, never a
//! divergence. DDL/DML stay at accept-parity + round-trip elsewhere.
//!
//! # Normalizations (ADR-0015 representation-equivalence, category 1)
//!
//! Two independently built trees differ in shape without differing in meaning; the
//! mapping **must** absorb those (category 1), and reserve the allowlist for genuine
//! divergences (category 2). The dumps proved these are needed:
//!
//! - **Operator-functions → `BinaryOp`/`UnaryOp`.** DuckDB folds `b + 1` to a
//!   `FUNCTION` named `"+"` (`is_operator: true`); `a AND b` to a `CONJUNCTION`; `a = b`
//!   to a `COMPARISON`. All normalize back to the operator shapes — exactly like PG's
//!   signed-literal folding. An n-ary `CONJUNCTION` (`a AND b AND c`, flattened by
//!   DuckDB) is re-nested left-associatively to match our binary tree.
//! - **Boolean literals.** DuckDB lowers `TRUE`/`FALSE` to `CAST('t'/'f' AS BOOLEAN)`;
//!   the mapping folds that back to [`LiteralShape::Boolean`](crate::shape::LiteralShape).
//! - **Signed / decimal literals.** DuckDB folds `-1` into a signed constant (matching
//!   our `fold_unary`) and stores `1.5` as a scaled `DECIMAL` (`value:15, scale:1`),
//!   reconstructed back to the decimal text our side keeps.
//! - **`count(*)` → `count_star`.** DuckDB's dedicated `count_star` function normalizes
//!   back to the wildcard `count(*)` [`FunctionShape`].
//! - **`VALUES` desugaring.** DuckDB lowers a `VALUES` table to `SELECT * FROM
//!   <expression_list>`; the mapping recognizes that wrapper and produces the neutral
//!   [`SetShape::Values`](crate::shape::SetShape).
//! - **`x IN (subquery)` / `x NOT IN (subquery)`.** DuckDB emits an `ANY`/`COMPARE_EQUAL`
//!   `SUBQUERY` (optionally under `OPERATOR_NOT`); normalized back to
//!   [`ExprShape::InSubquery`](crate::shape::ExprShape) with the negation, matching our AST.
//! - **Collection literals.** DuckDB desugars `[1,2,3]` -> `list_value(...)`,
//!   `{'a':1}` -> `struct_pack(...)` (each written key preserved in its child's
//!   `alias`), `a[1]`/`a[1:2]` -> `ARRAY_EXTRACT`/`ARRAY_SLICE` operators, and
//!   `MAP {…}` -> a `map(list_value, list_value)` call. The mapping normalizes the
//!   first three back to the real [`ExprShape::Array`](crate::shape::ExprShape)/
//!   [`Struct`](crate::shape::ExprShape::Struct)/[`Subscript`](crate::shape::ExprShape::Subscript)
//!   shapes (`duckdb-collection-literals`); the map call needs no normalization —
//!   our side's map shape *is* the documented desugaring, so the generic function
//!   path matches it. One lossy residual: the serializer discards whether the source
//!   wrote the literal or an explicit `list_value(…)`/`struct_pack(…)` call, so the
//!   mapping picks the literal reading and an explicit-call text that reaches a
//!   both-accept comparison is an [allowlisted](DUCKDB_STRUCTURAL_ALLOWLIST)
//!   serializer limitation (one `struct_pack(COLUMNS(…))` corpus line reaches it;
//!   every other such line skips for an independent reason first).
//! - **`->` lambdas vs JSON extraction.** DuckDB serializes *every* `->` as one
//!   `LAMBDA` class (the split is bind-time). The mapping applies the binder's
//!   parameter-shape rule — the same split our parser performs
//!   (`duckdb-lambda-expressions`) — producing [`ExprShape::Lambda`](crate::shape::ExprShape)
//!   for a name-list left side and the `JsonGet` [`BinaryOp`](crate::shape::ExprShape::BinaryOp)
//!   otherwise, so both readings compare structurally instead of skipping.
//!
//! # Honest gaps and residuals (deferred / lossy)
//!
//! - **Star modifiers.** DuckDB carries `EXCLUDE`/`REPLACE`/`RENAME` on the `STAR`
//!   node, and our parser reads them onto the wildcard select item
//!   (`duckdb-select-star-modifiers`), but the modifier lists have no neutral-shape
//!   fields yet — `duckdb-structural-oracle-select` owns that parity — so a
//!   modifier-bearing star stays a counted skip, *not* a mis-map onto a plain
//!   wildcard. `COLUMNS(...)` itself *is* mapped: the `columns: true` `STAR` folds
//!   onto [`ExprShape::Columns`](crate::shape::ExprShape) in both projection-item and
//!   expression position (`sum(COLUMNS(*))`, `ORDER BY COLUMNS('re')`), with the
//!   engine's `COLUMNS(λ)` -> `list_filter(<bare *>, λ)` desugaring unwrapped back to
//!   the lambda pattern, and the sole whole-projection `ORDER BY COLUMNS(*)` order
//!   key lifted to the order-by-all mode on *both* sides (`ORDER BY ALL` and
//!   `ORDER BY COLUMNS(*)` serialize identically; probed on 1.5.4). (The
//!   `ASOF`/`POSITIONAL` joins that used to skip alongside the old blanket
//!   `COLUMNS` skip are now grammar the DuckDb preset parses —
//!   `duckdb-nonstandard-joins` — and map one-to-one onto
//!   [`JoinOperatorShape::AsOf`]/[`Positional`](JoinOperatorShape::Positional).)
//! - **Case-insensitive function names.** DuckDB lowers an unquoted call (`COUNT` ->
//!   `count`); our neutral shape folds function names identically (`fold_object_name_shape`
//!   in [`shape`](crate::shape)), so the two compare equal without an allowlist entry — a
//!   category-1 normalization, not a residual.
//! - **Serializer-lossy forms (skipped, not compared).** `json_serialize_sql` discards
//!   the `RECURSIVE` keyword (a `WITH RECURSIVE` serializes identically to a plain
//!   `WITH`) and conflates `FROM a, b` with `a CROSS JOIN b` (both become a `CROSS`
//!   `JOIN` node). These are DuckDB serializer limitations, not parser divergences, so
//!   the comma/cross case is a counted skip and the recursive case, if it reaches the
//!   comparison, is an [allowlisted](DUCKDB_STRUCTURAL_ALLOWLIST) residual.
//!
//! Refs: ADR-0011 (one canonical shape), ADR-0015 (differential + the
//! representation-equivalence amendment).

use crate::duckdb_ffi::Connection as DuckDbConnection;
use serde_json::Value;
use squonk::Dialect;
use squonk::ast::{AsOfJoinKind, NoExt, SemiAntiSide, SubscriptKind};
use squonk::dialect::DuckDb;
use squonk::parse_with;
use squonk_ast::render::RenderMode;

use crate::oracle::{
    Comparison, OracleUnavailable, StructuralOracle, StructuralShape, structural_comparison,
};
use crate::render_statements;
use crate::shape::{
    AliasShape, BinaryOperatorShape, CteBodyShape, CteShape, DataTypeShape, ExprShape,
    FunctionShape, GroupByItemShape, JoinConstraintShape, JoinOperatorShape, JoinShape, LimitShape,
    LiteralShape, OrderByAllShape, OrderByShape, PivotColumnShape, PivotExprShape, PivotShape,
    QueryShape, SelectItemShape, SelectShape, SetOpShape, SetShape, StatementShape,
    TableFactorShape, TableWithJoinsShape, UnaryOperatorShape, UnpivotColumnShape, UnpivotShape,
    ValuesItemShape, WhenClauseShape, WithShape,
};

// ---------------------------------------------------------------------------
// The DuckDB structural oracle
// ---------------------------------------------------------------------------

/// The DuckDB structural oracle: `json_serialize_sql` in-process via the system-linked
/// `duckdb` engine, mapped into the neutral shape family.
///
/// Owns its own [`DuckDbConnection`] (mirroring [`DuckDbOracle`](crate::m2::DuckDbOracle)):
/// like the M2 oracle it reports [`OracleUnavailable`] only when the connection cannot be
/// opened (a missing/incompatible `libduckdb` at run time).
pub struct DuckDbStructuralOracle {
    conn: DuckDbConnection,
}

impl DuckDbStructuralOracle {
    /// A bare in-memory connection — `json_serialize_sql` is a parser-only serialization,
    /// so no schema is provisioned (unlike the M2 prepare/bind oracle).
    pub fn new() -> Result<Self, OracleUnavailable> {
        let conn = DuckDbConnection::open_in_memory()?;
        Ok(Self { conn })
    }

    /// Serialize `sql` to DuckDB's parse-tree JSON. The SQL is inlined as a string
    /// literal (single quotes doubled) — the corpus is single-statement, newline-free
    /// test SQL, so this is injection-safe and avoids any bind-time constant-folding
    /// surprise inside `json_serialize_sql`.
    fn serialize(&self, sql: &str) -> Result<Value, OracleUnavailable> {
        let escaped = sql.replace('\'', "''");
        let query = format!("SELECT json_serialize_sql('{escaped}')");
        let json = self.conn.query_string(&query)?;
        serde_json::from_str(&json)
            .map_err(|err| OracleUnavailable(format!("duckdb JSON parse failed: {err}")))
    }
}

impl StructuralOracle for DuckDbStructuralOracle {
    fn name(&self) -> &'static str {
        "duckdb"
    }

    fn shape(&self, sql: &str) -> Result<StructuralShape, OracleUnavailable> {
        let root = self.serialize(sql)?;
        // `json_serialize_sql` reports a parse/"only SELECT" failure in-band via the
        // top-level `error` flag (the query itself succeeds) — a skip, not a divergence.
        if field(&root, "error").as_bool().unwrap_or(false) {
            let msg = field(&root, "error_message")
                .as_str()
                .unwrap_or("unknown error");
            return Ok(StructuralShape::OutsideSubset(format!(
                "duckdb rejected: {msg}"
            )));
        }
        Ok(match duckdb_json_shape(&root) {
            Ok(shape) => StructuralShape::Mapped(shape),
            Err(reason) => StructuralShape::OutsideSubset(reason),
        })
    }
}

// ---------------------------------------------------------------------------
// The DuckDB differential (over the shared `structural_comparison`)
// ---------------------------------------------------------------------------

/// The dialect our parser runs under when comparing against DuckDB's structural
/// oracle. The structural differential is only meaningful over the subset BOTH sides
/// accept, so this single knob fixes our half of "comparable".
///
/// The fitted [`DuckDb`] preset (PostgreSQL-derived): the comparable subset is the
/// PostgreSQL-shared surface both sides accept, and every comparable tree matches. DuckDB
/// serializes *every* `->` as a `LAMBDA`; the preset parses the
/// same split DuckDB's binder applies (`duckdb-lambda-expressions`), so the mapping
/// resolves that class structurally (see [`duckdb_expr_shape`]'s `LAMBDA` arm) rather
/// than skipping it — which also let the co-occurring residuals that skip was masking
/// reach the comparison (the triaged [`DUCKDB_STRUCTURAL_ALLOWLIST`] entries). This is
/// the ONE place the parse dialect lives; nothing else names one.
/// matches. The one new collision — DuckDB serializes *every* `->` as a `LAMBDA` where
/// our preset reads the inherited PostgreSQL JSON `->` as a `JsonGet` — is resolved by a
/// mapping skip (see [`duckdb_expr_shape`]'s `LAMBDA` arm), not a divergence, so
/// [`DUCKDB_STRUCTURAL_ALLOWLIST`] stays a short, ticketed residual. This is the ONE place the parse dialect
/// lives; nothing else names one.
fn duckdb_parse_dialect() -> impl Dialect<Ext = NoExt> {
    DuckDb
}

/// The DuckDB structural divergence for `sql` — `Some(detail)` when our parse (under
/// `duckdb_parse_dialect`) and DuckDB's `json_serialize_sql` tree map to different
/// neutral shapes, else `None` (match, or either side outside the comparable subset).
/// Mirrors [`pg_structural_divergence`](crate::pg::pg_structural_divergence). Raw: the
/// [allowlist](DUCKDB_STRUCTURAL_ALLOWLIST) is composed by callers, not here.
pub fn duckdb_structural_divergence(oracle: &DuckDbStructuralOracle, sql: &str) -> Option<String> {
    match structural_comparison(sql, duckdb_parse_dialect(), oracle) {
        Comparison::Divergence(detail) => Some(detail),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Triaged residual allowlist (ADR-0015 category 2 / serializer limitations)
// ---------------------------------------------------------------------------

/// A triaged DuckDB structural residual: a both-accept SELECT statement whose neutral
/// shapes differ for a reason that is *not* a category-1 normalization the mapping should
/// absorb — either a genuine divergence (ADR-0015 category 2) or an information the DuckDB
/// serializer discards (so parity is unrecoverable from the JSON). Every entry names a
/// ticket; a test asserts each still diverges so a silent fix forces its removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuckDbStructuralAllowlistEntry {
    pub sql: &'static str,
    pub ticket: &'static str,
    pub reason: &'static str,
}

/// The shared reason for the `list_value(…)` explicit-call entries below.
const LIST_VALUE_CALL_REASON: &str = "json_serialize_sql emits an explicit list_value(…) \
     call and the [..] literal identically; the mapping picks the literal reading (the \
     dominant surface), so the written call diverges — a serializer limitation, not a \
     parser bug";

/// The shared reason for the relaxed-interval-literal entries below.
const INTERVAL_UNIT_DESUGAR_REASON: &str = "json_serialize_sql desugars every \
     `INTERVAL <amount> <unit>` at parse into a `to_<unit>(...)` function call \
     (`INTERVAL 1000 DAY` -> `to_days(CAST(TRUNC(CAST(1000 AS float8)) AS int4))`, \
     `INTERVAL '1' hours` -> `to_hours(...)`), while our parse keeps the surface interval \
     literal (mapped to Unmapped like every temporal literal). A serializer-lossy desugar, \
     not a parser bug — both halves accept; only the engine's internal to_X rewrite differs. \
     Newly comparable once duckdb-interval-spellings landed the relaxed amount forms";

/// Current DuckDB structural residuals — the six statements the former `LAMBDA`
/// whole-statement skip was masking (`duckdb-lambda-expressions` mapped that class,
/// so its co-occurring residuals now reach the comparison; the lambda halves of all
/// six match). Two categories:
///
/// - **`list_value(…)` explicit-call lossiness** (five entries): the serializer
///   emits an explicit `list_value(1, 2, 3)` call and the `[1, 2, 3]` literal
///   identically, so the mapping's documented literal-reading choice (module docs,
///   `duckdb-collection-literals`) diverges against our faithful `Function` parse of
///   the written call — the exact "allowlisted serializer limitation" the mapping
///   docs predicted for any such text that reaches a both-accept comparison.
/// - **Bare `DOUBLE` type name** (two entries): DuckDB's native `x::DOUBLE` spelling is
///   not in the PostgreSQL-derived type vocabulary (PostgreSQL requires `DOUBLE
///   PRECISION`), so our parse reads a user-defined type named `DOUBLE` where the
///   engine serializes the builtin `float8` — a genuine DuckDB type-vocabulary
///   coverage gap owned by the umbrella programme. The second entry
///   (`list_filter([2], x -> x::DOUBLE == 2)`) is comparable via the `==` spelling
///   (`duckdb-operator-and-literal-gaps`) and shows the same
///   `DOUBLE`→`float8` divergence — not an operator-mapping fault (the `==`
///   halves match; the mismatch is confined to the cast's type name).
pub const DUCKDB_STRUCTURAL_ALLOWLIST: &[DuckDbStructuralAllowlistEntry] = &[
    DuckDbStructuralAllowlistEntry {
        sql: "select list_apply(i, x -> x * 3 + 2 / zz) from (values (list_value(1, 2, 3))) tbl(i)",
        ticket: "duckdb-structural-oracle-select",
        reason: LIST_VALUE_CALL_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: "select x -> x + 1 from (values (list_value(1, 2, 3))) tbl(i)",
        ticket: "duckdb-structural-oracle-select",
        reason: LIST_VALUE_CALL_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: "SELECT list_apply(i, a.x -> x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)",
        ticket: "duckdb-structural-oracle-select",
        reason: LIST_VALUE_CALL_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: "select list_apply(i, x -> x + 1 AND y + 1) from (values (list_value(1, 2, 3))) tbl(i)",
        ticket: "duckdb-structural-oracle-select",
        reason: LIST_VALUE_CALL_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: "SELECT list_apply(i, a.x -> a.x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)",
        ticket: "duckdb-structural-oracle-select",
        reason: LIST_VALUE_CALL_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: "SELECT list_transform([2.0::DOUBLE], x -> x::INTEGER)",
        ticket: "duckdb-dialect-100-percent-programme",
        reason: "DuckDB's bare `DOUBLE` type spelling is outside the PostgreSQL-derived \
                 vocabulary (PostgreSQL spells it `DOUBLE PRECISION`), so our parse reads \
                 a user-defined type where the engine serializes the builtin float8 — a \
                 type-vocabulary coverage gap, not a lambda mapping fault (the lambda \
                 halves match)",
    },
    DuckDbStructuralAllowlistEntry {
        sql: "SELECT list_filter([2], x -> x::DOUBLE == 2)",
        ticket: "duckdb-dialect-100-percent-programme",
        reason: "the same bare-`DOUBLE`→`float8` type-vocabulary divergence as the \
                 `list_transform` entry above, newly comparable once \
                 `duckdb-operator-and-literal-gaps` landed the `==` equality spelling: our \
                 `x::DOUBLE` reads a user-defined type where the engine serializes the \
                 builtin float8. The `==` halves match (both `op: Eq`); the mismatch is \
                 confined to the cast type name, so it is the type-vocabulary gap, not an \
                 operator-spelling fault",
    },
    DuckDbStructuralAllowlistEntry {
        sql: "select COLUMNS(*), struct_pack(COLUMNS(['id'])) from data",
        ticket: "duckdb-structural-oracle-select",
        reason: "the struct_pack analogue of the list_value entries above: the \
                 serializer emits an explicit struct_pack(…) call and the {…} literal \
                 identically, so the mapping's documented literal-reading choice reads \
                 the engine tree as a Struct where our parse keeps the written call — \
                 a serializer limitation the COLUMNS mapping unmasked (the statement \
                 previously skipped on its star expression, \
                 duckdb-select-star-modifiers); the COLUMNS halves match",
    },
    DuckDbStructuralAllowlistEntry {
        sql: r#"SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM (UNPIVOT t1 ON "Sales (05/19/2020)", "Sales (06/03/2020)", "Sales (10/23/2020)" INTO NAME date VALUE sales) ORDER BY ALL"#,
        ticket: "duckdb-pivot-unpivot",
        reason: UNPIVOT_STATEMENT_DESUGAR_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: r#"SELECT * FROM (UNPIVOT t1 ON "Sales (05/19/2020)" AS "2020-05-19", "Sales (06/03/2020)" AS "2020-06-03", "Sales (10/23/2020)" AS "2020-10-23" INTO NAME date VALUE sales) ORDER BY ALL"#,
        ticket: "duckdb-pivot-unpivot",
        reason: UNPIVOT_STATEMENT_DESUGAR_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: "WITH CPB(CPDH,NF,JG) AS MATERIALIZED ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) from CPB pivot (sum(jg) for nf in (2010, 2017, 2018, 2022) group by cpdh)",
        ticket: "duckdb-structural-oracle-select",
        reason: "DuckDB's serializer erases the CTE materialization hint: both AS \
                 MATERIALIZED and AS NOT MATERIALIZED serialize as \
                 CTE_MATERIALIZE_DEFAULT (probed on 1.5.4 - CTEs materialize by \
                 default in 1.x, so the engine stores no distinct flag), while our \
                 parse keeps the written hint. Serializer-lossy, the \
                 list_value/struct_pack class; unmasked when pivot made the \
                 statement comparable",
    },
    DuckDbStructuralAllowlistEntry {
        sql: "SELECT COLUMNS([x for x in COLUMNS(*)]) FROM integers",
        ticket: "duckdb-python-style-expressions",
        reason: "the list comprehension is a surface-preserving node \
                 (ArrayExpr::Comprehension -> ExprShape::Unmapped, like ARRAY(<query>)), \
                 while DuckDB's serializer desugars it to a list_apply/list_filter call \
                 tree, so the outer COLUMNS pattern reads Some(Unmapped) here against the \
                 engine's Some(Function(list_apply)) — a modelling choice, not a parser \
                 bug (DuckDB binder-rejects the nested COLUMNS anyway; only the \
                 parse-only serializer reaches this comparison)",
    },
    DuckDbStructuralAllowlistEntry {
        sql: "SELECT * FROM issue14384 INNER JOIN ( SELECT INTERVAL 1000 DAY AS col0 FROM issue14384) AS sub0 ON (issue14384.i < sub0.col0) ORDER BY ALL",
        ticket: "duckdb-interval-spellings",
        reason: INTERVAL_UNIT_DESUGAR_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: "SELECT * FROM issue14384 INNER JOIN ( SELECT INTERVAL 1000 DAY AS col0 FROM issue14384) AS sub0 ON (issue14384.i < sub0.col0) WHERE (NOT (issue14384.i != issue14384.i)) ORDER BY ALL",
        ticket: "duckdb-interval-spellings",
        reason: "the `INTERVAL 1000 DAY` -> `to_days(...)` serializer desugar of the entry \
                 above, plus a second engine-side rewrite riding along on the same \
                 newly-comparable statement: DuckDB folds the WHERE `NOT (i != i)` into \
                 `i = i` (double-negation simplification), where our parse keeps the written \
                 `NOT (… != …)`. Both are serializer/parser rewrites, not parser bugs",
    },
    DuckDbStructuralAllowlistEntry {
        sql: "from tbl1 asof join tbl2 on tbl1.x = tbl2.x and tbl1.ts >= tbl2.ts and (tbl1.ts - tbl2.ts) < interval '1' hours",
        ticket: "duckdb-interval-spellings",
        reason: INTERVAL_UNIT_DESUGAR_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: "SELECT list_transform([NULL, DATE '1992-09-20', DATE '2021-09-20'], elem -> extract('year' FROM elem) BETWEEN 2000 AND 2022)",
        ticket: "duckdb-expression-and-clause-tails",
        reason: EXTRACT_STRING_FIELD_DATE_LITERAL_REASON,
    },
    DuckDbStructuralAllowlistEntry {
        sql: "SELECT list_filter([NULL, DATE '1992-09-20', DATE '2021-09-20'], elem -> extract('year' FROM elem) BETWEEN 2000 AND 2022)",
        ticket: "duckdb-expression-and-clause-tails",
        reason: EXTRACT_STRING_FIELD_DATE_LITERAL_REASON,
    },
];

/// The shared reason for the two `extract('field' FROM …)` list-lambda entries above.
const EXTRACT_STRING_FIELD_DATE_LITERAL_REASON: &str = "newly comparable once `duckdb-expression-and-clause-tails` landed the quoted \
     `extract('field' FROM x)` field spelling (our parser previously rejected these, so \
     they skipped as `OursReject`). The divergence is confined to the array's `DATE '…'` \
     typed literals: our shape deliberately maps every typed temporal literal to \
     `ExprShape::Unmapped` (see `squonk_shape`'s `LiteralKind::Date` arm), while \
     DuckDB's serializer emits an explicit `CAST('…' AS DATE)`. The extract/lambda halves \
     are `Unmapped` on both sides and match — the temporal-literal mapping gap is \
     unrelated to the extract-field feature that made the statements comparable, the same \
     class as the bare-`DOUBLE` type-vocabulary entries above";

/// The shared reason for the parenthesized-unpivot-statement entries above.
const UNPIVOT_STATEMENT_DESUGAR_REASON: &str = "DuckDB desugars a parenthesized UNPIVOT statement in FROM into a SUBQUERY node \
     (`SELECT * FROM <pivot>`) that serializes identically to a written derived table, \
     so the engine tree cannot be told apart from `FROM (SELECT * FROM t1 UNPIVOT (…))` \
     while our parse keeps the statement-spelled factor — a serializer-lossy desugar, \
     not a parser bug (the parenthesized PIVOT statement never even reaches comparison: \
     json_serialize_sql refuses it outright)";

/// Whether a divergence for `sql` is named in [`DUCKDB_STRUCTURAL_ALLOWLIST`].
pub fn duckdb_structural_allowlisted(sql: &str) -> bool {
    DUCKDB_STRUCTURAL_ALLOWLIST
        .iter()
        .any(|entry| entry.sql == sql)
}

// ---------------------------------------------------------------------------
// Oracle-mediated structural lane (conformance-mediated-structural-lane-duckdb)
// ---------------------------------------------------------------------------

/// The classification of one DuckDB both-accept SELECT under the
/// `json_serialize_sql`-mediated structural lane ([`DuckDbMediatedStructuralOracle`]).
///
/// The DuckDB analogue of [`PgMediatedVerdict`](crate::pg::PgMediatedVerdict): where PG
/// self-compares two `pg_query::fingerprint` hexes, DuckDB self-compares two
/// `json_serialize_sql` parse trees, each with its byte-offset `query_location` fields
/// stripped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DuckDbMediatedVerdict {
    /// `sql` is outside the comparable subset — our parser rejects it, or DuckDB cannot
    /// serialize the original (a non-SELECT, or a statement DuckDB itself rejects: both
    /// surface as the in-band `error` flag on `json_serialize_sql`) — so there is nothing
    /// to compare. A skip: never a match/mismatch/unparseable.
    Skip(String),
    /// Our canonical render serializes to the SAME `json_serialize_sql` tree as the
    /// original (`query_location` stripped): the parse-tree shape survived the parse ->
    /// render round trip, modulo the serializer's documented blindness (see the
    /// [oracle](DuckDbMediatedStructuralOracle) docs).
    Match,
    /// The two normalized trees differ — a structural drift implicating the PARSER (we
    /// built the wrong tree) OR the RENDERER (we canonicalized to a different shape). The
    /// rendered SQL and both compact trees are carried for triage.
    Mismatch {
        rendered: String,
        original_tree: String,
        render_tree: String,
    },
    /// DuckDB set the in-band `error` flag on our canonical render: the renderer emitted
    /// SQL DuckDB cannot serialize back (a syntax error, or a non-SELECT form). Also a
    /// parser-OR-renderer drift, isolated for triage.
    RenderUnparseable(String),
}

/// The outcome of serializing one statement to its normalized `json_serialize_sql` tree.
enum Serialized {
    /// A serializable SELECT: its parse tree with every `query_location` stripped.
    Tree(Value),
    /// `json_serialize_sql` set its in-band `error` flag — a parse error or a non-SELECT.
    Rejected(String),
}

/// The oracle-mediated structural lane for DuckDB — the **commodity default** structural
/// check (conformance-mediated-structural-lane-duckdb), the DuckDB analogue of
/// [`PgMediatedStructuralOracle`](crate::pg::PgMediatedStructuralOracle).
///
/// For a both-accept SELECT `s`, it round-trips `s` through our parser and canonical
/// renderer and asks DuckDB whether the *shape* survived, by comparing
/// `json_serialize_sql(s)` against
/// `json_serialize_sql(render_statements(&parse_with(s, DuckDb)?, squonk::ParseConfig::new(Canonical)))`, each
/// normalized by stripping DuckDB's byte-offset `query_location` fields (the sole
/// position-bearing field; the same-named `offset` field is the semantic LIMIT OFFSET and
/// is kept). Each side self-compares engine-tree vs engine-tree in DuckDB's OWN
/// serialization, so there is no cross-engine neutral vocabulary to reconcile — a small
/// adapter instead of the hundreds-of-lines hand-written [`DuckDbStructuralOracle`] mapper.
///
/// # Commodity vs premium — why [`DuckDbStructuralOracle`] STAYS
///
/// Because BOTH sides pass through `json_serialize_sql`, this lane is BLIND, by
/// construction, to every distinction DuckDB's serializer itself discards or folds — a
/// misparse whose ONLY symptom is one of these re-spells into the same serialized tree and
/// is invisible here:
///
/// - **Erased spellings** — the `RECURSIVE` keyword (`WITH RECURSIVE` serializes as a
///   plain `WITH`), the `FROM a, b` vs `a CROSS JOIN b` comma/cross split, the CTE
///   materialization hint (`AS [NOT] MATERIALIZED` both serialize as
///   `CTE_MATERIALIZE_DEFAULT` on 1.x), and the literal-vs-explicit-constructor spelling
///   (`[1, 2, 3]` vs `list_value(1, 2, 3)`, `{...}` vs `struct_pack(...)`).
/// - **Folded representations** — boolean literals lowered to `CAST('t' AS BOOLEAN)`,
///   `count(*)` to `count_star`, signed/decimal-literal folding, operator functions
///   (`a + b`, `a AND b`, `a = b`) to their `FUNCTION`/`CONJUNCTION`/`COMPARISON` forms,
///   every `->` to one `LAMBDA` class, `ORDER BY ALL` to `ORDER BY COLUMNS(*)`, and
///   identifier-vs-string unpivot column names.
///
/// Detecting those is exactly what the hand-written [`DuckDbStructuralOracle`]
/// neutral-shape mapper encodes — it applies the binder's split rules and preserves the
/// written spelling where our AST does — which is why it remains the **premium** tier:
/// this lane augments it, it does not replace it. This is the DuckDB analogue of the PG
/// fingerprint lane's literal/alias/arity blindness.
pub struct DuckDbMediatedStructuralOracle {
    conn: DuckDbConnection,
}

impl DuckDbMediatedStructuralOracle {
    /// A bare in-memory connection — `json_serialize_sql` is a parser-only serialization,
    /// so no schema is provisioned (mirroring [`DuckDbStructuralOracle::new`]). Reports
    /// [`OracleUnavailable`] only when the connection cannot be opened.
    pub fn new() -> Result<Self, OracleUnavailable> {
        let conn = DuckDbConnection::open_in_memory()?;
        Ok(Self { conn })
    }

    /// Stable identifier used in divergence reports.
    pub fn name(&self) -> &'static str {
        "duckdb (json_serialize_sql-mediated)"
    }

    /// Classify `sql` under the mediated lane (see [`DuckDbMediatedVerdict`]).
    /// Self-contained: it recomputes the comparability precondition (our parse + DuckDB's
    /// serialization of the original), so a non-comparable statement is a
    /// [`Skip`](DuckDbMediatedVerdict::Skip). `Err` is a genuine transport fault (the
    /// engine or the json serializer became unavailable), never a parse verdict — those
    /// arrive in-band via `json_serialize_sql`'s `error` flag. This is why the lane
    /// returns a `Result` where the always-in-process PG lane does not.
    pub fn verdict(&self, sql: &str) -> Result<DuckDbMediatedVerdict, OracleUnavailable> {
        let parsed = match parse_with(sql, squonk::ParseConfig::new(DuckDb)) {
            Ok(parsed) => parsed,
            Err(err) => {
                return Ok(DuckDbMediatedVerdict::Skip(format!(
                    "squonk rejected: {err:?}"
                )));
            }
        };
        // Serializing the original IS the DuckDB side of the comparable precondition:
        // success requires DuckDB to parse it AND it to be a SELECT; the in-band error
        // (non-SELECT or DuckDB syntax reject) is the skip.
        let original = match self.serialize_normalized(sql)? {
            Serialized::Tree(tree) => tree,
            Serialized::Rejected(msg) => {
                return Ok(DuckDbMediatedVerdict::Skip(format!(
                    "duckdb could not serialize the original: {msg}"
                )));
            }
        };
        let rendered = render_statements(&parsed, RenderMode::Canonical);
        Ok(match self.serialize_normalized(&rendered)? {
            Serialized::Tree(tree) if tree == original => DuckDbMediatedVerdict::Match,
            Serialized::Tree(tree) => DuckDbMediatedVerdict::Mismatch {
                rendered,
                original_tree: original.to_string(),
                render_tree: tree.to_string(),
            },
            Serialized::Rejected(msg) => DuckDbMediatedVerdict::RenderUnparseable(format!(
                "duckdb rejected our canonical render {rendered:?}: {msg}"
            )),
        })
    }

    /// Serialize `sql` to its `json_serialize_sql` parse tree with every `query_location`
    /// stripped, or report the in-band serializer error. The SQL is inlined as a doubled
    /// string literal (as [`DuckDbStructuralOracle`]'s own serializer — the corpus is
    /// single-statement, newline-free test SQL, so this is injection-safe).
    fn serialize_normalized(&self, sql: &str) -> Result<Serialized, OracleUnavailable> {
        let escaped = sql.replace('\'', "''");
        let query = format!("SELECT json_serialize_sql('{escaped}')");
        let json = self.conn.query_string(&query)?;
        let mut root: Value = serde_json::from_str(&json)
            .map_err(|err| OracleUnavailable(format!("duckdb JSON parse failed: {err}")))?;
        if field(&root, "error").as_bool().unwrap_or(false) {
            let msg = field(&root, "error_message")
                .as_str()
                .unwrap_or("unknown error")
                .to_owned();
            return Ok(Serialized::Rejected(msg));
        }
        strip_query_location(&mut root);
        Ok(Serialized::Tree(root))
    }
}

/// Recursively remove every `query_location` field from a `json_serialize_sql` tree.
///
/// `query_location` is DuckDB's byte-offset position marker — the SOLE position-bearing
/// field (verified against 1.5.4: reformatting a statement changes only `query_location`,
/// so stripping it makes two spellings of one statement serialize identically). The
/// same-named `offset` field is the semantic LIMIT OFFSET expression, NOT a position, and
/// is deliberately preserved. Key order needs no canonicalization: `serde_json::Value`
/// object equality is order-independent.
fn strip_query_location(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("query_location");
            for child in map.values_mut() {
                strip_query_location(child);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_query_location),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// JSON -> neutral shape mapping
// ---------------------------------------------------------------------------

const NULL: Value = Value::Null;

/// A `json_serialize_sql` field, or JSON `null` when absent.
fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&NULL)
}

/// A string field, or `""` when absent / not a string.
fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    field(value, key).as_str().unwrap_or("")
}

/// An array field, or the empty slice when absent / not an array.
fn arr_field<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    field(value, key)
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// A DuckDB expression's node kind (`class`).
fn class(value: &Value) -> &str {
    str_field(value, "class")
}

/// The empty `LIMIT` shape (no count / offset / ties).
fn empty_limit() -> LimitShape {
    LimitShape {
        count: None,
        offset: None,
        with_ties: false,
    }
}

/// Map DuckDB's top-level serialization (`{"statements":[{"node":…}]}`) to the neutral
/// statement shapes. `Err` = outside the comparable subset (a skip).
pub fn duckdb_json_shape(root: &Value) -> Result<Vec<StatementShape>, String> {
    arr_field(root, "statements")
        .iter()
        .map(|statement| {
            let node = field(statement, "node");
            Ok(StatementShape::Query(duckdb_query_shape(node)?))
        })
        .collect()
}

/// The query-level clauses DuckDB carries as `modifiers` on a query node.
struct Modifiers {
    order_by: Vec<OrderByShape>,
    /// DuckDB's `ORDER BY ALL` clause mode. The engine serializes it as a single
    /// order entry whose expression is the whole-projection `COLUMNS(*)` star node
    /// (probed on 1.5.4: `ORDER BY ALL` and `ORDER BY COLUMNS(*)` serialize
    /// identically, so the engine's own tree cannot distinguish them), lifted here
    /// to the neutral mode rather than left to the star-in-expression skip.
    order_by_all: Option<OrderByAllShape>,
    limit: LimitShape,
    /// `DISTINCT` / `DISTINCT ON` (its targets are dropped, matching our `distinct` flag).
    distinct: bool,
}

fn collect_modifiers(node: &Value) -> Result<Modifiers, String> {
    let mut order_by = Vec::new();
    let mut order_by_all = None;
    let mut limit = empty_limit();
    let mut distinct = false;
    let (mut seen_order, mut seen_limit) = (false, false);

    for modifier in arr_field(node, "modifiers") {
        match str_field(modifier, "type") {
            "ORDER_MODIFIER" => {
                if seen_order {
                    return Err("multiple ORDER modifiers".into());
                }
                seen_order = true;
                let orders = arr_field(modifier, "orders");
                // `ORDER BY ALL`: a single order whose expression is the bare
                // whole-projection star (`columns: true`, no wildcard modifiers).
                // The mode admits no sibling keys (DuckDB rejects `ORDER BY ALL,
                // x`), so only a sole entry can be the mode; a `COLUMNS(...)`
                // carrying a pattern/`EXCLUDE` stays an expression and falls to
                // the star-in-expression skip below.
                if let [order] = orders
                    && is_order_by_all_star(field(order, "expression"))
                {
                    order_by_all = Some(OrderByAllShape {
                        asc: duckdb_order_direction(order)?,
                        nulls_first: duckdb_nulls_order(order)?,
                    });
                } else {
                    order_by = orders
                        .iter()
                        .map(duckdb_order_by_shape)
                        .collect::<Result<_, _>>()?;
                }
            }
            "LIMIT_MODIFIER" => {
                if seen_limit {
                    return Err("multiple LIMIT modifiers".into());
                }
                seen_limit = true;
                limit = LimitShape {
                    count: opt_expr(field(modifier, "limit"))?,
                    offset: opt_expr(field(modifier, "offset"))?,
                    with_ties: false,
                };
            }
            // `DISTINCT ON` drops its targets to our bool `distinct`, matching
            // `select_shape`'s `SelectDistinct::On => distinct: true`.
            "DISTINCT_MODIFIER" => distinct = true,
            "LIMIT_PERCENT_MODIFIER" => return Err("LIMIT % has no neutral shape".into()),
            other => return Err(format!("unsupported query modifier {other}")),
        }
    }

    Ok(Modifiers {
        order_by,
        order_by_all,
        limit,
        distinct,
    })
}

/// Whether an ORDER-entry expression is the bare whole-projection star DuckDB
/// serializes for `ORDER BY ALL`: the `STAR` class with `columns: true` and no
/// relation qualifier, wildcard modifier, or `COLUMNS(<pattern>)` argument.
fn is_order_by_all_star(expr: &Value) -> bool {
    class(expr) == "STAR"
        && field(expr, "columns").as_bool().unwrap_or(false)
        && str_field(expr, "relation_name").is_empty()
        && arr_field(expr, "exclude_list").is_empty()
        && arr_field(expr, "replace_list").is_empty()
        && arr_field(expr, "rename_list").is_empty()
        && arr_field(expr, "qualified_exclude_list").is_empty()
        && field(expr, "expr").is_null()
}

fn duckdb_order_by_shape(order: &Value) -> Result<OrderByShape, String> {
    Ok(OrderByShape {
        expr: duckdb_expr_shape(field(order, "expression"))?,
        asc: duckdb_order_direction(order)?,
        // DuckDB has no `ORDER BY … USING <operator>` sort-operator form.
        using: None,
        nulls_first: duckdb_nulls_order(order)?,
    })
}

fn duckdb_order_direction(order: &Value) -> Result<Option<bool>, String> {
    match str_field(order, "type") {
        "ASCENDING" | "ORDER_ASCENDING" => Ok(Some(true)),
        "DESCENDING" | "ORDER_DESCENDING" => Ok(Some(false)),
        "ORDER_DEFAULT" => Ok(None),
        other => Err(format!("unsupported ORDER direction {other}")),
    }
}

fn duckdb_nulls_order(order: &Value) -> Result<Option<bool>, String> {
    match str_field(order, "null_order") {
        "NULLS FIRST" | "NULLS_FIRST" | "ORDER_NULLS_FIRST" => Ok(Some(true)),
        "NULLS LAST" | "NULLS_LAST" | "ORDER_NULLS_LAST" => Ok(Some(false)),
        "ORDER_DEFAULT" | "NULLS_DEFAULT" => Ok(None),
        other => Err(format!("unsupported NULLS order {other}")),
    }
}

/// A full query node (`SELECT_NODE` / `SET_OPERATION_NODE`) with its own modifiers, as a
/// [`QueryShape`]. Mirrors [`pg_query_shape`](crate::pg) — DuckDB carries `ORDER BY` /
/// `LIMIT` in `modifiers` and `DISTINCT` in a `DISTINCT_MODIFIER` (folded into the select
/// body), where PostgreSQL splits them across protobuf fields.
fn duckdb_query_shape(node: &Value) -> Result<QueryShape, String> {
    let modifiers = collect_modifiers(node)?;
    Ok(QueryShape {
        with: duckdb_with_shape(node)?,
        body: duckdb_body_shape(node, modifiers.distinct)?,
        order_by: modifiers.order_by,
        order_by_all: modifiers.order_by_all,
        limit: modifiers.limit,
        // DuckDB has no row-locking clause, so the neutral shape carries none.
        locking: Vec::new(),
    })
}

/// A set-operation operand. Mirrors [`pg_set_operand_shape`](crate::pg): an operand that
/// carries its own `WITH`/`ORDER BY`/`LIMIT` keeps them under a [`SetShape::Query`]
/// wrapper; a clause-free operand stays the flat set shape (DISTINCT belongs to the select
/// body, so it does not trigger wrapping).
fn duckdb_set_operand_shape(node: &Value) -> Result<SetShape, String> {
    let modifiers = collect_modifiers(node)?;
    let with = duckdb_with_shape(node)?;
    if with.is_some()
        || !modifiers.order_by.is_empty()
        || modifiers.order_by_all.is_some()
        || modifiers.limit != empty_limit()
    {
        Ok(SetShape::Query(Box::new(QueryShape {
            with,
            body: duckdb_body_shape(node, modifiers.distinct)?,
            order_by: modifiers.order_by,
            order_by_all: modifiers.order_by_all,
            limit: modifiers.limit,
            // DuckDB has no row-locking clause, so the neutral shape carries none.
            locking: Vec::new(),
        })))
    } else {
        duckdb_body_shape(node, modifiers.distinct)
    }
}

fn duckdb_with_shape(node: &Value) -> Result<Option<WithShape>, String> {
    let entries = arr_field(field(node, "cte_map"), "map");
    if entries.is_empty() {
        return Ok(None);
    }
    let ctes = entries
        .iter()
        .map(|entry| {
            let value = field(entry, "value");
            // 1.5.4 serializes every CTE as CTE_MATERIALIZE_DEFAULT (both explicit
            // spellings erased — CTEs materialize by default in 1.x); the arms stay
            // for older/future serializations, and the erased-hint divergence is a
            // narrated allowlist entry above.
            // 1.5.4 serializes every CTE as CTE_MATERIALIZE_DEFAULT (both explicit
            // spellings erased - CTEs materialize by default in 1.x); the arms stay
            // for older/future serializations, and the erased-hint divergence is a
            // narrated allowlist entry.
            let materialized = match str_field(value, "materialized") {
                "CTE_MATERIALIZE_ALWAYS" => Some(true),
                "CTE_MATERIALIZE_NEVER" => Some(false),
                _ => None,
            };
            // DuckDB CTE bodies are always queries (a DML body is `Parser Error: A
            // CTE needs a SELECT`, probed on 1.5.4), so only the query arm arises.
            Ok(CteShape {
                name: str_field(entry, "key").to_owned(),
                columns: string_list(field(value, "aliases")),
                materialized,
                body: CteBodyShape::Query(Box::new(duckdb_query_shape(field(
                    field(value, "query"),
                    "node",
                ))?)),
            })
        })
        .collect::<Result<_, String>>()?;
    // `json_serialize_sql` discards the `RECURSIVE` keyword (a `WITH RECURSIVE`
    // serializes identically to a plain `WITH`), so the only recoverable value is
    // non-recursive; a genuine `WITH RECURSIVE` in the comparable subset is an
    // allowlisted residual (module docs), not a shape the mapping can recover.
    Ok(Some(WithShape {
        recursive: false,
        ctes,
    }))
}

/// The body of a query node. Detects DuckDB's `VALUES` desugaring (`SELECT * FROM
/// <expression_list>`) before falling back to the plain select / set-operation shapes.
fn duckdb_body_shape(node: &Value, distinct: bool) -> Result<SetShape, String> {
    match str_field(node, "type") {
        "SELECT_NODE" => {
            if let Some(rows) = duckdb_values_wrapper(node)? {
                return Ok(SetShape::Values(rows));
            }
            Ok(SetShape::Select(duckdb_select_shape(node, distinct)?))
        }
        "SET_OPERATION_NODE" => {
            if distinct {
                return Err("DISTINCT modifier on a set operation".into());
            }
            // DuckDB folds the name-matched modifier into `setop_type` (`UNION_BY_NAME`,
            // distinct from `UNION`) while keeping `setop_all` orthogonal (probed on
            // 1.5.4); the neutral shape splits it back into `op` + `by_name`. `BY NAME`
            // is UNION-only, so there is no `INTERSECT_BY_NAME`/`EXCEPT_BY_NAME` to map.
            let (op, by_name) = match str_field(node, "setop_type") {
                "UNION" => (SetOpShape::Union, false),
                "UNION_BY_NAME" => (SetOpShape::Union, true),
                "INTERSECT" => (SetOpShape::Intersect, false),
                "EXCEPT" => (SetOpShape::Except, false),
                other => return Err(format!("unsupported set operation {other}")),
            };
            let all = field(node, "setop_all").as_bool().unwrap_or(false);
            let left = duckdb_set_operand_shape(field(node, "left"))?;
            let right = duckdb_set_operand_shape(field(node, "right"))?;
            // DuckDB serializes UNION chains right-associatively (`a UNION (b UNION c)`,
            // and it right-canonicalizes even explicit left-parens; probed on 1.5.4),
            // while our parser — like standard SQL / PostgreSQL — is left-associative.
            // UNION is associative, so a homogeneous right-leaning chain (identical
            // `op`/`all`/`by_name` at every level) re-linearizes into the left-
            // associative shape we build, mirroring the n-ary CONJUNCTION re-nesting.
            // INTERSECT/EXCEPT serialize left-leaning already (probed on 1.5.4) and are
            // non-associative, so they are never rotated — a genuine associativity
            // divergence there would still surface. A parenthesized operand carrying its
            // own clauses maps to `SetShape::Query`, not a bare `SetOperation`, so it
            // correctly terminates the flattening instead of being absorbed.
            if op == SetOpShape::Union {
                Ok(left_associative_union_chain(op, all, by_name, left, right))
            } else {
                Ok(SetShape::SetOperation {
                    op,
                    all,
                    by_name,
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
        }
        other => Err(format!("unsupported query node type {other}")),
    }
}

/// Re-linearize a DuckDB UNION operation into the left-associative shape our parser
/// builds. DuckDB serializes UNION chains right-associatively while we (and standard
/// SQL / PostgreSQL) build them left-associatively; UNION is associative, so flattening
/// the homogeneous chain and rebuilding it left-first is shape-preserving. Mirrors
/// [`duckdb_conjunction_shape`]'s n-ary re-nesting.
fn left_associative_union_chain(
    op: SetOpShape,
    all: bool,
    by_name: bool,
    left: SetShape,
    right: SetShape,
) -> SetShape {
    let mut operands = Vec::new();
    collect_union_operands(op, all, by_name, left, &mut operands);
    collect_union_operands(op, all, by_name, right, &mut operands);
    let mut iter = operands.into_iter();
    let mut acc = iter
        .next()
        .expect("a set operation always has a left operand");
    for operand in iter {
        acc = SetShape::SetOperation {
            op,
            all,
            by_name,
            left: Box::new(acc),
            right: Box::new(operand),
        };
    }
    acc
}

/// In-order collect the operands of a homogeneous UNION chain (identical
/// `op`/`all`/`by_name`). A differing operator, an `all`/`by_name` mismatch, or a
/// clause-bearing `SetShape::Query` operand is not part of the chain and is pushed
/// whole as a single operand.
fn collect_union_operands(
    op: SetOpShape,
    all: bool,
    by_name: bool,
    node: SetShape,
    out: &mut Vec<SetShape>,
) {
    match node {
        SetShape::SetOperation {
            op: node_op,
            all: node_all,
            by_name: node_by_name,
            left,
            right,
        } if node_op == op && node_all == all && node_by_name == by_name => {
            collect_union_operands(op, all, by_name, *left, out);
            collect_union_operands(op, all, by_name, *right, out);
        }
        other => out.push(other),
    }
}

/// `Some(rows)` when `node` is DuckDB's desugaring of a `VALUES` clause: a `SELECT` of a
/// single bare `*` over an `EXPRESSION_LIST` table with no other clauses.
fn duckdb_values_wrapper(node: &Value) -> Result<Option<Vec<Vec<ValuesItemShape>>>, String> {
    let projection = arr_field(node, "select_list");
    let is_bare_star = projection.len() == 1
        && class(&projection[0]) == "STAR"
        && !field(&projection[0], "columns").as_bool().unwrap_or(false)
        && !has_wildcard_modifiers(&projection[0])
        && str_field(&projection[0], "relation_name").is_empty();
    let from = field(node, "from_table");
    if !is_bare_star
        || str_field(from, "type") != "EXPRESSION_LIST"
        || !field(node, "where_clause").is_null()
        || !arr_field(node, "group_expressions").is_empty()
        || !field(node, "having").is_null()
        || !field(node, "qualify").is_null()
    {
        return Ok(None);
    }
    let rows = arr_field(from, "values")
        .iter()
        .map(|row| {
            row.as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .map(|item| Ok(ValuesItemShape::Expr(duckdb_expr_shape(item)?)))
                .collect::<Result<_, String>>()
        })
        .collect::<Result<_, String>>()?;
    Ok(Some(rows))
}

fn duckdb_select_shape(node: &Value, distinct: bool) -> Result<SelectShape, String> {
    if !field(node, "sample").is_null() {
        return Err("TABLESAMPLE has no neutral shape".into());
    }
    // `GROUP BY ALL` serializes as `FORCE_AGGREGATES` with an empty key list —
    // the engine's own mode framing (probed on 1.5.4). A `FORCE_AGGREGATES` with
    // keys would be a serialization we have never observed; skip it rather than
    // guess a shape.
    let group_by_all = match str_field(node, "aggregate_handling") {
        "" | "STANDARD_HANDLING" => false,
        "FORCE_AGGREGATES"
            if arr_field(node, "group_expressions").is_empty()
                && arr_field(node, "group_sets").is_empty() =>
        {
            true
        }
        other => return Err(format!("unsupported aggregate handling {other}")),
    };

    Ok(SelectShape {
        distinct,
        projection: arr_field(node, "select_list")
            .iter()
            .map(duckdb_select_item_shape)
            .collect::<Result<_, _>>()?,
        from: duckdb_from_shape(field(node, "from_table"))?,
        selection: opt_expr(field(node, "where_clause"))?,
        group_by: duckdb_group_by_shape(node)?,
        // DuckDB's grammar has no `GROUP BY DISTINCT` grouping-set quantifier.
        group_by_distinct: false,
        group_by_all,
        having: opt_expr(field(node, "having"))?,
        // A first-class SELECT_NODE field in DuckDB's serialization, mapped to the
        // neutral `qualify` member `duckdb-qualify-clause` added.
        qualify: opt_expr(field(node, "qualify"))?.map(Box::new),
    })
}

/// Map a plain `GROUP BY`. `ROLLUP`/`CUBE`/`GROUPING SETS` (encoded via non-trivial
/// `group_sets`) have no neutral shape here yet and Ansi rejects them, so they are a
/// counted skip rather than a mis-map onto plain grouping.
fn duckdb_group_by_shape(node: &Value) -> Result<Vec<GroupByItemShape>, String> {
    let exprs = arr_field(node, "group_expressions");
    if exprs.is_empty() {
        return Ok(Vec::new());
    }
    let sets = arr_field(node, "group_sets");
    let simple = sets.len() == 1
        && sets[0]
            .as_array()
            .map(|set| {
                set.len() == exprs.len()
                    && set
                        .iter()
                        .enumerate()
                        .all(|(index, value)| value.as_u64() == Some(index as u64))
            })
            .unwrap_or(false);
    if !simple {
        return Err("ROLLUP/CUBE/GROUPING SETS have no neutral shape".into());
    }
    exprs
        .iter()
        .map(|expr| Ok(GroupByItemShape::Expr(duckdb_expr_shape(expr)?)))
        .collect()
}

fn duckdb_select_item_shape(item: &Value) -> Result<SelectItemShape, String> {
    if class(item) == "STAR" {
        // `COLUMNS(...)` as a projection item is an *expression* on our side
        // (`Expr::Columns` inside `SelectItem::Expr`), so it maps through the
        // expression arm like any other item — including its alias.
        if field(item, "columns").as_bool().unwrap_or(false) {
            let alias = str_field(item, "alias");
            return Ok(SelectItemShape::Expr {
                expr: duckdb_expr_shape(item)?,
                alias: (!alias.is_empty()).then(|| alias.to_owned()),
            });
        }
        if has_wildcard_modifiers(item) {
            return Err(
                "SELECT * EXCLUDE/REPLACE/RENAME modifiers have no neutral select-item \
                 shape yet (duckdb-structural-oracle-select owns the shape fields)"
                    .into(),
            );
        }
        let relation = str_field(item, "relation_name");
        return Ok(if relation.is_empty() {
            SelectItemShape::Wildcard
        } else {
            SelectItemShape::QualifiedWildcard(vec![relation.to_owned()])
        });
    }
    let alias = str_field(item, "alias");
    Ok(SelectItemShape::Expr {
        expr: duckdb_expr_shape(item)?,
        alias: (!alias.is_empty()).then(|| alias.to_owned()),
    })
}

/// Whether a `STAR` node carries a DuckDB wildcard modifier (`EXCLUDE`/`REPLACE`/
/// `RENAME`) or a `COLUMNS(<pattern>)` argument. Our parser reads all of these
/// (`duckdb-select-star-modifiers`), but the modifier lists have no neutral-shape
/// fields yet — `duckdb-structural-oracle-select` owns that parity — so a
/// modifier-bearing star stays a counted skip, never a mis-map onto a plain wildcard.
fn has_wildcard_modifiers(star: &Value) -> bool {
    !arr_field(star, "exclude_list").is_empty()
        || !arr_field(star, "replace_list").is_empty()
        || !arr_field(star, "rename_list").is_empty()
        || !arr_field(star, "qualified_exclude_list").is_empty()
        || !field(star, "expr").is_null()
}

/// Map DuckDB's single `from_table` node to our `Vec<TableWithJoinsShape>`.
fn duckdb_from_shape(from: &Value) -> Result<Vec<TableWithJoinsShape>, String> {
    match str_field(from, "type") {
        "EMPTY" => Ok(Vec::new()),
        "JOIN" => {
            let (relation, joins) = duckdb_flatten_join(from)?;
            Ok(vec![TableWithJoinsShape { relation, joins }])
        }
        _ => Ok(vec![TableWithJoinsShape {
            relation: duckdb_table_factor_shape(from)?,
            joins: Vec::new(),
        }]),
    }
}

/// Flatten DuckDB's binary `JOIN` tree down its left spine into a relation plus a
/// left-deep join list, matching our `TableWithJoins { relation, joins }`.
fn duckdb_flatten_join(node: &Value) -> Result<(TableFactorShape, Vec<JoinShape>), String> {
    // DuckDB serializes both `FROM a, b` and `a CROSS JOIN b` as one `CROSS`-ref
    // `JOIN` node, so a `CROSS` join is an irrecoverable comma/cross ambiguity — a
    // counted skip rather than a guess (module docs).
    if str_field(node, "ref_type") == "CROSS" {
        return Err("comma/CROSS JOIN ambiguity (DuckDB serializer conflates them)".into());
    }
    let operator = duckdb_join_operator(node)?;
    let right = duckdb_table_factor_shape(field(node, "right"))?;
    let left = field(node, "left");
    if str_field(left, "type") == "JOIN" {
        let (relation, mut joins) = duckdb_flatten_join(left)?;
        joins.push(JoinShape {
            relation: right,
            operator,
        });
        Ok((relation, joins))
    } else {
        Ok((
            duckdb_table_factor_shape(left)?,
            vec![JoinShape {
                relation: right,
                operator,
            }],
        ))
    }
}

fn duckdb_join_operator(node: &Value) -> Result<JoinOperatorShape, String> {
    let constraint = if str_field(node, "ref_type") == "NATURAL" {
        JoinConstraintShape::Natural
    } else if !field(node, "condition").is_null() {
        JoinConstraintShape::On(duckdb_expr_shape(field(node, "condition"))?)
    } else {
        let using = arr_field(node, "using_columns");
        if using.is_empty() {
            JoinConstraintShape::None
        } else {
            JoinConstraintShape::Using {
                columns: string_slice(using),
                alias: None,
            }
        }
    };
    match str_field(node, "ref_type") {
        // `ref_type` is orthogonal to `join_type` in DuckDB's tree; ASOF composes
        // with all four sides and always carries an ON/USING constraint (both
        // parse-enforced by the engine, mirrored by our `ASOF` grammar arm).
        "ASOF" => {
            // ASOF composes with the four sides and with the SEMI/ANTI join_types
            // (`ASOF SEMI JOIN`, `asof: true`), always carrying an ON/USING constraint.
            return Ok(match str_field(node, "join_type") {
                "INNER" => JoinOperatorShape::AsOf(AsOfJoinKind::Inner, constraint),
                "LEFT" => JoinOperatorShape::AsOf(AsOfJoinKind::Left, constraint),
                "RIGHT" => JoinOperatorShape::AsOf(AsOfJoinKind::Right, constraint),
                "OUTER" | "FULL" => JoinOperatorShape::AsOf(AsOfJoinKind::Full, constraint),
                "SEMI" => JoinOperatorShape::Semi(true, SemiAntiSide::Sideless, constraint),
                "ANTI" => JoinOperatorShape::Anti(true, SemiAntiSide::Sideless, constraint),
                other => return Err(format!("unsupported ASOF join type {other}")),
            });
        }
        // `POSITIONAL JOIN` never carries a constraint or side (engine
        // parse-rejects both), so the serialized node is always the bare pairing.
        "POSITIONAL" => return Ok(JoinOperatorShape::Positional),
        _ => {}
    }
    match str_field(node, "join_type") {
        "INNER" => Ok(JoinOperatorShape::Inner(constraint)),
        "LEFT" => Ok(JoinOperatorShape::LeftOuter(constraint)),
        "RIGHT" => Ok(JoinOperatorShape::RightOuter(constraint)),
        "OUTER" | "FULL" => Ok(JoinOperatorShape::FullOuter(constraint)),
        // SEMI/ANTI under the REGULAR or NATURAL ref-type (`asof: false`); the ASOF
        // composition is handled above. The `constraint` is the ON/USING (REGULAR) or
        // `Natural` (NATURAL SEMI JOIN) already computed above.
        "SEMI" => Ok(JoinOperatorShape::Semi(
            false,
            SemiAntiSide::Sideless,
            constraint,
        )),
        "ANTI" => Ok(JoinOperatorShape::Anti(
            false,
            SemiAntiSide::Sideless,
            constraint,
        )),
        // MARK / SINGLE joins (internal DuckDB rewrites) have no neutral shape.
        other => Err(format!("unsupported join type {other}")),
    }
}

fn duckdb_table_factor_shape(node: &Value) -> Result<TableFactorShape, String> {
    match str_field(node, "type") {
        "BASE_TABLE" => {
            if !field(node, "sample").is_null() {
                return Err("TABLESAMPLE has no neutral shape".into());
            }
            let mut name = Vec::new();
            for part in ["catalog_name", "schema_name", "table_name"] {
                let text = str_field(node, part);
                if !text.is_empty() {
                    name.push(text.to_owned());
                }
            }
            Ok(TableFactorShape::Table {
                name,
                alias: duckdb_table_alias(node),
                only: false,
                sample: None,
            })
        }
        "SUBQUERY" => {
            if !field(node, "sample").is_null() {
                return Err("TABLESAMPLE has no neutral shape".into());
            }
            Ok(TableFactorShape::Derived {
                lateral: false,
                subquery: Box::new(duckdb_query_shape(field(field(node, "subquery"), "node"))?),
                alias: duckdb_table_alias(node),
            })
        }
        "JOIN" => {
            let (relation, joins) = duckdb_flatten_join(node)?;
            Ok(TableFactorShape::NestedJoin {
                table: Box::new(TableWithJoinsShape { relation, joins }),
                alias: duckdb_table_alias(node),
            })
        }
        // The table-factor pivot operators serialize as one first-class `PIVOT` node
        // (probed on 1.5.4) — real tree parity, not a desugar match. The *statement*
        // spelling never reaches here: `json_serialize_sql` refuses a top-level or
        // parenthesized `PIVOT` statement ("Only SELECT statements…"), making those
        // a counted skip in the error branch above.
        "PIVOT" => duckdb_pivot_shape(node),
        // `range(...)` and friends: table functions have no neutral shape here (Ansi
        // rejects them).
        other => Err(format!("unsupported table factor {other}")),
    }
}

/// Map DuckDB's `PIVOT` `from_table` node — one node class for both operators,
/// discriminated by its top-level `unpivot_names` (empty = pivot, non-empty =
/// unpivot; probed on 1.5.4).
fn duckdb_pivot_shape(node: &Value) -> Result<TableFactorShape, String> {
    if !field(node, "sample").is_null() {
        return Err("TABLESAMPLE has no neutral shape".into());
    }
    let source = Box::new(duckdb_table_factor_shape(field(node, "source"))?);
    let alias = duckdb_table_alias(node);
    let include_nulls = field(node, "include_nulls").as_bool().unwrap_or(false);
    let pivots = arr_field(node, "pivots");
    let [pivot, ..] = pivots else {
        return Err("PIVOT with no FOR head".into());
    };

    let value_names = string_list(field(node, "unpivot_names"));
    if value_names.is_empty() {
        // PIVOT: `<source> PIVOT (<aggregates> FOR <col> IN (…) [<col> IN (…)]…
        // [GROUP BY …])` — one serialized `pivots` element per column head.
        let aggregates = arr_field(node, "aggregates")
            .iter()
            .map(duckdb_pivot_expr_shape)
            .collect::<Result<_, _>>()?;
        let pivot_on = pivots
            .iter()
            .map(duckdb_pivot_column_shape)
            .collect::<Result<_, _>>()?;
        Ok(TableFactorShape::Pivot(Box::new(PivotShape {
            source,
            aggregates,
            pivot_on,
            group_by: duckdb_pivot_groups(node),
            alias,
        })))
    } else {
        // UNPIVOT: `<source> UNPIVOT [… NULLS] (<value> FOR <name> IN (<cols>))`.
        // The engine lowers the unpivoted column references to VARCHAR constants
        // naming them (probed on 1.5.4); the mapping restores the column reading
        // our faithful parse keeps — a category-1 normalization.
        if !arr_field(node, "aggregates").is_empty() {
            return Err("UNPIVOT with aggregates has no neutral shape".into());
        }
        let columns = arr_field(pivot, "entries")
            .iter()
            .map(|entry| {
                if !field(entry, "star_expr").is_null() {
                    return Err(
                        "UNPIVOT COLUMNS(...) star expansion has no neutral shape yet \
                         (duckdb-select-star-modifiers)"
                            .to_owned(),
                    );
                }
                let columns = arr_field(entry, "values")
                    .iter()
                    .map(|value| {
                        let name = field(value, "value")
                            .as_str()
                            .ok_or("UNPIVOT column entry is not a name constant")?;
                        // The serializer conflates an identifier entry with a
                        // string-spelled one (`jan` and `'jan'` both arrive as the
                        // VARCHAR); the mapping picks the identifier reading (the
                        // dominant surface) — except the empty string, which can
                        // never be an identifier, so the literal reading is the
                        // only sound one (`IN ('')` is in the vendored corpus).
                        Ok(if name.is_empty() {
                            ExprShape::Literal(LiteralShape::String(String::new()))
                        } else {
                            ExprShape::Column(vec![name.to_owned()])
                        })
                    })
                    .collect::<Result<_, String>>()?;
                Ok(UnpivotColumnShape {
                    columns,
                    alias: duckdb_pivot_alias(entry),
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(TableFactorShape::Unpivot(Box::new(UnpivotShape {
            source,
            value: value_names,
            name: string_list(field(pivot, "unpivot_names")),
            columns,
            include_nulls,
            alias,
        })))
    }
}

/// One serialized pivot column head (`pivots[i]`): its column expression and its
/// `IN` source — the written value list, or the ENUM name the `pivot_enum` field
/// carries for the `IN <enum>` form.
fn duckdb_pivot_column_shape(pivot: &Value) -> Result<PivotColumnShape, String> {
    let exprs = arr_field(pivot, "pivot_expressions");
    // A single column per head maps directly; the multi-column row form
    // (`FOR (a, b) IN …`) has no corpus occurrence, so it stays a counted skip
    // rather than a guessed row shape.
    let [column] = exprs else {
        return Err(format!("PIVOT FOR head with {} columns", exprs.len()));
    };
    let enum_source = str_field(pivot, "pivot_enum");
    let values = arr_field(pivot, "entries")
        .iter()
        .map(|entry| {
            let values = arr_field(entry, "values");
            let [value] = values else {
                return Err(format!("PIVOT IN entry with {} values", values.len()));
            };
            if !field(entry, "star_expr").is_null() {
                return Err("PIVOT IN entry with a star expression".into());
            }
            Ok(PivotExprShape {
                expr: duckdb_constant_shape(value)?,
                alias: duckdb_pivot_alias(entry),
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(PivotColumnShape {
        expr: duckdb_expr_shape(column)?,
        values,
        enum_source: (!enum_source.is_empty()).then(|| enum_source.to_owned()),
    })
}

/// An aliased pivot expression (a paren-list aggregate) from its serialized node,
/// whose `alias` field rides the expression itself.
fn duckdb_pivot_expr_shape(expr: &Value) -> Result<PivotExprShape, String> {
    Ok(PivotExprShape {
        expr: duckdb_expr_shape(expr)?,
        alias: duckdb_pivot_alias(expr),
    })
}

/// A pivot node's optional `alias` field (`""` = unwritten).
fn duckdb_pivot_alias(node: &Value) -> Option<String> {
    let alias = str_field(node, "alias");
    (!alias.is_empty()).then(|| alias.to_owned())
}

/// The `GROUP BY` column names a serialized pivot carries (`groups`, plain strings)
/// restored to the column references our parse keeps.
fn duckdb_pivot_groups(node: &Value) -> Vec<ExprShape> {
    string_list(field(node, "groups"))
        .into_iter()
        .map(|name| ExprShape::Column(vec![name]))
        .collect()
}

/// The optional table alias (`alias` name + `column_name_alias` list) of a `from_table`.
fn duckdb_table_alias(node: &Value) -> Option<AliasShape> {
    let name = str_field(node, "alias");
    if name.is_empty() {
        return None;
    }
    Some(AliasShape {
        name: name.to_owned(),
        columns: string_list(field(node, "column_name_alias")),
    })
}

// ---- expressions ----------------------------------------------------------

fn opt_expr(value: &Value) -> Result<Option<ExprShape>, String> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(duckdb_expr_shape(value)?))
    }
}

fn duckdb_expr_shape(expr: &Value) -> Result<ExprShape, String> {
    match class(expr) {
        "COLUMN_REF" => Ok(ExprShape::Column(string_list(field(expr, "column_names")))),
        "CONSTANT" => duckdb_constant_shape(field(expr, "value")),
        "COMPARISON" => Ok(ExprShape::BinaryOp {
            left: Box::new(duckdb_expr_shape(field(expr, "left"))?),
            op: duckdb_comparison_op(str_field(expr, "type"))?,
            right: Box::new(duckdb_expr_shape(field(expr, "right"))?),
        }),
        "CONJUNCTION" => duckdb_conjunction_shape(expr),
        "FUNCTION" => duckdb_function_shape(expr),
        "OPERATOR" => duckdb_operator_shape(expr),
        "CAST" => duckdb_cast_shape(expr),
        "SUBQUERY" => duckdb_subquery_shape(expr),
        "CASE" => duckdb_case_shape(expr),
        // `BETWEEN` and the `COMPARE_IN` in-list (below) mirror our side's
        // `Expr::Between`/`Expr::InList => Unmapped` — an explicit gap, not a mis-map.
        "BETWEEN" => Ok(ExprShape::Unmapped),
        // A `STAR` node in expression position is DuckDB's `COLUMNS(...)` star
        // expansion (`min(COLUMNS(*))`, `ORDER BY COLUMNS('re')`, `COLUMNS(*) + 1`),
        // which our parser reads as the dedicated `Expr::Columns` node
        // (`duckdb-select-star-modifiers`).
        "STAR" => duckdb_columns_star_shape(expr),
        // DuckDB parses *every* `->` as a `LAMBDA` at the syntactic level (a json
        // accessor `a -> k` and a lambda `x -> x + 1` both serialize as this class;
        // the json-vs-lambda disambiguation is deferred to bind time, where a lambda
        // requires an unqualified-name parameter list). Our parser applies that same
        // parameter-shape split at parse time (`duckdb-lambda-expressions`), and the
        // mapping mirrors it: a name-list `lhs` is the `Lambda` shape, any other
        // `lhs` normalizes to the `JsonGet` binary op our fall-through parse
        // produces. `(x, y)` and `ROW(x, y)` serialize identically (both a `row`
        // call) — which is exactly why both spell the *same* lambda on our side too;
        // the spelling tag sits outside the neutral shape (ADR-0011).
        "LAMBDA" => {
            let lhs = field(expr, "lhs");
            let body = duckdb_expr_shape(field(expr, "expr"))?;
            match duckdb_lambda_params(lhs) {
                Some(params) => Ok(ExprShape::Lambda {
                    params,
                    body: Box::new(body),
                }),
                None => Ok(ExprShape::BinaryOp {
                    left: Box::new(duckdb_expr_shape(lhs)?),
                    op: BinaryOperatorShape::JsonGet,
                    right: Box::new(body),
                }),
            }
        }
        // Any other class (window functions, struct/list access, …) is an honest gap
        // that matches our side's `Unmapped` for the same construct (a windowed call
        // maps to `Unmapped` there too — load-bearing for QUALIFY predicates, whose
        // window calls sit inside the compared subset).
        _ => Ok(ExprShape::Unmapped),
    }
}

/// Map a `STAR` node in expression position to the neutral `COLUMNS(...)` shape
/// ([`ExprShape::Columns`]).
///
/// The engine reuses one `STAR` node for every star surface, so this arm narrows:
/// only the `columns: true` selector without `EXCLUDE`/`REPLACE`/`RENAME` modifiers
/// is in the neutral subset (the modifier lists have no shape fields yet —
/// `duckdb-structural-oracle-select` owns that parity — and a plain `*` in
/// expression position has no reading on our side at all). The `COLUMNS(λ)` lambda
/// form desugars in the serialized tree to `list_filter(<bare *>, λ)` (probed on
/// 1.5.4); the mapping unwraps that back to the lambda pattern our parse carries
/// (ADR-0015 representation equivalence). The serializer discards whether the source
/// wrote `COLUMNS(c -> …)` or the explicit `COLUMNS(list_filter(*, c -> …))`, so the
/// mapping picks the lambda reading — the same lossy-spelling policy as the
/// `list_value` desugar-normalization.
fn duckdb_columns_star_shape(star: &Value) -> Result<ExprShape, String> {
    if !field(star, "columns").as_bool().unwrap_or(false) {
        return Err("a plain `*` in expression position has no neutral shape".into());
    }
    if !arr_field(star, "exclude_list").is_empty()
        || !arr_field(star, "replace_list").is_empty()
        || !arr_field(star, "rename_list").is_empty()
        || !arr_field(star, "qualified_exclude_list").is_empty()
    {
        return Err(
            "COLUMNS(* EXCLUDE/REPLACE/RENAME ...) modifiers have no neutral shape \
             yet (duckdb-structural-oracle-select owns the shape fields)"
                .into(),
        );
    }
    // The qualified star `COLUMNS(t.*)` rides the STAR node's single
    // `relation_name` slot (one part only; the engine rejects `s.t.*`).
    let relation = str_field(star, "relation_name");
    let qualifier = (!relation.is_empty()).then(|| vec![relation.to_owned()]);
    let pattern = field(star, "expr");
    if pattern.is_null() {
        return Ok(ExprShape::Columns {
            qualifier,
            pattern: None,
        });
    }
    let pattern = columns_lambda_desugar(pattern).unwrap_or(pattern);
    Ok(ExprShape::Columns {
        qualifier,
        pattern: Some(Box::new(duckdb_expr_shape(pattern)?)),
    })
}

/// The lambda inside DuckDB's serialized `COLUMNS(λ)` desugaring — a
/// `list_filter(<bare *>, λ)` call whose first child is the plain whole-projection
/// star — or `None` when the pattern is not that desugar.
fn columns_lambda_desugar(pattern: &Value) -> Option<&Value> {
    if class(pattern) != "FUNCTION"
        || str_field(pattern, "function_name") != "list_filter"
        || field(pattern, "is_operator").as_bool().unwrap_or(false)
    {
        return None;
    }
    match arr_field(pattern, "children") {
        [star, lambda]
            if class(star) == "STAR"
                && !field(star, "columns").as_bool().unwrap_or(false)
                && str_field(star, "relation_name").is_empty()
                && !has_wildcard_modifiers(star)
                && class(lambda) == "LAMBDA" =>
        {
            Some(lambda)
        }
        _ => None,
    }
}

/// The lambda parameter names of a serialized `LAMBDA` node's `lhs`, when it has the
/// shape DuckDB's binder admits — the same rule the parser applies to a `->` left
/// operand (`lambda_params` in `squonk::parser::expr`): a single unqualified
/// `COLUMN_REF`, or a `row(…)` call whose every child is one. `None` keeps the
/// JSON-arrow reading.
fn duckdb_lambda_params(lhs: &Value) -> Option<Vec<String>> {
    let single_name = |node: &Value| -> Option<String> {
        if class(node) != "COLUMN_REF" {
            return None;
        }
        match string_list(field(node, "column_names")).as_mut_slice() {
            [name] => Some(std::mem::take(name)),
            _ => None,
        }
    };
    if let Some(name) = single_name(lhs) {
        return Some(vec![name]);
    }
    if class(lhs) == "FUNCTION"
        && str_field(lhs, "function_name") == "row"
        && !field(lhs, "is_operator").as_bool().unwrap_or(false)
    {
        let children = arr_field(lhs, "children");
        if children.is_empty() {
            return None;
        }
        return children.iter().map(single_name).collect();
    }
    None
}

fn duckdb_constant_shape(value: &Value) -> Result<ExprShape, String> {
    if field(value, "is_null").as_bool().unwrap_or(false) {
        return Ok(ExprShape::Literal(LiteralShape::Null));
    }
    let inner = field(value, "value");
    match str_field(field(value, "type"), "id") {
        // A `HUGEINT` beyond the i64/u64 JSON range serializes as the two-limb
        // `{upper, lower}` object `hugeint_i128` decodes; a `UHUGEINT` above i128::MAX
        // has no signed decode and stays an honest gap.
        "HUGEINT" => Ok(
            match integer_text(inner).or_else(|| hugeint_i128(inner).map(|raw| raw.to_string())) {
                Some(text) => ExprShape::Literal(LiteralShape::Integer(text)),
                None => ExprShape::Unmapped,
            },
        ),
        "TINYINT" | "SMALLINT" | "INTEGER" | "BIGINT" | "UTINYINT" | "USMALLINT" | "UINTEGER"
        | "UBIGINT" | "UHUGEINT" => Ok(match integer_text(inner) {
            Some(text) => ExprShape::Literal(LiteralShape::Integer(text)),
            // Beyond the i64/u64 range serde_json cannot represent it exactly — an honest
            // gap rather than a lossy `f64` round-trip.
            None => ExprShape::Unmapped,
        }),
        "DECIMAL" => {
            let scale = field(field(value, "type"), "type_info")
                .get("scale")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            // A narrow DECIMAL's raw value is a plain JSON integer; a wide one
            // (width > 18) is the two-limb `{upper, lower}` hugeint object.
            let raw = inner
                .as_i64()
                .map(i128::from)
                .or_else(|| hugeint_i128(inner));
            Ok(match raw {
                Some(raw) => ExprShape::Literal(LiteralShape::Float(decimal_text(raw, scale))),
                None => ExprShape::Unmapped,
            })
        }
        "VARCHAR" => Ok(ExprShape::Literal(LiteralShape::String(
            inner.as_str().unwrap_or("").to_owned(),
        ))),
        "BOOLEAN" => Ok(ExprShape::Literal(LiteralShape::Boolean(
            inner.as_bool().unwrap_or(false),
        ))),
        // FLOAT/DOUBLE arrive as a JSON number whose original source text is
        // unrecoverable, and temporal/blob constants have no neutral literal — both are
        // honest gaps (our side maps its temporal literals to `Unmapped` too).
        _ => Ok(ExprShape::Unmapped),
    }
}

fn duckdb_comparison_op(kind: &str) -> Result<BinaryOperatorShape, String> {
    Ok(match kind {
        "COMPARE_EQUAL" => BinaryOperatorShape::Eq,
        "COMPARE_NOTEQUAL" => BinaryOperatorShape::NotEq,
        "COMPARE_LESSTHAN" => BinaryOperatorShape::Lt,
        "COMPARE_GREATERTHAN" => BinaryOperatorShape::Gt,
        "COMPARE_LESSTHANOREQUALTO" => BinaryOperatorShape::LtEq,
        "COMPARE_GREATERTHANOREQUALTO" => BinaryOperatorShape::GtEq,
        "COMPARE_DISTINCT_FROM" => BinaryOperatorShape::IsDistinctFrom,
        "COMPARE_NOT_DISTINCT_FROM" => BinaryOperatorShape::IsNotDistinctFrom,
        other => return Err(format!("unsupported comparison {other}")),
    })
}

/// DuckDB flattens `a AND b AND c` into one n-ary `CONJUNCTION`; re-nest it
/// left-associatively to match our binary `BinaryOp` tree.
fn duckdb_conjunction_shape(expr: &Value) -> Result<ExprShape, String> {
    let op = match str_field(expr, "type") {
        "CONJUNCTION_AND" => BinaryOperatorShape::And,
        "CONJUNCTION_OR" => BinaryOperatorShape::Or,
        other => return Err(format!("unsupported conjunction {other}")),
    };
    let children = arr_field(expr, "children");
    let mut iter = children.iter();
    let first = iter.next().ok_or_else(|| "empty conjunction".to_string())?;
    let mut acc = duckdb_expr_shape(first)?;
    for child in iter {
        acc = ExprShape::BinaryOp {
            left: Box::new(acc),
            op,
            right: Box::new(duckdb_expr_shape(child)?),
        };
    }
    Ok(acc)
}

fn duckdb_function_shape(expr: &Value) -> Result<ExprShape, String> {
    // Aggregate/window modifiers have no neutral shape (our `FunctionShape` compares only
    // name/args/wildcard); a call bearing them is a counted skip, mirroring the PG side's
    // rejection of modifier-bearing calls.
    if !field(expr, "filter").is_null() {
        return Err("aggregate FILTER has no neutral shape".into());
    }
    if !arr_field(field(expr, "order_bys"), "orders").is_empty() {
        return Err("aggregate ORDER BY has no neutral shape".into());
    }
    if field(expr, "distinct").as_bool().unwrap_or(false) {
        return Err("DISTINCT aggregate has no neutral shape".into());
    }

    let name = str_field(expr, "function_name");
    let children = arr_field(expr, "children");

    if field(expr, "is_operator").as_bool().unwrap_or(false) {
        // Operator-functions normalize back to the operator shapes (category 1).
        if children.len() == 2 {
            if let Some(op) = binary_operator_function(name) {
                return Ok(ExprShape::BinaryOp {
                    left: Box::new(duckdb_expr_shape(&children[0])?),
                    op,
                    right: Box::new(duckdb_expr_shape(&children[1])?),
                });
            }
        } else if children.len() == 1 {
            // DuckDB pre-folds a unary minus over a numeric literal into a signed
            // constant, so a surviving unary `-`/`+` operates on a non-literal — exactly
            // our `fold_unary` leaves as a `UnaryOp`.
            if let Some(op) = unary_operator_function(name) {
                return Ok(ExprShape::UnaryOp {
                    op,
                    expr: Box::new(duckdb_expr_shape(&children[0])?),
                });
            }
        }
        // Bitwise/integer-divide/power operators (`//`, `<<`, `**`, …) are not in the
        // neutral operator vocabulary — an honest gap.
        return Ok(ExprShape::Unmapped);
    }

    // `count(*)` serializes as the dedicated `count_star`; normalize to the wildcard call.
    if name == "count_star" && children.is_empty() {
        return Ok(ExprShape::Function(FunctionShape {
            name: vec!["count".into()],
            args: Vec::new(),
            wildcard: true,
        }));
    }
    // Collection-literal desugarings normalize back to the real shapes
    // (duckdb-collection-literals; ADR-0015 category 1): `[1, 2]` serializes as
    // `list_value(1, 2)` and `{'a': 1}` as `struct_pack(…)` with each written key
    // preserved verbatim in its child's `alias`. The serializer discards whether the
    // source wrote the literal or an explicit `list_value(…)`/`struct_pack(…)` call,
    // so the mapping picks the literal reading (the dominant surface); an explicit-call
    // text that reaches a both-accept comparison diverges against our `Function` parse
    // and lands in [`DUCKDB_STRUCTURAL_ALLOWLIST`] as a serializer limitation. (`MAP
    // {…}` needs no arm: it serializes as an ordinary `map(…)` call over two
    // `list_value`s — exactly the desugaring our side's map shape emits — so the
    // generic path below already matches it.)
    if name == "list_value" {
        return Ok(ExprShape::Array(
            children
                .iter()
                .map(duckdb_expr_shape)
                .collect::<Result<_, _>>()?,
        ));
    }
    if name == "struct_pack" {
        return Ok(ExprShape::Struct(
            children
                .iter()
                .map(|child| {
                    Ok((
                        str_field(child, "alias").to_owned(),
                        duckdb_expr_shape(child)?,
                    ))
                })
                .collect::<Result<_, String>>()?,
        ));
    }

    // A qualified call (`rn.length()`, `pg_catalog.now()`) serializes its qualifiers
    // into dedicated `catalog`/`schema` fields, not the name — DuckDB's raw parse
    // keeps the dotted spelling qualified (its method-call desugaring to
    // `length(rn)` happens at bind, past the parse-level contract), exactly like our
    // multi-part `ObjectName`. Rebuild the dotted name so the two sides compare
    // (category 1; surfaced by the QUALIFY corpus statements, `duckdb-qualify-clause`).
    // The engine's own default-schema resolution ("main" — e.g. the `MAP {…}` desugar
    // serializes as `main.map`) is bind-time noise a parse never wrote, so it is
    // stripped rather than compared (category 1, `duckdb-collection-literals`); an
    // explicit source qualifier survives because DuckDB preserves it verbatim.
    let mut parts = Vec::new();
    for qualifier in [str_field(expr, "catalog"), str_field(expr, "schema")] {
        if !qualifier.is_empty() && qualifier != "main" {
            parts.push(qualifier.to_owned());
        }
    }
    parts.push(name.to_owned());
    Ok(ExprShape::Function(FunctionShape {
        name: parts,
        args: children
            .iter()
            .map(duckdb_expr_shape)
            .collect::<Result<_, _>>()?,
        wildcard: false,
    }))
}

fn binary_operator_function(name: &str) -> Option<BinaryOperatorShape> {
    Some(match name {
        "+" => BinaryOperatorShape::Plus,
        "-" => BinaryOperatorShape::Minus,
        "*" => BinaryOperatorShape::Multiply,
        "/" => BinaryOperatorShape::Divide,
        "%" => BinaryOperatorShape::Modulo,
        "||" => BinaryOperatorShape::StringConcat,
        _ => return None,
    })
}

fn unary_operator_function(name: &str) -> Option<UnaryOperatorShape> {
    Some(match name {
        "-" => UnaryOperatorShape::Minus,
        "+" => UnaryOperatorShape::Plus,
        _ => return None,
    })
}

fn duckdb_operator_shape(expr: &Value) -> Result<ExprShape, String> {
    let children = arr_field(expr, "children");
    match str_field(expr, "type") {
        "OPERATOR_NOT" => {
            let child = children
                .first()
                .ok_or_else(|| "NOT without operand".to_string())?;
            // `NOT` over an IN-subquery / EXISTS folds into the negated forms our AST
            // uses, matching `expr_shape` (which also drops `negated` on `Exists`).
            if class(child) == "SUBQUERY" {
                if let Some(shape) = duckdb_subquery_shape_negated(child, true)? {
                    return Ok(shape);
                }
            }
            // `NOT` over an in-list stays `Unmapped` (our `Expr::InList` maps to
            // `Unmapped` regardless of negation).
            if class(child) == "OPERATOR" && str_field(child, "type") == "COMPARE_IN" {
                return Ok(ExprShape::Unmapped);
            }
            Ok(ExprShape::UnaryOp {
                op: UnaryOperatorShape::Not,
                expr: Box::new(duckdb_expr_shape(child)?),
            })
        }
        "OPERATOR_IS_NULL" => Ok(ExprShape::IsNull {
            expr: Box::new(duckdb_expr_shape(
                children
                    .first()
                    .ok_or_else(|| "IS NULL without operand".to_string())?,
            )?),
            negated: false,
        }),
        "OPERATOR_IS_NOT_NULL" => Ok(ExprShape::IsNull {
            expr: Box::new(duckdb_expr_shape(
                children
                    .first()
                    .ok_or_else(|| "IS NOT NULL without operand".to_string())?,
            )?),
            negated: true,
        }),
        // `COMPARE_IN` (in-list) mirrors our `Expr::InList => Unmapped`.
        "COMPARE_IN" | "COMPARE_NOT_IN" => Ok(ExprShape::Unmapped),
        // `UNPACK(...)` / `*COLUMNS(...)` wraps a `COLUMNS(...)` star expansion (its
        // child is the STAR node); our parser has no reading for the unpack operator
        // (it is not part of the wildcard-modifier family `duckdb-select-star-modifiers`
        // landed), so the statement is skipped rather than forced to diverge.
        "OPERATOR_UNPACK" => Err("UNPACK(...) star expansion has no neutral shape yet \
             (duckdb-dialect-100-percent-programme: unpack operator unowned)"
            .into()),
        // `COALESCE(a, b, …)` keeps a dedicated operator node in DuckDB's tree; our
        // parser reads it as an ordinary call, so it folds onto the plain function
        // shape (the `list_value` desugar-normalization precedent).
        "OPERATOR_COALESCE" => Ok(ExprShape::Function(FunctionShape {
            name: vec!["coalesce".to_owned()],
            args: children
                .iter()
                .map(duckdb_expr_shape)
                .collect::<Result<_, _>>()?,
            wildcard: false,
        })),
        // The keyword-spelled `ARRAY[a, b]` keeps a dedicated node (unlike the bare
        // `[a, b]`, which desugars to `list_value`); both fold onto the one canonical
        // `Array` shape — the spelling is exactly what the neutral shape ignores
        // (ADR-0011).
        "ARRAY_CONSTRUCTOR" => Ok(ExprShape::Array(
            children
                .iter()
                .map(duckdb_expr_shape)
                .collect::<Result<_, _>>()?,
        )),
        // `base[index]` — children are `[base, index]` (duckdb-collection-literals).
        "ARRAY_EXTRACT" => {
            let [base, index] = children else {
                return Err("ARRAY_EXTRACT without base + index".into());
            };
            Ok(ExprShape::Subscript {
                base: Box::new(duckdb_expr_shape(base)?),
                lower: Some(Box::new(duckdb_expr_shape(index)?)),
                upper: None,
                step: None,
                kind: SubscriptKind::Index,
            })
        }
        // `base[lower:upper]` — children are `[base, lower, upper]`; the three-bound
        // `base[lower:upper:step]` adds a fourth `step` child. An omitted bound (including
        // the stepped `-` open-upper placeholder and an omitted trailing step) is
        // serialized as an empty-LIST-typed constant sentinel (see [`slice_bound`]).
        "ARRAY_SLICE" => match children {
            [base, lower, upper] => Ok(ExprShape::Subscript {
                base: Box::new(duckdb_expr_shape(base)?),
                lower: slice_bound(lower)?,
                upper: slice_bound(upper)?,
                step: None,
                kind: SubscriptKind::Slice,
            }),
            [base, lower, upper, step] => Ok(ExprShape::Subscript {
                base: Box::new(duckdb_expr_shape(base)?),
                lower: slice_bound(lower)?,
                upper: slice_bound(upper)?,
                step: slice_bound(step)?,
                kind: SubscriptKind::SliceWithStep,
            }),
            _ => Err("ARRAY_SLICE without base + bounds".into()),
        },
        // `STRUCT_EXTRACT` (`(s).field` selection) and the rest stay the honest gap
        // matching our side's `Unmapped` for the same construct.
        _ => Ok(ExprShape::Unmapped),
    }
}

/// One `ARRAY_SLICE` bound: `None` for the omitted-bound sentinel — a `CONSTANT`
/// whose value is LIST-typed (`x[2:]` serializes its missing upper bound as an empty
/// LIST constant) — else the bound's own shape. The sentinel cannot collide with a
/// real `[]` bound: an empty list literal serializes as a `list_value()` FUNCTION
/// node, never a LIST-typed CONSTANT.
fn slice_bound(bound: &Value) -> Result<Option<Box<ExprShape>>, String> {
    if class(bound) == "CONSTANT" && str_field(field(field(bound, "value"), "type"), "id") == "LIST"
    {
        return Ok(None);
    }
    Ok(Some(Box::new(duckdb_expr_shape(bound)?)))
}

fn duckdb_cast_shape(expr: &Value) -> Result<ExprShape, String> {
    let child = field(expr, "child");
    let cast_type = field(expr, "cast_type");
    // Boolean literals: DuckDB lowers `TRUE`/`FALSE` to `CAST('t'/'f' AS BOOLEAN)`.
    if str_field(cast_type, "id") == "BOOLEAN" && class(child) == "CONSTANT" {
        let inner = field(child, "value");
        if str_field(field(inner, "type"), "id") == "VARCHAR" {
            match inner.get("value").and_then(Value::as_str) {
                Some("t") => return Ok(ExprShape::Literal(LiteralShape::Boolean(true))),
                Some("f") => return Ok(ExprShape::Literal(LiteralShape::Boolean(false))),
                _ => {}
            }
        }
    }
    Ok(ExprShape::Cast {
        expr: Box::new(duckdb_expr_shape(child)?),
        data_type: duckdb_data_type_shape(cast_type)?,
    })
}

/// Map a DuckDB cast target type id to the neutral [`DataTypeShape`], using the same
/// canonical (PostgreSQL-style) names [`squonk_data_type_shape`](crate::shape) produces
/// so a `CAST(x AS INTEGER)` compares equal on both sides.
fn duckdb_data_type_shape(cast_type: &Value) -> Result<DataTypeShape, String> {
    let (name, modifiers): (&str, Vec<i64>) = match str_field(cast_type, "id") {
        "BOOLEAN" => ("bool", Vec::new()),
        "TINYINT" | "SMALLINT" => ("int2", Vec::new()),
        "INTEGER" => ("int4", Vec::new()),
        "BIGINT" => ("int8", Vec::new()),
        "FLOAT" => ("float4", Vec::new()),
        "DOUBLE" => ("float8", Vec::new()),
        "VARCHAR" => ("varchar", Vec::new()),
        "BLOB" => ("bytea", Vec::new()),
        "DATE" => ("date", Vec::new()),
        "UUID" => ("uuid", Vec::new()),
        "DECIMAL" => {
            let info = field(cast_type, "type_info");
            let width = info.get("width").and_then(Value::as_u64);
            let scale = info.get("scale").and_then(Value::as_u64);
            // A bare `DECIMAL` cast target materializes DuckDB's default width/scale
            // in the serialization, making it indistinguishable from an explicit
            // `DECIMAL(18,3)` — an irrecoverable surface ambiguity, so a counted skip
            // rather than a guess (the comma/CROSS JOIN precedent; module docs).
            if width == Some(18) && scale == Some(3) {
                return Err(
                    "bare DECIMAL and explicit DECIMAL(18,3) serialize identically \
                     (DuckDB materializes the default width/scale)"
                        .into(),
                );
            }
            let mut modifiers = Vec::new();
            if let Some(width) = width {
                modifiers.push(width as i64);
            }
            if let Some(scale) = scale {
                modifiers.push(scale as i64);
            }
            ("numeric", modifiers)
        }
        // Unknown/DuckDB-specific target types would mis-map, so skip the whole statement.
        other => return Err(format!("unsupported cast target {other}")),
    };
    Ok(DataTypeShape::Named {
        name: vec![name.to_owned()],
        modifiers,
        array_depth: 0,
    })
}

fn duckdb_subquery_shape(expr: &Value) -> Result<ExprShape, String> {
    match duckdb_subquery_shape_negated(expr, false)? {
        Some(shape) => Ok(shape),
        // A quantified `SUBQUERY` we do not model (`< ANY`, `= ALL`, …) is an honest gap.
        None => Ok(ExprShape::Unmapped),
    }
}

/// The neutral shape of a `SUBQUERY` node, or `None` for a quantified form outside the
/// neutral vocabulary. `negated` folds an enclosing `OPERATOR_NOT` into the result.
fn duckdb_subquery_shape_negated(expr: &Value, negated: bool) -> Result<Option<ExprShape>, String> {
    let inner = || duckdb_query_shape(field(field(expr, "subquery"), "node"));
    Ok(match str_field(expr, "subquery_type") {
        // `NOT` over a scalar/EXISTS still yields the same neutral shape our AST keeps:
        // `Exists` drops `negated` (matching `expr_shape`), and a negated scalar subquery
        // is not itself a distinct shape.
        "SCALAR" if !negated => Some(ExprShape::Subquery(Box::new(inner()?))),
        "EXISTS" => Some(ExprShape::Exists(Box::new(inner()?))),
        // `x IN (subquery)` is DuckDB's `ANY`/`COMPARE_EQUAL` with the probe in `child`.
        "ANY"
            if str_field(expr, "comparison_type") == "COMPARE_EQUAL"
                && !field(expr, "child").is_null() =>
        {
            Some(ExprShape::InSubquery {
                expr: Box::new(duckdb_expr_shape(field(expr, "child"))?),
                subquery: Box::new(inner()?),
                negated,
            })
        }
        _ => None,
    })
}

fn duckdb_case_shape(expr: &Value) -> Result<ExprShape, String> {
    let when_clauses = arr_field(expr, "case_checks")
        .iter()
        .map(|check| {
            Ok(WhenClauseShape {
                condition: duckdb_expr_shape(field(check, "when_expr"))?,
                result: duckdb_expr_shape(field(check, "then_expr"))?,
            })
        })
        .collect::<Result<_, String>>()?;
    let else_expr = field(expr, "else_expr");
    Ok(ExprShape::Case {
        // DuckDB desugars a simple `CASE x WHEN …` into searched form (its checks carry
        // the rebuilt `x = …`), so the serialized tree never carries an operand; a simple
        // `CASE` therefore surfaces as a residual against our operand-bearing shape.
        operand: None,
        when_clauses,
        else_result: if else_expr.is_null() {
            None
        } else {
            Some(Box::new(duckdb_expr_shape(else_expr)?))
        },
    })
}

// ---- small helpers --------------------------------------------------------

/// A JSON array of strings to `Vec<String>` (`""` for non-strings).
fn string_list(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .map(string_slice)
        .unwrap_or_default()
}

fn string_slice(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.as_str().unwrap_or("").to_owned())
        .collect()
}

/// The decimal text of a JSON integer, preserving exact value across the i64/u64 range.
fn integer_text(value: &Value) -> Option<String> {
    value
        .as_i64()
        .map(|n| n.to_string())
        .or_else(|| value.as_u64().map(|n| n.to_string()))
}

/// Decode DuckDB's two-limb 128-bit integer serialization: `{upper, lower}` is the
/// value's i128 in two's complement, `upper` the signed high 64 bits and `lower` the
/// unsigned low 64. `None` when the object shape is absent (a plain JSON number
/// never reaches this).
fn hugeint_i128(value: &Value) -> Option<i128> {
    let upper = value.get("upper")?.as_i64()?;
    let lower = value.get("lower")?.as_u64()?;
    Some((i128::from(upper) << 64) | i128::from(lower))
}

/// Reconstruct decimal text from DuckDB's scaled-integer `DECIMAL` encoding
/// (`value:15, scale:1` -> `"1.5"`), matching the source text our side keeps. Takes
/// i128 because a wide `DECIMAL` (width > 18) carries a two-limb 128-bit raw value.
fn decimal_text(raw: i128, scale: usize) -> String {
    let negative = raw < 0;
    let digits = raw.unsigned_abs().to_string();
    let magnitude = if scale == 0 {
        digits
    } else {
        // Pad so there is at least one digit left of the point (`5` scale 2 -> `0.05`).
        let padded = if digits.len() <= scale {
            format!("{}{digits}", "0".repeat(scale - digits.len() + 1))
        } else {
            digits
        };
        let point = padded.len() - scale;
        format!("{}.{}", &padded[..point], &padded[point..])
    };
    if negative {
        format!("-{magnitude}")
    } else {
        magnitude
    }
}

#[cfg(test)]
mod tests;
