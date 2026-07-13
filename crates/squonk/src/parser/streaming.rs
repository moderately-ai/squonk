// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Public streaming statement iteration.
//!
//! [`statements`] parses a SQL script one statement at a time over the lazy token
//! buffer, holding only the in-flight statement's tokens, so a large multi-statement
//! script parses in bounded memory. [`parse_with`](super::parse_with) is the
//! collecting convenience built on the same iterator.

use crate::ast::{Resolver, Statement};
use crate::error::ParseResult;
use crate::interner::FrozenResolver;

use super::Dialect;
use super::engine::Parser;

/// A lazy iterator over the top-level statements of a SQL script.
///
/// Each [`next`](Iterator::next) parses one statement on demand and yields a
/// `ParseResult<Statement>`. Under the M1 fail-fast error policy the
/// first error is yielded once and the iterator is then exhausted, so a script does
/// not silently continue past a syntax error.
///
/// Symbols in a yielded statement resolve through [`resolver`](Self::resolver), a
/// view over the still-live interner; [`finish`](Self::finish) freezes the final
/// resolver after iteration.
pub struct Statements<'a, D: Dialect> {
    parser: Parser<'a, D>,
    /// Set once end of input or a fail-fast error is reached, fusing the iterator.
    done: bool,
}

impl<'a, D: Dialect> Statements<'a, D> {
    /// A resolver for the symbols of the statements yielded so far.
    ///
    /// Backed by the live interner, which only grows and never reassigns a symbol,
    /// so symbols from already-yielded statements stay valid. Because
    /// [`next`](Iterator::next) borrows the iterator mutably, resolve a statement's
    /// symbols *between* `next` calls (e.g. in a `while let Some(stmt) = it.next()`
    /// loop) rather than holding the resolver across a `next`.
    pub fn resolver(&self) -> &impl Resolver {
        self.parser.live_resolver()
    }

    /// Freeze the interner into the final, `Send + Sync` [`FrozenResolver`].
    ///
    /// Call once iteration is complete to resolve symbols of any statements the
    /// consumer kept. Resolving before exhausting the iterator would freeze an
    /// interner that later statements still need, so this consumes the iterator.
    pub fn finish(self) -> FrozenResolver {
        self.parser.finish()
    }
}

impl<'a, D: Dialect> Iterator for Statements<'a, D> {
    type Item = ParseResult<Statement<D::Ext>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.parser.parse_next_statement() {
            Ok(Some(statement)) => Some(Ok(statement)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(error) => {
                // Fail-fast (ADR-0005): surface the error once, then stop.
                self.done = true;
                Some(Err(error))
            }
        }
    }
}

/// Parse `src` under `dialect` as a lazy [`Statements`] iterator.
///
/// Statements are parsed one at a time in bounded memory; only the
/// in-flight statement's tokens are buffered. Use
/// [`parse_with`](super::parse_with) instead to collect every statement into an
/// owned [`Parsed`](super::Parsed) tree.
///
/// # Errors
///
/// Returns a [`ParseError`](crate::error::ParseError) only if streaming setup fails
/// (the tokenizer guards that `src` fits in `u32` bytes). Per-statement lexical and
/// grammar errors surface as `Err` items from the iterator, not here.
pub(crate) fn statements<D: Dialect>(src: &str, dialect: D) -> ParseResult<Statements<'_, D>> {
    statements_with_limit(src, dialect, super::engine::DEFAULT_RECURSION_LIMIT, false)
}

