// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The ANSI/standard dialect — the always-compiled baseline `parse` defaults to.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;
use crate::render::RenderDialect;

/// The ANSI/standard dialect: the principled neutral baseline for M1.
///
/// This is "generic" in the only honest sense: the SQL:2016 standard
/// baseline ([`FeatureSet::ANSI`]), **not** a vibe-union of whatever several
/// dialects happen to accept. [`parse`](super::parse) defaults to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Ansi;

impl Dialect for Ansi {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        // The borrow of an associated const is promoted to `'static`, so the unit
        // struct stores nothing and the parser's field reads const-fold.
        &FeatureSet::ANSI
    }
}

impl RenderDialect for Ansi {
    fn render_features(&self) -> FeatureSet {
        FeatureSet::ANSI
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Statement;
    use crate::dialect::parse;
    use crate::dialect::test_support::assert_full_grammar;

    #[test]
    fn ansi_parses_the_full_m1_select_grammar() {
        assert_full_grammar(Ansi);
    }

    #[test]
    fn parse_convenience_defaults_to_ansi() {
        // `parse` is exactly `parse_with(.., crate::ParseConfig::new(Ansi))`; `SELECT 1` is the smoke test.
        let parsed = parse("SELECT 1").expect("`SELECT 1` parses");
        assert_eq!(parsed.statements().len(), 1, "one statement");
        assert!(
            matches!(parsed.statements()[0], Statement::Query { .. }),
            "a query statement",
        );
    }
}
