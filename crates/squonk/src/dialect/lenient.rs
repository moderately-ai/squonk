// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The optional permissive [`Lenient`] tooling dialect.
//!
//! The whole module is gated by the `lenient` cargo feature (one `#[cfg]` on its `mod`
//! declaration), so the struct, both `Dialect`/`RenderDialect` impls, and the tests
//! compile only when the feature is on.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;
use crate::render::RenderDialect;

/// The permissive "parse anything" tooling dialect ([`FeatureSet::LENIENT`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(Lenient))`, or by
/// runtime name through [`BuiltinDialect`](super::BuiltinDialect). It is the honest,
/// fully-documented permissive union (every enabled feature and every conflict-resolution
/// rule is spelled out on [`FeatureSet::LENIENT`]) — **not** a "generic" vibe-union;
/// the principled neutral baseline is [`Ansi`](super::Ansi).
///
/// Distinctively, it accepts all three identifier quote styles at once — `"x"`, `` `x` ``,
/// and `[x]` — matching the breadth of a tool that must read SQL of unknown origin.
///
/// Like every dialect it is a zero-sized unit struct whose [`Dialect`] impl hands back a
/// `&'static` borrow of the const preset, so it carries no per-parse cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lenient;

impl Dialect for Lenient {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::LENIENT
    }
}

impl RenderDialect for Lenient {
    fn render_features(&self) -> FeatureSet {
        FeatureSet::LENIENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, SelectItem, SetExpr, Statement};
    use crate::dialect::Ansi;
    use crate::dialect::test_support::assert_full_grammar;
    use crate::parse_with;

    #[test]
    fn lenient_parses_the_full_m1_select_grammar() {
        assert_full_grammar(Lenient);
    }

    #[test]
    fn lenient_is_zero_sized_and_returns_its_preset() {
        // Zero-sized: the `features()` borrow is a `'static` const, so there is no
        // per-parse cost — the same const-fold path as every other dialect.
        assert_eq!(std::mem::size_of::<Lenient>(), 0);
        assert_eq!(Lenient.features(), &FeatureSet::LENIENT);
    }

    /// Parse `SELECT <list> FROM t` under Lenient and, when every projection item is a
    /// bare column reference ([`Expr::Column`], i.e. a quoted/unquoted *identifier* and
    /// not a string [`Expr::Literal`]), return how many there are. `None` on a parse
    /// failure or a non-column projection — so it doubles as the conflict-rule-1 lens
    /// (a `"x"` lexed as a string would not be `Expr::Column`).
    fn lenient_identifier_projection_count(sql: &str) -> Option<usize> {
        let parsed = parse_with(sql, crate::ParseConfig::new(Lenient)).ok()?;
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            return None;
        };
        let SetExpr::Select { select, .. } = &query.body else {
            return None;
        };
        select
            .projection
            .iter()
            .all(|item| {
                matches!(
                    item,
                    SelectItem::Expr {
                        expr: Expr::Column { .. },
                        ..
                    }
                )
            })
            .then_some(select.projection.len())
    }

    #[test]
    fn lenient_accepts_all_three_identifier_quote_styles() {
        // The headline capability: `"x"`, backtick `` `x` ``, and bracket `[x]` are all
        // quoted identifiers under one dialect — what a single-style dialect cannot do.
        assert_eq!(
            lenient_identifier_projection_count(r#"SELECT "a", `b`, [c] FROM t"#),
            Some(3),
            "all three quote styles parse as identifier projections under Lenient",
        );
    }

    #[test]
    fn lenient_resolves_the_documented_lexical_conflicts() {
        // Rule 1: `"x"` is a quoted *identifier* (Expr::Column), not a string literal —
        // the helper returns `Some(1)` only because the projection is a column.
        assert_eq!(
            lenient_identifier_projection_count(r#"SELECT "x" FROM t"#),
            Some(1),
            "`\"x\"` is a quoted identifier, not a string",
        );

        // Rule 3: `$1` is a positional *parameter*, not a money literal. It parses (the
        // parameter form is enabled) where a stray `$` would be a lex error.
        assert!(
            parse_with("SELECT $1", crate::ParseConfig::new(Lenient)).is_ok(),
            "`$1` lexes as a positional parameter under Lenient",
        );

        // Rule 2: `[1]` is a bracket *identifier*, not array-subscript punctuation, so
        // `SELECT a[1]` is not a subscript expression — `[1]` lexes as a quoted identifier
        // and is consumed as `a`'s AS-less column alias (`a AS "1"`).
        let parsed = parse_with("SELECT a[1] FROM t", crate::ParseConfig::new(Lenient))
            .expect("parses under Lenient");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a select body");
        };
        assert!(
            matches!(
                select.projection.as_slice(),
                [SelectItem::Expr {
                    expr: Expr::Column { .. },
                    alias: Some(_),
                    ..
                }]
            ),
            "`a[1]` is column `a` aliased by the bracket identifier `[1]`, not a subscript: {:?}",
            select.projection,
        );
    }

    #[test]
    fn lenient_accepts_a_spread_of_additive_permissive_forms() {
        // A representative spread of the pure-addition permissive features, each a hard
        // error under the strict ANSI baseline — the proof LENIENT widens acceptance with
        // no regression to the strict dialect.
        for sql in [
            "SELECT a && b FROM t",         // `&&` as logical AND
            "SELECT a FROM t LIMIT 5, 10",  // MySQL `LIMIT <offset>, <count>` comma form
            "SELECT `c` FROM t",            // backtick identifiers
            "SELECT a # c\nFROM t",         // `#` line comment
            "SELECT $tag$body$tag$ FROM t", // dollar-quoted string literal
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
                "Lenient accepts {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "strict ANSI still rejects {sql:?} (no regression)",
            );
        }
    }

    #[test]
    fn strict_ansi_still_rejects_non_ansi_quote_styles() {
        // Regression guard: the permissive union is opt-in. ANSI keeps rejecting the
        // backtick and bracket identifier styles it always did.
        for sql in [r#"SELECT `x` FROM t"#, "SELECT [x] FROM t"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
        }
    }
}
