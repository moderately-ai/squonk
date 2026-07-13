// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The PostgreSQL dialect.
//!
//! The whole module is gated by the `postgres` cargo feature (one `#[cfg]` on its
//! `mod` declaration), so the struct, both `Dialect`/`RenderDialect` impls, and the
//! PostgreSQL-needing tests are compiled only when the feature is on — no per-item
//! gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;
use crate::render::RenderDialect;

/// The PostgreSQL dialect for M1 ([`FeatureSet::POSTGRES`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(Postgres))`.
/// In the M1 surface it differs from [`Ansi`](super::Ansi) in
/// [`identifier_casing`](FeatureSet::identifier_casing) (lower- vs upper-folding
/// for unquoted-identifier *identity*) and in
/// [`string_literals`](FeatureSet::string_literals) (PostgreSQL escape strings
/// and dollar-quoted strings).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Postgres;

impl Dialect for Postgres {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::POSTGRES
    }
}

impl RenderDialect for Postgres {
    fn render_features(&self) -> FeatureSet {
        FeatureSet::POSTGRES
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::DataType;
    use crate::ast::dialect::{Casing, FeatureSet};
    use crate::dialect::Ansi;
    use crate::dialect::test_support::{REPRESENTATIVE, assert_full_grammar, first_column_type};
    use crate::parse_with;
    use crate::parser::Parsed;

    #[test]
    fn postgres_parses_the_full_m1_select_grammar() {
        assert_full_grammar(Postgres);
    }

    #[test]
    fn features_return_their_presets_and_differ_in_dialect_specific_fields() {
        assert_eq!(std::mem::size_of::<Ansi>(), 0);
        assert_eq!(std::mem::size_of::<Postgres>(), 0);

        assert_eq!(Ansi.features(), &FeatureSet::ANSI, "ANSI preset");
        assert_eq!(
            Postgres.features(),
            &FeatureSet::POSTGRES,
            "PostgreSQL preset"
        );

        // Unquoted-identifier identity folds upper for ANSI, lower for PostgreSQL.
        assert_eq!(Ansi.features().identifier_casing, Casing::Upper);
        assert_eq!(Postgres.features().identifier_casing, Casing::Lower);
        assert_ne!(
            Ansi.features().identifier_casing,
            Postgres.features().identifier_casing,
        );

        assert!(!Ansi.features().string_literals.escape_strings);
        assert!(!Ansi.features().string_literals.dollar_quoted_strings);
        assert!(Postgres.features().string_literals.escape_strings);
        assert!(Postgres.features().string_literals.dollar_quoted_strings);
    }

    #[test]
    fn ansi_and_postgres_render_identically() {
        // Shared ANSI/PostgreSQL syntax renders identically through the Tier-1
        // canonical renderer; dialect-only source spellings are handled by their
        // parse feature gates and exact literal spans.
        use squonk_ast::render::{RenderConfig, RenderCtx, RenderExt, RenderMode};

        let render = |parsed: &Parsed| {
            let config = RenderConfig {
                mode: RenderMode::Canonical,
                ..RenderConfig::default()
            };
            let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
            parsed.statements()[0].displayed(&ctx).to_string()
        };

        let ansi =
            parse_with(REPRESENTATIVE, crate::ParseConfig::new(Ansi)).expect("parses under ANSI");
        let postgres = parse_with(REPRESENTATIVE, crate::ParseConfig::new(Postgres))
            .expect("parses under PostgreSQL");

        assert_eq!(
            render(&ansi),
            render(&postgres),
            "M1 has no ANSI/PostgreSQL rendering divergence",
        );
        // And the canonical render round-trips the (canonical) source verbatim.
        assert_eq!(render(&ansi), REPRESENTATIVE);
    }

    #[test]
    fn ansi_and_postgres_reject_mysql_only_surface() {
        // No regression: the MySQL-only forms stay rejected under the other presets,
        // because each is gated as data (the gate-off path leaves a clean error).
        // (`SELECT a && b` is deliberately NOT here: `&&` is not MySQL-only — PostgreSQL
        // accepts it as the array-overlap operator under `OperatorSyntax::custom_operators`,
        // a coverage gap closed by pg-operator-surface-regex-geometric-network. ANSI still
        // rejects it, verified in `ansi_rejects_the_general_operator_surface` below.)
        for sql in [
            "SELECT a FROM t LIMIT 5, 10", // comma form: the `,` is trailing input
            "SELECT `c` FROM t",           // backtick identifiers do not lex
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects {sql:?}"
            );
        }
    }

