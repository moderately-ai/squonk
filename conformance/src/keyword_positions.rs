// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The libpg_query differential over the {keyword} × {position} matrix.
//!
//! This is the acceptance oracle for the per-position keyword reservation model
//! (prod-keyword-position-reserved-sets): for every keyword in our inventory and
//! every grammatical identifier position, our parser's accept/reject verdict must
//! agree with libpg_query (the real PostgreSQL parser), *except* for the
//! deliberately-documented gaps in [`EXPECTED_DIVERGENCES`].
//!
//! It proves the model is correct rather than merely plausible: a per-position
//! reject set that admitted too much or too little would disagree with PostgreSQL
//! on some `(keyword, position)` cell and fail here. The remaining divergences are
//! all grammar-coverage gaps orthogonal to the reservation model: operator keywords
//! our Pratt parser consumes, a `graph_table` keyword-class data gap, and a few
//! reserved words PostgreSQL admits via parser leniency. Each is enumerated and
//! justified below, and the test pins the set exactly so a model regression cannot
//! hide among them.

use squonk::dialect::Postgres;
use squonk::parse_with;
use squonk_ast::Keyword;

/// A grammatical identifier position, as a probe template over a keyword spelling.
///
/// `x` is a non-keyword stand-in column, so the only keyword under test is the one
/// substituted for `{kw}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Position {
    /// Column reference: `SELECT <kw>` (PostgreSQL `columnref`/`ColId`).
    ColumnRef,
    /// Table name: `SELECT * FROM <kw>` (`relation_expr`/`ColId`).
    TableName,
    /// Bare column alias, written without `AS`: `SELECT x <kw>` (`BareColLabel`).
    BareLabel,
    /// `AS` column alias: `SELECT x AS <kw>` (`ColLabel`).
    AsLabel,
    /// Function name: `SELECT <kw>(1)` (`func_application`/`type_function_name`).
    FunctionName,
    /// Type name: `SELECT CAST(x AS <kw>)` (`Typename`/`type_function_name`).
    TypeName,
}

impl Position {
    /// Every probed position, in declaration order.
    pub const ALL: [Position; 6] = [
        Position::ColumnRef,
        Position::TableName,
        Position::BareLabel,
        Position::AsLabel,
        Position::FunctionName,
        Position::TypeName,
    ];

    /// The probe SQL for `keyword` in this position.
    pub fn sql(self, keyword: &str) -> String {
        match self {
            Position::ColumnRef => format!("SELECT {keyword}"),
            Position::TableName => format!("SELECT * FROM {keyword}"),
            Position::BareLabel => format!("SELECT x {keyword}"),
            Position::AsLabel => format!("SELECT x AS {keyword}"),
            Position::FunctionName => format!("SELECT {keyword}(1)"),
            Position::TypeName => format!("SELECT CAST(x AS {keyword})"),
        }
    }
}

/// One deliberately-accepted disagreement between our parser and libpg_query.
#[derive(Clone, Copy, Debug)]
pub struct KeywordPositionDivergence {
    pub keyword: &'static str,
    pub position: Position,
    /// Why we diverge — always a grammar-coverage gap orthogonal to the
    /// per-position reservation model, never a reservation-set error.
    pub reason: &'static str,
}

const fn div(
    keyword: &'static str,
    position: Position,
    reason: &'static str,
) -> KeywordPositionDivergence {
    KeywordPositionDivergence {
        keyword,
        position,
        reason,
    }
}

/// An infix or predicate operator keyword (`AND`, `OR`, `IS`, `IN`, `BETWEEN`) the
/// Pratt expression grammar consumes; a dangling trailing occurrence (`SELECT x and`)
/// fails as an incomplete operator rather than being reduced to a bare alias, which
/// PostgreSQL's LALR grammar does. Not real-world SQL.
const OPERATOR_KEYWORD: &str = "operator keyword consumed by the Pratt expression grammar; a dangling trailing \
     occurrence is an incomplete operator, not a bare alias";

/// A reserved keyword PostgreSQL's grammar happens to admit in this position via
/// leniency (`SELECT all`, `SELECT having(1)`, …); we reject reserved keywords
/// uniformly outside the `ColLabel` (`AS` alias) position.
const RESERVED_LENIENT: &str = "reserved keyword PostgreSQL admits here via grammar leniency; we reject \
     reserved keywords outside the AS-label position";

/// `graph_table` as a type name (`CAST(x AS graph_table)`). libpg_query admits it as
/// a type/function name, but our keyword data classifies it `col_name`, so it is
/// reserved as a type name and rejected. A keyword-classification data gap, not a
/// missing production: the other special types this probe pinned (`BIT`, `JSON`,
/// `NCHAR`, bare `DOUBLE`) are now modelled by the type grammar.
const SPECIAL_TYPE_KEYWORD_CLASS: &str = "keyword libpg_query admits as a type name but our keyword data classifies \
     `col_name`; a keyword-class data gap, not a missing type production";