/// [`statements`] with an explicit recursion-depth limit and float-as-decimal
/// classification request.
///
/// Backs the public options-bearing entries
/// ([`statements_with`](super::statements_with) and the collecting
/// [`parse_with`](super::parse_with)); the plain [`statements`] is
/// this with [`DEFAULT_RECURSION_LIMIT`](super::DEFAULT_RECURSION_LIMIT).
pub(crate) fn statements_with_limit<D: Dialect>(
    src: &str,
    dialect: D,
    recursion_limit: usize,
    parse_float_as_decimal: bool,
) -> ParseResult<Statements<'_, D>> {
    let parser = Parser::streaming(src, dialect)?
        .recursion_limit(recursion_limit)
        .parse_float_as_decimal(parse_float_as_decimal);
    Ok(Statements {
        parser,
        done: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, ObjectName, SelectItem, SetExpr, Symbol};
    use crate::parser::{TestDialect, parse_with};

    /// The interned symbol of the sole projection column of a single SELECT.
    fn projection_column_symbol(statement: &Statement) -> Symbol {
        let Statement::Query { query, .. } = statement else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        match &select.projection[0] {
            SelectItem::Expr {
                expr:
                    Expr::Column {
                        name: ObjectName(parts),
                        ..
                    },
                ..
            } => parts[0].sym,
            other => panic!("expected a single column projection, got {other:?}"),
        }
    }

    #[test]
    fn iterates_each_statement_in_order() {
        let mut iter = statements("SELECT 1; SELECT 2; SELECT 3", TestDialect).expect("valid");
        let collected: Vec<_> = std::iter::from_fn(|| iter.next())
            .map(|result| result.expect("each statement parses"))
            .collect();
        assert_eq!(collected.len(), 3);
    }

    #[test]
    fn empty_input_yields_no_statements() {
        let mut iter = statements("  ; ;  ", TestDialect).expect("only separators");
        assert!(iter.next().is_none());
        // Fused: still none on a second poll.
        assert!(iter.next().is_none());
    }

    #[test]
    fn fail_fast_yields_the_error_once_then_stops() {
        // The middle statement is malformed; fail-fast must not yield the third.
        let mut iter = statements("SELECT 1; SELECT FROM; SELECT 3", TestDialect).expect("setup");
        assert!(iter.next().expect("first item").is_ok());
        assert!(iter.next().expect("second item").is_err());
        // Exhausted after the error — the trailing valid statement is never reached.
        assert!(iter.next().is_none());
    }

    #[test]
    fn live_resolver_resolves_each_statement_before_the_next() {
        // Resolve each statement's symbol while the interner is still live, proving
        // bounded-memory consumers can process one statement at a time.
        let mut iter = statements("SELECT alpha; SELECT beta", TestDialect).expect("valid");

        let first = iter.next().expect("first").expect("parses");
        assert_eq!(
            iter.resolver().resolve(projection_column_symbol(&first)),
            "alpha",
        );

        let second = iter.next().expect("second").expect("parses");
        // The earlier symbol still resolves after more interning.
        assert_eq!(
            iter.resolver().resolve(projection_column_symbol(&first)),
            "alpha",
        );
        assert_eq!(
            iter.resolver().resolve(projection_column_symbol(&second)),
            "beta",
        );

        assert!(iter.next().is_none());
    }

    #[test]
    fn early_drop_after_partial_iteration_is_clean() {
        // A consumer may stop early (and drop) without parsing the rest — the lazy
        // buffer means the unparsed tail is never tokenized.
        let mut iter = statements("SELECT 1; SELECT 2; SELECT 3", TestDialect).expect("valid");
        let _first = iter.next().expect("first").expect("parses");
        drop(iter);
    }

    #[test]
    fn parse_with_matches_the_streaming_iterator() {
        // parse_with is the collecting convenience over the same iterator, so the two
        // produce identical statement trees (source compatibility).
        let src = "SELECT a, b FROM t; SELECT 1 + 2";
        let collected = parse_with(src, crate::ParseConfig::new(TestDialect)).expect("collects");

        let mut iter = statements(src, TestDialect).expect("streams");
        let streamed: Vec<_> = std::iter::from_fn(|| iter.next())
            .map(|result| result.expect("parses"))
            .collect();

        assert_eq!(collected.statements(), streamed.as_slice());
    }

    #[test]
    fn adjacent_statements_require_a_separator() {
        use crate::dialect::Postgres;

        // The top-level statement list is `;`-delimited: a statement whose grammar can
        // cleanly stop mid-stream (DO, VALUES, TABLE) must still be followed by a `;` or
        // end of input, so a separator-less run of two statements is a syntax error —
        // matching libpg_query, which rejects every one of these. `SELECT 1 SELECT 2`
        // rejects only incidentally (the reserved `SELECT` cannot be a projection alias);
        // this pins the rule uniformly across the kinds that *can* cleanly stop mid-stream.
        for sql in [
            "SELECT 1 SELECT 2",
            "VALUES (1) VALUES (2)",
            "TABLE t TABLE t",
            "DO '' DO ''",
            "Do''Do''",
            "DO 'x' DO 'y'",
            "DO $$a$$ DO $$b$$",
            "DO '' SELECT 1",
            "VALUES (1) SELECT 2",
            "TABLE t SELECT 1",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "separator-less statement pair must reject: {sql:?}",
            );
        }

        // A `;` separator (or end of input on a lone statement) makes the same pairs a
        // well-formed statement list again — the fix rejects only the *missing* separator.
        for sql in [
            "SELECT 1; SELECT 2",
            "VALUES (1); VALUES (2)",
            "TABLE t; TABLE t",
            "DO ''; DO ''",
            "SELECT 1",
            "SELECT 1;",
            "DO ''",
            "VALUES (1)",
            "TABLE t",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_ok(),
                "`;`-delimited (or lone) statements must accept: {sql:?}",
            );
        }
    }

    #[test]
    fn finish_yields_a_resolver_for_retained_statements() {
        let mut iter = statements("SELECT kept", TestDialect).expect("valid");
        let statement = iter.next().expect("one").expect("parses");
        let sym = projection_column_symbol(&statement);
        let resolver = iter.finish();
        assert_eq!(resolver.resolve(sym), "kept");
    }
}
