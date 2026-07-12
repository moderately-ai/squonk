// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Resilient, multi-error parsing over the error-sink seam.
//!
//! The default parse path is strictly fail-fast: the first error short-circuits and
//! no partial tree is returned. Tooling that wants *every* diagnostic in a
//! script — a compiler-style "all errors in the file" — calls [`parse_recovering`]
//! instead. On a statement's parse error it records the diagnostic and resynchronizes
//! at the next `;` (panic-mode recovery, the statement separator as the single sync
//! point), then resumes, so one run yields BOTH the well-formed statements (as
//! ordinary AST) AND the collected errors for the broken ones. It is an additional
//! entry point, not a mode of the default one: fail-fast [`parse_with`](super::parse_with)
//! stays the untouched lean path, since the two return fundamentally different shapes
//! (a single `Result<Parsed>` vs. a partial tree plus a list of errors).
//!
//! This is statement-level recovery only; clause/expression-level resync is
//! deliberately deferred until a consumer needs it. No error *nodes* enter the AST:
//! there is no lossless CST, so a broken statement contributes an entry to
//! [`Recovered::errors`], never a node to the tree. The partial AST and the
//! diagnostics travel out of band, side by side.
//!
//! [`Recovered`] is a differently-shaped result from [`Parsed`] — partial AST plus a diagnostic list, not a single tree — so it keeps its own verb pair rather than folding into [`ParseOptions`] (the crate's entry-point rule: a same-shape knob is a `ParseOptions` field, a new result shape is a new verb). [`parse_recovering_with_options`] still composes every `ParseOptions` field that a `Parsed`-carrying root can honour, [`ParseOptions::capture_trivia`] included: the returned [`Recovered`] embeds a real [`Parsed`], so its [`parsed()`](Recovered::parsed) exposes the same trivia queries [`parse_with_trivia`](super::parse_with_trivia) does.

use std::sync::Arc;

use crate::ast::{Extension, NoExt, SourceStore, Statement};
use crate::error::{ParseError, ParseResult};

use super::engine::Parser;
use super::{DEFAULT_RECURSION_LIMIT, Dialect, ParseOptions, Parsed};

/// The result of a recovering parse: the partial AST plus every collected error.
///
/// Composes the owned [`Parsed`] root — the sole holder of the source and
/// resolver that give the recovered statements meaning — with the out-of-band
/// diagnostics for the statements that failed to parse. A fully well-formed script
/// yields the same statements as [`parse_with`](super::parse_with) and an empty
/// [`errors`](Self::errors); a broken statement is absent from the tree and present
/// in `errors` (no error nodes).
#[derive(Debug)]
pub struct Recovered<S: SourceStore = Arc<str>, X: Extension = NoExt> {
    parsed: Parsed<S, X>,
    errors: Vec<ParseError>,
}

impl<S: SourceStore, X: Extension> Recovered<S, X> {
    /// The owned [`Parsed`] root of the well-formed statements, with the source and
    /// resolver they were parsed against. Use it exactly like a [`parse_with`]
    /// result — render, resolve symbols, query line/column.
    ///
    /// [`parse_with`]: super::parse_with
    pub fn parsed(&self) -> &Parsed<S, X> {
        &self.parsed
    }

    /// The recovered statements, in source order — shorthand for
    /// [`parsed().statements()`](Parsed::statements).
    pub fn statements(&self) -> &[Statement<X>] {
        self.parsed.statements()
    }

    /// Every collected parse error, in source order — one per statement that failed.
    ///
    /// Each carries its byte [`Span`](crate::ast::Span), so a diagnostic can resolve
    /// line/column through the [`parsed`](Self::parsed) root's
    /// [`span_line_col`](Parsed::span_line_col). Empty when the whole script parsed.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Whether any statement failed to parse.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Split into the owned [`Parsed`] root and the collected errors.
    pub fn into_parts(self) -> (Parsed<S, X>, Vec<ParseError>) {
        (self.parsed, self.errors)
    }
}