/// The exact, justified set of `(keyword, position)` cells where our parser and
/// libpg_query disagree. Every entry is a grammar-coverage gap orthogonal to the
/// per-position reservation model. Regenerate the membership with the
/// `keyword_position_matrix_*` test failure as a guide; each addition must carry a
/// reason that is a grammar gap, never a reservation-set error.
pub const EXPECTED_DIVERGENCES: &[KeywordPositionDivergence] = &[
    // --- OPERATOR_KEYWORD (7) ---
    div("and", Position::BareLabel, OPERATOR_KEYWORD),
    div("between", Position::BareLabel, OPERATOR_KEYWORD),
    // `LIKE`/`ILIKE` open the pattern-match predicate, so a dangling trailing
    // occurrence is an incomplete operator rather than a bare alias. (`SIMILAR`
    // needs a following `TO`, so a bare `SIMILAR` stays a usable alias and does
    // not diverge.)
    div("ilike", Position::BareLabel, OPERATOR_KEYWORD),
    div("in", Position::BareLabel, OPERATOR_KEYWORD),
    div("is", Position::BareLabel, OPERATOR_KEYWORD),
    div("like", Position::BareLabel, OPERATOR_KEYWORD),
    div("or", Position::BareLabel, OPERATOR_KEYWORD),
    // --- RESERVED_LENIENT (1) ---
    // `SELECT default`: PostgreSQL admits `default` as a bare column ref via grammar
    // leniency; we reject reserved keywords outside the `AS`-label position. The clause
    // keywords (`having`/`where`/`limit`/`offset`) and the `all` quantifier are *not*
    // leniency and carry no divergence: `SELECT having(1)` / `SELECT all` parse as an
    // empty-projection SELECT plus that clause or the `ALL` quantifier, which our
    // empty-target-list grammar now models to agree with PostgreSQL.
    div("default", Position::ColumnRef, RESERVED_LENIENT),
    // --- SPECIAL_TYPE_KEYWORD_CLASS (1) ---
    div(
        "graph_table",
        Position::TypeName,
        SPECIAL_TYPE_KEYWORD_CLASS,
    ),
];

/// Whether our PostgreSQL-preset parser accepts the probe SQL for `keyword` in
/// `position`.
pub fn squonk_accepts(position: Position, keyword: &str) -> bool {
    parse_with(&position.sql(keyword), Postgres).is_ok()
}

/// Whether libpg_query accepts the probe SQL for `keyword` in `position`.
pub fn libpg_query_accepts(position: Position, keyword: &str) -> bool {
    pg_query::parse(&position.sql(keyword)).is_ok()
}

/// Every `(position, keyword)` cell where our parser and libpg_query disagree,
/// across the whole inventory — the raw differential, sorted for comparison.
pub fn actual_divergences() -> Vec<(Position, &'static str)> {
    let mut out = Vec::new();
    for keyword in Keyword::ALL {
        let spelling = keyword.as_str();
        for position in Position::ALL {
            if squonk_accepts(position, spelling) != libpg_query_accepts(position, spelling) {
                out.push((position, spelling));
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The matrix oracle: across `{keyword} × {position}`, our parser agrees with
    /// libpg_query on every cell except the documented grammar-coverage gaps. A
    /// reservation-set error would surface here as an untriaged divergence.
    #[test]
    fn keyword_position_matrix_agrees_with_libpg_query_except_documented_gaps() {
        let actual = actual_divergences();
        let mut expected: Vec<(Position, &'static str)> = EXPECTED_DIVERGENCES
            .iter()
            .map(|d| (d.position, d.keyword))
            .collect();
        expected.sort();

        let untriaged: Vec<_> = actual
            .iter()
            .filter(|cell| !expected.contains(cell))
            .collect();
        let stale: Vec<_> = expected
            .iter()
            .filter(|cell| !actual.contains(cell))
            .collect();
        assert!(
            untriaged.is_empty() && stale.is_empty(),
            "keyword-position differential drifted from EXPECTED_DIVERGENCES.\n\
             untriaged (our parser disagrees with PostgreSQL, not allowlisted) — triage each: fix the \
             reservation set, or add an EXPECTED_DIVERGENCES entry naming the tracking ticket: {untriaged:#?}\n\
             stale (allowlisted but no longer diverges) — the gap is fixed, so SWEEP each (delete its \
             EXPECTED_DIVERGENCES entry), never re-pin it to keep it allowlisted: {stale:#?}",
        );
    }

    /// The whole probed inventory is non-trivial, so the oracle is exercising real
    /// coverage rather than an empty matrix.
    #[test]
    fn matrix_covers_the_full_inventory() {
        assert!(
            Keyword::ALL.len() > 700,
            "expected the full keyword inventory"
        );
        let cells = Keyword::ALL.len() * Position::ALL.len();
        assert!(
            cells > 4_000,
            "expected thousands of probed cells, got {cells}"
        );
    }

    /// D1 (`prod-keyword-position-reserved-sets`): an `AS` alias (`ColLabel`) admits
    /// every keyword — including reserved ones — so these ACCEPT, matching PostgreSQL.
    #[test]
    fn d1_as_alias_accepts_every_keyword() {
        for keyword in ["window", "select", "from", "over", "filter"] {
            assert!(
                squonk_accepts(Position::AsLabel, keyword),
                "D1: SELECT x AS {keyword} must parse",
            );
            assert!(libpg_query_accepts(Position::AsLabel, keyword), "PG agrees");
        }
    }

    /// D2 (`prod-keyword-position-reserved-sets`): `OVER`/`FILTER` are `AS_LABEL`, so
    /// a bare alias REJECTS them, matching PostgreSQL.
    #[test]
    fn d2_bare_over_and_filter_are_rejected() {
        for keyword in ["over", "filter"] {
            assert!(
                !squonk_accepts(Position::BareLabel, keyword),
                "D2: SELECT x {keyword} must be rejected as a bare alias",
            );
            assert!(
                !libpg_query_accepts(Position::BareLabel, keyword),
                "PG agrees"
            );
        }
    }

    /// D3 (`prod-keyword-position-reserved-sets`): `SELECT` is reserved-as-`ColId` but
    /// `BARE_LABEL`, so it ACCEPTS as a bare alias, matching PostgreSQL.
    #[test]
    fn d3_bare_select_is_accepted() {
        assert!(
            squonk_accepts(Position::BareLabel, "select"),
            "D3: SELECT x select must parse",
        );
        assert!(
            libpg_query_accepts(Position::BareLabel, "select"),
            "PG agrees"
        );
    }
}
