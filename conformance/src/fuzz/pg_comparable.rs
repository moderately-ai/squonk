// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The `*_pg_comparable` predicate family: whether a generated statement's neutral
//! shape falls in the subset the PostgreSQL *structural* differential compares. Driven
//! one-directionally from `differential_statement` at the module root (and its one
//! `fuzz_excluded_divergence_classes_still_diverge` test) via [`structurally_pg_comparable`];
//! it is a coverage gate over the structural half, never an accept/reject gate.

use squonk_ast::{
    Expr, GroupByItem, JoinConstraint, JoinOperator, LiteralKind, NoExt, Query,
    RelationInheritance, Select, SelectItem, SetExpr, Statement, TableFactor, TableWithJoins,
    ValuesItem,
};

/// Whether `statement` is in the subset whose neutral shape we expect to match
/// PostgreSQL's, i.e. it avoids the known structural-divergence classes and the
/// constructs outside the PostgreSQL structural corpus.
///
/// This is intentionally conservative: it is a coverage gate for the *structural*
/// differential, never an accept/reject gate, so over-exclusion only narrows
/// structural coverage (accept/reject still covers everything) and never hides a
/// real divergence.
pub(crate) fn structurally_pg_comparable(statement: &Statement<NoExt>) -> bool {
    match statement {
        Statement::Query { query, .. } => query_pg_comparable(query),
        // Non-query statements are not in the M1 structural corpus.
        _ => false,
    }
}

fn query_pg_comparable(query: &Query<NoExt>) -> bool {
    query.with.is_none()
        && set_expr_pg_comparable(&query.body)
        && query
            .order_by
            .iter()
            .all(|item| expr_pg_comparable(&item.expr))
        && query.limit.as_ref().is_none_or(|limit| {
            limit.limit.as_ref().is_none_or(expr_pg_comparable)
                && limit.offset.as_ref().is_none_or(expr_pg_comparable)
        })
}

fn set_expr_pg_comparable(set: &SetExpr<NoExt>) -> bool {
    match set {
        SetExpr::Select { select, .. } => select_pg_comparable(select),
        SetExpr::Values { values, .. } => values.rows.iter().all(|row| {
            row.iter().all(|item| match item {
                // A `DEFAULT` row element maps to a `Default` shape on both sides, so
                // it stays structurally comparable (prod-sql-values-default).
                ValuesItem::Expr { expr, .. } => expr_pg_comparable(expr),
                ValuesItem::Default { .. } => true,
            })
        }),
        SetExpr::Query { query, .. } => query_pg_comparable(query),
        SetExpr::SetOperation { left, right, .. } => {
            set_expr_pg_comparable(left) && set_expr_pg_comparable(right)
        }
        // DuckDB-only PIVOT/UNPIVOT query bodies PostgreSQL never parses.
        SetExpr::Pivot { .. } | SetExpr::Unpivot { .. } => false,
    }
}

fn select_pg_comparable(select: &Select<NoExt>) -> bool {
    select.windows.is_empty()
        && select.projection.iter().all(select_item_pg_comparable)
        && select.from.iter().all(table_with_joins_pg_comparable)
        && select.selection.as_ref().is_none_or(expr_pg_comparable)
        && select.group_by.iter().all(group_by_item_pg_comparable)
        && select.having.as_ref().is_none_or(expr_pg_comparable)
}

fn group_by_item_pg_comparable(item: &GroupByItem<NoExt>) -> bool {
    match item {
        GroupByItem::Expr { expr, .. } => expr_pg_comparable(expr),
        // The fuzzer only synthesizes plain-expression items (above); the grouping-set
        // constructs are excluded from the fuzz differential and covered by the
        // dedicated `PG_GROUPING_SETS_CORPUS` structural cases instead.
        GroupByItem::Rollup { .. }
        | GroupByItem::Cube { .. }
        | GroupByItem::GroupingSets { .. }
        | GroupByItem::Empty { .. } => false,
    }
}

fn select_item_pg_comparable(item: &SelectItem<NoExt>) -> bool {
    match item {
        SelectItem::Wildcard { .. } | SelectItem::QualifiedWildcard { .. } => true,
        SelectItem::Expr { expr, .. } => expr_pg_comparable(expr),
    }
}

fn table_with_joins_pg_comparable(table: &TableWithJoins<NoExt>) -> bool {
    table_factor_pg_comparable(&table.relation)
        && table.joins.iter().all(|join| {
            table_factor_pg_comparable(&join.relation)
                && join_operator_pg_comparable(&join.operator)
        })
}