/// Parse `src` under `dialect`, recovering past errors to collect every diagnostic.
///
/// The resilient counterpart to [`parse_with`](super::parse_with): rather than
/// returning the first error, it records each broken statement's error, skips to the
/// next `;` boundary, and resumes, so the returned [`Recovered`] carries both the
/// well-formed statements and every collected error. The
/// default fail-fast [`parse_with`] is untouched and remains the lean path; reach for
/// this only when a tool wants all diagnostics at once.
///
/// # Errors
///
/// Returns a [`ParseError`] only if streaming setup fails (the tokenizer guards that
/// `src` fits in `u32` bytes), exactly as [`parse_with`](super::parse_with) does.
/// Per-statement errors do *not* surface here — they are collected into
/// [`Recovered::errors`].
///
/// [`parse_with`]: super::parse_with
pub fn parse_recovering<D: Dialect>(
    src: &str,
    dialect: D,
) -> ParseResult<Recovered<Arc<str>, D::Ext>> {
    collect_recovered::<D>(src, dialect, DEFAULT_RECURSION_LIMIT, false, false)
}

/// [`parse_recovering`] honouring `options` — the recursion-depth limit and trivia capture, for a caller recovering over untrusted SQL on a bounded stack, wanting the out-of-band trivia index on the recovered root, or both.
///
/// # Errors
///
/// As [`parse_recovering`]. A per-statement recursion-limit error is collected into
/// [`Recovered::errors`] like any other, then recovery resumes at the next `;`.
pub fn parse_recovering_with_options<D: Dialect>(
    src: &str,
    dialect: D,
    options: ParseOptions,
) -> ParseResult<Recovered<Arc<str>, D::Ext>> {
    collect_recovered::<D>(
        src,
        dialect,
        options.recursion_limit,
        options.capture_trivia,
        options.parse_float_as_decimal,
    )
}

