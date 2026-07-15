// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The Pratt (precedence-climbing) expression parser.
//!
//! One binding-power table drives every operator: [`parse_expr_bp`](crate::parser::Parser::parse_expr_bp)
//! recurses the right-hand side at the operator's *own* right binding power, so
//! the prior art's precedence mis-bind — a `parse_infix` hook that ignored the
//! precedence handed to it — is structurally impossible here: there is no
//! precedence *parameter* to ignore. Left-associativity falls out of recursing at
//! `right` (which is `left + 1`, so a same-level operator's `left` cannot clear
//! the bumped minimum and re-enter); non-associativity is rejected explicitly,
//! because the climbing loop would otherwise silently *left*-associate it.
//!
//! Parentheses are grouping only: they are consumed and discarded, never stored
//! as a node. During parsing they still form a transient
//! barrier for non-associative chain checks; render-time parenthesization is then
//! derived from the same binding powers, so the AST need not remember where the
//! source put parens.
//!
//! Richer expression families land as focused parser branches around the core:
//! `CAST` is a prefix primary because type names are shared parser surface for
//! casts, DDL, and parameters; subquery predicates bind at the comparison level
//! without becoming ordinary binary RHS expressions.

use crate::ast::{Expr, Extension, Keyword, LiteralKind, SpecialFunctionKeyword};

mod call;
mod collections;
mod core;
mod keyword_forms;
mod literals;
pub(in crate::parser) use literals::{string_literal_is_name_sconst, string_literal_is_sconst};
mod primary;
mod sqljson;
mod string_funcs;
mod xml;

#[cfg(test)]
mod tests;

/// Parser-internal expression plus transient source grouping state.
///
/// The public AST intentionally has no `Expr::Nested` node, but a
/// grouped expression must suppress exactly the next non-associative chain check:
/// `(a < b) < c` is valid PostgreSQL syntax, while `a < b < c` is not.
struct ParsedExpr<X: Extension> {
    expr: Expr<X>,
    grouped: bool,
}

impl<X: Extension> ParsedExpr<X> {
    fn bare(expr: Expr<X>) -> Self {
        Self {
            expr,
            grouped: false,
        }
    }

    fn grouped(expr: Expr<X>) -> Self {
        Self {
            expr,
            grouped: true,
        }
    }
}

/// Classify a `Number` lexeme as a money, integer, or float literal.
///
/// The tokenizer already validated the numeric shape. A `$` currency sigil marks a
/// T-SQL money literal (`$1234.56`); the check comes first because a money body holds
/// the same `.` that otherwise marks a float. The sigil may sit behind a folded sign
/// in SET-value position (`-$1000`), where the sign is part of the span (unlike
/// expression position), so an optional leading `+`/`-` is skipped. A
/// `0x`/`0o`/`0b` radix integer is recognised next — through the shared
/// [`split_radix_prefix`](crate::ast::split_radix_prefix) recognizer the AST value
/// decoders also use, so this classification and `as_i64`/`as_decimal_text` cannot
/// disagree on which spellings are radix integers — because such a literal has
/// no exponent form: its `E`/`e` bytes are hex *digits* (`0xBEEF`, `0x1E`), so they must
/// be ruled out before the decimal scan that would otherwise mistag them `Float`. With
/// neither sigil nor radix prefix, a fractional point or exponent marker distinguishes a
/// float (`3.14`, `1e9`, `.5`) from an integer (`42`).
///
/// When `parse_float_as_decimal` is set (the parser's off-by-default
/// [`ParseConfig::parse_float_as_decimal`](crate::parser::ParseConfig::parse_float_as_decimal)
/// consumer request), the float case is tagged [`LiteralKind::Decimal`] instead — the
/// only thing the flag changes. Integers, radix integers, and money are unaffected, and
/// the spelling round-trips from the span either way, so the flag is pure classification
/// metadata (see the `Decimal` variant's docs).
pub(super) fn number_literal_kind(text: &str, parse_float_as_decimal: bool) -> LiteralKind {
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if unsigned.starts_with('$') {
        LiteralKind::Money
    } else if crate::ast::split_radix_prefix(unsigned).0 != 10 {
        // A non-decimal base (16/8/2) witnesses a `0x`/`0o`/`0b` prefix; the tokenizer
        // only emits such a lexeme with a valid radix digit following, so the prefix alone
        // classifies it (`0xZ` lexes as `0` + `xZ`, never reaching here).
        LiteralKind::Integer
    } else if text.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E')) {
        if parse_float_as_decimal {
            LiteralKind::Decimal
        } else {
            LiteralKind::Float
        }
    } else {
        LiteralKind::Integer
    }
}