    #[test]
    fn postgres_reads_a_national_string_spelling_as_a_typed_literal() {
        // PostgreSQL has NO `N'…'` national-string constant — its scanner reads `N'x'`
        // as the identifier `nchar` + a separate string, i.e. the typed literal
        // `nchar 'x'` (engine-probed against pg_query 6.1.1;
        // pg-national-strings-lexing-divergence). The preset therefore leaves
        // `national_strings` off, `N` lexes as an ordinary word, and `N'x'` folds into
        // the generalized typed literal `N '…'` — an `Expr::Cast`, never a national
        // `Expr::Literal`. (The one-token national reading under a preset that arms the
        // flag is pinned by the tokenizer's
        // `national_string_lexes_as_one_string_token_only_when_enabled` and the
        // conformance `SELECT N'x'` labelled case — the dialect-override side of this
        // shared-table proof.)
        use crate::ast::{CastSyntax, Expr, SelectItem, SetExpr, Statement};

        assert!(!Postgres.features().string_literals.national_strings);

        let parsed = parse_with("SELECT N'x'", crate::ParseConfig::new(Postgres))
            .expect("parses under PostgreSQL");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let [
            SelectItem::Expr {
                expr:
                    Expr::Cast {
                        syntax: CastSyntax::PrefixTyped,
                        ..
                    },
                alias: None,
                ..
            },
        ] = select.projection.as_slice()
        else {
            panic!("expected the typed-literal cast projection");
        };

        // The DO-arg residual this flip closes: `nchar` is a legal language name to
        // PostgreSQL because `N'p'` is `N` + `'p'`, never one rejected national token.
        assert!(parse_with("DO LANGUAGE N'p'", crate::ParseConfig::new(Postgres)).is_ok());
    }

    #[test]
    fn ansi_rejects_the_general_operator_surface() {
        // The general symbolic-operator surface is off for ANSI (`custom_operators` off and
        // `caret_operator: Unsupported`), so the regex/geometric ops, `&&`, and `^`-as-power
        // all stay rejected — each ends the expression at a clean parse error. PostgreSQL accepts
        // every one (verified by the pg-regress corpus sweep); ANSI is the gate-off proof.
        for sql in [
            "SELECT a && b",  // array overlap
            "SELECT a ~ b",   // regex match
            "SELECT a <-> b", // geometric distance
            "SELECT 2 ^ 3",   // exponentiation
            "SELECT @ b",     // prefix absolute value
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_ok(),
                "PostgreSQL accepts {sql:?}"
            );
        }
    }

    #[test]
    fn ansi_and_postgres_do_not_recognize_mysql_type_names() {
        // The MySQL-only scalar names are not gated on under ANSI/PostgreSQL, so they
        // fall through to the user-defined-type path instead of a built-in variant —
        // the "they don't recognize it" proof (the word stays an ordinary type name).
        for sql in [
            "CREATE TABLE t (c TINYINT)",
            "CREATE TABLE t (c MEDIUMINT)",
            "CREATE TABLE t (c MEDIUMTEXT)",
            "CREATE TABLE t (c DATETIME)",
        ] {
            assert!(
                matches!(first_column_type(Ansi, sql), DataType::UserDefined { .. }),
                "ANSI reads {sql:?} as a user-defined type",
            );
            assert!(
                matches!(
                    first_column_type(Postgres, sql),
                    DataType::UserDefined { .. }
                ),
                "PostgreSQL reads {sql:?} as a user-defined type",
            );
        }

        // The structural `ENUM('a','b')` / `SET('a','b')` forms are a hard parse error
        // off-MySQL: a string value list is not the numeric type-modifier list the
        // user-defined-type path accepts.
        for sql in [
            "CREATE TABLE t (c ENUM('a', 'b'))",
            "CREATE TABLE t (c SET('a', 'b'))",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects {sql:?}",
            );
        }
    }
}