/// Shared recovering body over an `Arc<str>` root (the shared-ownership tier)
/// — the sole recovering tier, since there is no `parse_recovering_rc` (unlike
/// [`collect_parsed`](super::collect_parsed) /
/// [`collect_parsed_with_trivia`](super::collect_parsed_with_trivia), which are also
/// instantiated for the `Rc<str>` tier). `capture_trivia` picks between the same two cursor constructors those use, so a recovering parse can carry the same out-of-band trivia index a collecting one can.
///
/// Drives the per-statement step directly so a failed statement does not fuse the
/// run: on `Err` it records the diagnostic and resynchronizes at the next `;` via
/// [`recover_to_statement_boundary`](Parser::recover_to_statement_boundary), then
/// continues, until end of input. A lexical fault while resynchronizing cannot be
/// stepped over, so recovery stops there (the input is un-tokenizable past that
/// point); its diagnostic is recorded unless it merely repeats the one the statement
/// already failed on.
fn collect_recovered<D>(
    src: &str,
    dialect: D,
    recursion_limit: usize,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> ParseResult<Recovered<Arc<str>, D::Ext>>
where
    D: Dialect,
{
    // Capture the parse's string-literal syntax before the dialect is moved into the
    // parser, so the root can materialise string values dialect-correctly (ADR-0006).
    let string_literals = dialect.features().string_literals;
    let parser = if capture_trivia {
        Parser::streaming_with_trivia(src, dialect)?
    } else {
        Parser::streaming(src, dialect)?
    };
    let mut parser = parser
        .with_recursion_limit(recursion_limit)
        .with_parse_float_as_decimal(parse_float_as_decimal);
    let mut statements = Vec::new();
    let mut errors = Vec::new();

    loop {
        match parser.parse_next_statement() {
            Ok(Some(statement)) => statements.push(statement),
            Ok(None) => break,
            Err(error) => {
                errors.push(error);
                match parser.recover_to_statement_boundary() {
                    // Resynced at a `;` — a known statement start; more may follow.
                    Ok(true) => {}
                    // Consumed to end of input without a boundary: recovery is
                    // complete. Stop here rather than re-entering `parse_next_statement`
                    // — an unterminated construct at EOF pins the cursor (its scan runs
                    // to EOF then errors, so the failing peek reports the fault while a
                    // re-advance reports EOF, making no token progress), so re-parsing
                    // would re-fail at the same position forever.
                    Ok(false) => break,
                    // The skip itself hit a lexical fault: no further progress is
                    // possible, so stop. Record it unless it is the very fault the
                    // statement already failed on (a bad token blocks both the parse
                    // and the skip at the same span).
                    Err(resync_error) => {
                        if errors.last() != Some(&resync_error) {
                            errors.push(resync_error);
                        }
                        break;
                    }
                }
            }
        }
    }

    // `take_trivia` is always safe to call: it drains an empty index when the parser
    // above was built without trivia capture, so `with_trivia` here is a no-op in
    // that case rather than a branch.
    let trivia = parser.take_trivia();
    let resolver = parser.finish();
    Ok(Recovered {
        parsed: Parsed::new(Arc::from(src), resolver, statements, string_literals)
            .with_trivia(trivia),
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, ObjectName, Resolver as _, SelectItem, SetExpr, Symbol};
    use crate::parser::{TestDialect, parse_with};

    /// The interned symbol of the sole projection column of a single `SELECT col`.
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
    fn collects_every_error_and_keeps_the_well_formed_statements() {
        // Two broken statements (`FROM x` cannot begin a statement; `)` cannot either)
        // among three well-formed ones. Recovery resynchronizes at each `;` so all
        // three good statements parse and both errors are reported.
        let src = "SELECT alpha; FROM x; SELECT beta; ); SELECT gamma";
        let recovered = parse_recovering(src, TestDialect).expect("streaming setup succeeds");

        // The three well-formed statements survive, as ordinary, usable AST.
        assert_eq!(recovered.statements().len(), 3);
        let resolver = recovered.parsed().resolver();
        let columns: Vec<&str> = recovered
            .statements()
            .iter()
            .map(|stmt| resolver.resolve(projection_column_symbol(stmt)))
            .collect();
        assert_eq!(columns, ["alpha", "beta", "gamma"]);

        // Both broken statements are reported, each with a real (non-synthetic) span.
        assert!(recovered.has_errors());
        assert_eq!(recovered.errors().len(), 2);
        for error in recovered.errors() {
            assert!(
                !error.span.is_synthetic(),
                "each error carries a source span: {error}",
            );
        }
        // Errors are in source order: the `FROM` error precedes the `)` error.
        assert!(recovered.errors()[0].span.start() < recovered.errors()[1].span.start());

        // The root still owns the original source (ADR-0001).
        assert_eq!(recovered.parsed().source(), src);
    }

    #[test]
    fn fail_fast_default_reports_only_the_first_error() {
        // The default path stays strictly fail-fast: it returns the FIRST error and
        // never reaches the second broken statement — the contrast that proves
        // recovery is opt-in and the lean default is untouched.
        let src = "SELECT alpha; FROM x; SELECT beta; ); SELECT gamma";

        let recovered = parse_recovering(src, TestDialect).expect("setup");
        assert_eq!(recovered.errors().len(), 2, "recovery sees both");

        let error = parse_with(src, TestDialect).expect_err("default path is fail-fast");
        // The single fail-fast error is exactly the first one recovery collected.
        assert_eq!(error.span, recovered.errors()[0].span);
    }

    #[test]
    fn well_formed_script_recovers_with_no_errors() {
        // Recovery does not change the happy path: every statement parses, no errors.
        let recovered =
            parse_recovering("SELECT 1; SELECT 2; SELECT 3", TestDialect).expect("setup");
        assert_eq!(recovered.statements().len(), 3);
        assert!(recovered.errors().is_empty());
        assert!(!recovered.has_errors());
    }

    #[test]
    fn recovers_to_end_of_input_when_the_last_statement_is_broken() {
        // A broken final statement with no trailing `;` resynchronizes to EOF: the
        // earlier good statement survives and the one error is reported.
        let recovered = parse_recovering("SELECT 1; SELECT FROM", TestDialect).expect("setup");
        assert_eq!(recovered.statements().len(), 1);
        assert_eq!(recovered.errors().len(), 1);
        assert!(!recovered.errors()[0].span.is_synthetic());
    }

    #[test]
    fn a_new_lexical_fault_while_resyncing_is_reported_then_recovery_stops() {
        // A statement that cannot begin (`)` is not a statement start) is a grammar
        // error the parser hits at the leading token without scanning ahead; resyncing
        // forward then reaches an unterminated string — a *different*, freshly-scanned
        // lexical fault, recorded before recovery stops (the input cannot be tokenized
        // past it). Contrast `SELECT FROM '…`, where the parser itself reaches the
        // string, so the fault is the *parse* error, not a separate resync one.
        let recovered =
            parse_recovering("SELECT alpha; ) 'unterminated", TestDialect).expect("setup");

        assert_eq!(recovered.statements().len(), 1);
        assert_eq!(recovered.errors().len(), 2);
        // The grammar error and the lexical resync fault are at distinct positions.
        assert_ne!(recovered.errors()[0].span, recovered.errors()[1].span);
    }

    #[test]
    fn a_repeated_lexical_fault_while_resyncing_is_not_double_reported() {
        // Here the statement fails *on* the unterminated string, and the resync hits
        // the same fault at the same span — it must not be reported twice.
        let recovered =
            parse_recovering("SELECT alpha; SELECT 'unterminated", TestDialect).expect("setup");

        assert_eq!(recovered.statements().len(), 1);
        assert_eq!(recovered.errors().len(), 1);
    }

    #[test]
    fn into_parts_yields_the_root_and_errors() {
        let src = "SELECT kept; )";
        let (parsed, errors) = parse_recovering(src, TestDialect)
            .expect("setup")
            .into_parts();

        assert_eq!(parsed.statements().len(), 1);
        assert_eq!(errors.len(), 1);
        // The detached root still renders the well-formed statement canonically.
        assert_eq!(parsed.to_string(), "SELECT kept");
    }

    #[test]
    fn with_options_defaults_to_no_trivia() {
        // Recovery's zero-cost-off contract mirrors `parse_with`'s (ADR-0005):
        // neither the argument-free `parse_recovering` nor
        // `parse_recovering_with_options` with default options captures trivia.
        let src = "SELECT alpha; /* oops */ FROM x -- trailing";
        assert!(
            parse_recovering(src, TestDialect)
                .expect("setup")
                .parsed()
                .trivia()
                .is_empty()
        );
        let recovered = parse_recovering_with_options(src, TestDialect, ParseOptions::default())
            .expect("setup");
        assert!(recovered.parsed().trivia().is_empty());
    }

    #[test]
    fn with_options_honours_trivia_capture_across_a_recovered_statement() {
        // Trivia capture composes with recovery: comment runs both before and after
        // a broken statement still land on the recovered root, proving the
        // panic-mode resync does not disturb the cursor's trivia sink.
        use crate::tokenizer::TriviaKind::{BlockComment, LineComment};

        let src = "SELECT alpha; /* oops */ FROM x; -- trailing\nSELECT beta";
        let options = ParseOptions::default().with_trivia_capture(true);
        let recovered = parse_recovering_with_options(src, TestDialect, options).expect("setup");

        assert_eq!(
            recovered.statements().len(),
            2,
            "both well-formed statements survive"
        );
        assert_eq!(
            recovered.errors().len(),
            1,
            "the FROM statement is recorded as broken"
        );

        let kinds: Vec<_> = recovered
            .parsed()
            .trivia()
            .iter()
            .map(|range| range.kind())
            .collect();
        assert!(
            kinds.contains(&BlockComment),
            "the comment leading the broken statement: {kinds:?}",
        );
        assert!(
            kinds.contains(&LineComment),
            "the comment past the resync, leading the next good statement: {kinds:?}",
        );
    }
}