fn table_factor_pg_comparable(factor: &TableFactor<NoExt>) -> bool {
    match factor {
        TableFactor::Table {
            inheritance,
            sample,
            ..
        } => matches!(inheritance, RelationInheritance::Plain) && sample.is_none(),
        TableFactor::Derived {
            lateral, subquery, ..
        } => !*lateral && query_pg_comparable(subquery),
        // `UNNEST(…)` maps to the same neutral Function shape PostgreSQL produces for
        // `FROM unnest(…)`, so it is comparable — except the BigQuery `WITH OFFSET` form,
        // which PostgreSQL has no counterpart for (and which the PG-dialect fuzzer, with
        // `unnest_with_offset` off, never synthesizes).
        TableFactor::Unnest { with_offset, .. } => !*with_offset,
        TableFactor::Function { .. }
        | TableFactor::RowsFrom { .. }
        | TableFactor::NestedJoin { .. }
        // The fuzzer's generative surface never synthesizes a special-function
        // table reference (no `FuzzTableFactor` arm for it), so this is a
        // reachable-only-in-principle default, matching its unfuzzed siblings.
        // The DuckDB-only pivot factors are likewise unfuzzed dialect-gated
        // grammar PostgreSQL never parses.
        | TableFactor::SpecialFunction { .. }
        | TableFactor::Pivot { .. }
        | TableFactor::Unpivot { .. }
        // DuckDB's DESCRIBE/SHOW/SUMMARIZE table source is likewise dialect-gated
        // grammar PostgreSQL never parses.
        | TableFactor::ShowRef { .. }
        // JSON_TABLE / XMLTABLE lower to PostgreSQL FROM items (`JsonTable`/`RangeTableFunc`)
        // the neutral shape mapper reports as OutsideSubset, so they are never comparable —
        // and the fuzzer never synthesizes them either.
        | TableFactor::JsonTable { .. }
        | TableFactor::XmlTable { .. }
        // `TABLE(<expr>)` is a Lenient-only factor PostgreSQL parse-rejects (no oracle
        // preset ships it), and the fuzzer never synthesizes it either.
        | TableFactor::TableExpr { .. }
        // MATCH_RECOGNIZE is a Snowflake/Lenient-only factor PostgreSQL parse-rejects (no
        // oracle preset ships it), and the fuzzer never synthesizes it either.
        | TableFactor::MatchRecognize { .. }
        // OPENJSON is an MSSQL/Lenient-only factor PostgreSQL parse-rejects (no oracle preset
        // ships it), and the fuzzer never synthesizes it either.
        | TableFactor::OpenJson { .. } => false,
        TableFactor::Other { ext, .. } => match *ext {},
    }
}

fn join_operator_pg_comparable(operator: &JoinOperator<NoExt>) -> bool {
    match operator {
        JoinOperator::Inner { constraint, .. }
        | JoinOperator::LeftOuter { constraint, .. }
        | JoinOperator::RightOuter { constraint, .. }
        | JoinOperator::FullOuter { constraint, .. } => match constraint {
            JoinConstraint::On { expr, .. } => expr_pg_comparable(expr),
            JoinConstraint::Using { .. }
            | JoinConstraint::Natural { .. }
            | JoinConstraint::None { .. } => true,
        },
        JoinOperator::Cross { .. } => true,
        // The DuckDB-only and MSSQL-only joins are dialect-gated grammar PostgreSQL
        // never parses; the fuzzer's generative surface never synthesizes them (no
        // `FuzzJoinOperator` arm), so this is a reachable-only-in-principle default,
        // matching the unfuzzed `TableFactor` siblings above.
        JoinOperator::AsOf { .. }
        | JoinOperator::Positional { .. }
        | JoinOperator::Semi { .. }
        | JoinOperator::Anti { .. }
        | JoinOperator::Apply { .. } => false,
    }
}

fn expr_pg_comparable(expr: &Expr<NoExt>) -> bool {
    match expr {
        Expr::Column { .. } => true,
        // Only the literal kinds the neutral structural corpus models compare here. The
        // typed temporal literals (DATE/TIME/TIMESTAMP/INTERVAL) lower to a PostgreSQL
        // `TypeCast` rather than an `A_Const`, so `pg::expr_shape` maps them to
        // `ExprShape::Unmapped`; the generator now emits them (mirroring the proptest
        // literal growth), but they ride accept/reject parity and the render round-trip
        // rather than structural comparison — the same split that mapping documents.
        Expr::Literal { literal, .. } => matches!(
            &literal.kind,
            LiteralKind::Integer
                | LiteralKind::String
                | LiteralKind::Boolean(_)
                | LiteralKind::Null
        ),
        // A unary minus over a numeric literal — which PostgreSQL folds into a signed
        // constant while we keep the `UnaryOp` — is now normalized by the structural
        // mapping (prod-pg-map-expressions, ADR-0015), so it compares like any other
        // unary operator.
        Expr::UnaryOp { expr, .. } => expr_pg_comparable(expr),
        Expr::BinaryOp { left, right, .. } => expr_pg_comparable(left) && expr_pg_comparable(right),
        Expr::Cast { expr, .. } => expr_pg_comparable(expr),
        Expr::Subquery { query, .. } | Expr::Exists { query, .. } => query_pg_comparable(query),
        Expr::InSubquery { expr, subquery, .. } => {
            expr_pg_comparable(expr) && query_pg_comparable(subquery)
        }
        Expr::QuantifiedComparison { left, subquery, .. } => {
            expr_pg_comparable(left) && query_pg_comparable(subquery)
        }
        // Function calls (plain and window) are now emitted by the generator, but —
        // like CASE / EXTRACT / IS NULL / BETWEEN / IN-list — they are outside the
        // PostgreSQL structural corpus, so they are excluded here; accept/reject parity
        // still covers them.
        _ => false,
    }
}