/// Map a special-function [`Keyword`] to its [`SpecialFunctionKeyword`] tag and
/// whether it accepts a `(precision)` argument.
///
/// Only the four temporal forms (`CURRENT_TIME`, `CURRENT_TIMESTAMP`,
/// `LOCALTIME`, `LOCALTIMESTAMP`) take a precision; the rest are strictly nullary.
/// `pub(super)` so [`super::from`]'s `FROM`-position special function table
/// reference (`SELECT * FROM current_date`, the same grammar production
/// promoted to `func_table`) shares this one mapping.
pub(super) fn special_function_keyword(keyword: Keyword) -> (SpecialFunctionKeyword, bool) {
    match keyword {
        Keyword::CurrentCatalog => (SpecialFunctionKeyword::CurrentCatalog, false),
        Keyword::CurrentDate => (SpecialFunctionKeyword::CurrentDate, false),
        Keyword::CurrentRole => (SpecialFunctionKeyword::CurrentRole, false),
        Keyword::CurrentSchema => (SpecialFunctionKeyword::CurrentSchema, false),
        Keyword::CurrentTime => (SpecialFunctionKeyword::CurrentTime, true),
        Keyword::CurrentTimestamp => (SpecialFunctionKeyword::CurrentTimestamp, true),
        Keyword::CurrentUser => (SpecialFunctionKeyword::CurrentUser, false),
        Keyword::Localtime => (SpecialFunctionKeyword::LocalTime, true),
        Keyword::Localtimestamp => (SpecialFunctionKeyword::LocalTimestamp, true),
        Keyword::SessionUser => (SpecialFunctionKeyword::SessionUser, false),
        Keyword::SystemUser => (SpecialFunctionKeyword::SystemUser, false),
        Keyword::User => (SpecialFunctionKeyword::User, false),
        // MySQL UTC forms: `UTC_TIME`/`UTC_TIMESTAMP` take a fractional-seconds precision,
        // `UTC_DATE` does not — matching the `CURRENT_*` split.
        Keyword::UtcDate => (SpecialFunctionKeyword::UtcDate, false),
        Keyword::UtcTime => (SpecialFunctionKeyword::UtcTime, true),
        Keyword::UtcTimestamp => (SpecialFunctionKeyword::UtcTimestamp, true),
        _ => unreachable!("special_function_keyword called with a non-special-function keyword"),
    }
}

/// True if `keyword` opens a SQL special value function (PostgreSQL
/// `SQLValueFunction`): `CURRENT_DATE`, `CURRENT_USER`, `USER`, or one of the
/// four temporal forms. Mirrors the keyword set in the expression dispatch
/// (`Parser::parse_keyword_prefix` in `keyword_forms.rs`; kept as a separate list
/// rather than refactored to share it, so as not to disturb that already-tested
/// match); a new special-function keyword valid in FROM position must be added to
/// both.
///
/// The MySQL `UTC_DATE`/`UTC_TIME`/`UTC_TIMESTAMP` forms are deliberately absent: they
/// are expression-position-only (MySQL has no PostgreSQL-style `func_table` promotion),
/// so admitting them here would wrongly parse `FROM utc_date` as a special function
/// across every dialect. The expression dispatch gates them behind the MySQL-only flag.
pub(super) fn is_special_function_keyword(keyword: Keyword) -> bool {
    matches!(
        keyword,
        Keyword::CurrentCatalog
            | Keyword::CurrentDate
            | Keyword::CurrentRole
            | Keyword::CurrentSchema
            | Keyword::CurrentTime
            | Keyword::CurrentTimestamp
            | Keyword::CurrentUser
            | Keyword::Localtime
            | Keyword::Localtimestamp
            | Keyword::SessionUser
            | Keyword::SystemUser
            | Keyword::User
    )
}
