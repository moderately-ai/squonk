// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The `keywordCase` knob: recase keyword tokens in the formatted output.
//!
//! The pretty renderer always emits keywords in canonical uppercase (it reuses the
//! canonical [`Render`](crate::ast::render::Render) path for expression fragments,
//! which uppercases). Casing is applied afterwards as a single token-aware pass over
//! the *output* string: the tokenizer marks exactly which byte ranges are keywords —
//! distinct from identifiers, string literals, and comments — so recasing a keyword
//! can never touch a quoted identifier, a string body, or a comment. This is the
//! spelling-fidelity spike's recommendation (item 2): a whole-document knob, not
//! per-token spelling fidelity (which the AST does not record).
//!
//! `preserve` resolves to the document's *dominant* keyword case, computed in one
//! pass over the source token stream (sqlfluff's "consistent" precedent): if most
//! source keywords are lowercase the output is lowercased, otherwise uppercased.

use crate::dialect::BuiltinDialect;
use crate::tokenize_with_builtin;
use crate::tokenizer::TokenKind;

/// How keyword tokens are cased in formatted output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KeywordCase {
    /// Uppercase every keyword (`SELECT`). The canonical default.
    #[default]
    Upper,
    /// Lowercase every keyword (`select`).
    Lower,
    /// Match the source document's dominant keyword case (all-upper or all-lower).
    Preserve,
}

impl KeywordCase {
    /// Parse a `keywordCase` name, case-insensitively. `None` for an unknown name.
    pub fn from_name(name: &str) -> Option<KeywordCase> {
        if name.eq_ignore_ascii_case("upper") {
            Some(KeywordCase::Upper)
        } else if name.eq_ignore_ascii_case("lower") {
            Some(KeywordCase::Lower)
        } else if name.eq_ignore_ascii_case("preserve") {
            Some(KeywordCase::Preserve)
        } else {
            None
        }
    }
}

/// Whether keywords should be lowercased, resolving [`Preserve`](KeywordCase::Preserve)
/// against the dominant case of `source`'s keyword tokens.
fn lowercase_keywords(case: KeywordCase, source: &str, dialect: BuiltinDialect) -> bool {
    match case {
        KeywordCase::Upper => false,
        KeywordCase::Lower => true,
        KeywordCase::Preserve => dominant_is_lower(source, dialect),
    }
}

/// True when the majority of `source`'s keyword tokens are written all-lowercase.
///
/// Ties (equal lower/upper counts, or no cased keywords at all) resolve to uppercase,
/// the canonical default — so a document with no keywords, or a deliberate 50/50
/// split, renders uppercase.
fn dominant_is_lower(source: &str, dialect: BuiltinDialect) -> bool {
    let Ok(tokens) = tokenize_with_builtin(source, dialect) else {
        return false;
    };
    let mut lower = 0usize;
    let mut upper = 0usize;
    for token in &tokens {
        if !matches!(token.kind, TokenKind::Keyword(_)) {
            continue;
        }
        let Some(text) = slice(source, token.span.start(), token.span.end()) else {
            continue;
        };
        let has_alpha = text.chars().any(|c| c.is_ascii_alphabetic());
        if !has_alpha {
            continue;
        }
        if text.chars().all(|c| !c.is_ascii_uppercase()) {
            lower += 1;
        } else if text.chars().all(|c| !c.is_ascii_lowercase()) {
            upper += 1;
        }
    }
    lower > upper
}

/// Rewrite every keyword token in `rendered` to `case`, leaving all other tokens,
/// string bodies, comments, and whitespace byte-for-byte unchanged.
///
/// `source` is the *original* input, used only to resolve
/// [`Preserve`](KeywordCase::Preserve)'s dominant case; the recasing itself scans the
/// already-rendered output.
pub fn apply(rendered: String, source: &str, dialect: BuiltinDialect, case: KeywordCase) -> String {
    if case == KeywordCase::Upper {
        // The renderer already emits uppercase keywords; nothing to do.
        return rendered;
    }
    let to_lower = lowercase_keywords(case, source, dialect);
    let Ok(tokens) = tokenize_with_builtin(&rendered, dialect) else {
        // The formatter's own output failed to re-tokenize — return it unchanged
        // rather than corrupting it. (A parse-back test guards against this.)
        return rendered;
    };

    let mut out = String::with_capacity(rendered.len());
    let mut cursor = 0usize;
    for token in &tokens {
        if !matches!(token.kind, TokenKind::Keyword(_)) {
            continue;
        }
        let start = token.span.start() as usize;
        let end = token.span.end() as usize;
        if start < cursor || end > rendered.len() {
            continue;
        }
        out.push_str(&rendered[cursor..start]);
        let keyword = &rendered[start..end];
        if to_lower {
            out.push_str(&keyword.to_ascii_lowercase());
        } else {
            out.push_str(&keyword.to_ascii_uppercase());
        }
        cursor = end;
    }
    out.push_str(&rendered[cursor..]);
    out
}

/// `source[start..end]`, or `None` on an out-of-range or non-char-boundary span.
fn slice(source: &str, start: u32, end: u32) -> Option<&str> {
    source.get(start as usize..end as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_is_identity() {
        let rendered = "SELECT a FROM t".to_owned();
        assert_eq!(
            apply(
                rendered.clone(),
                "select a from t",
                BuiltinDialect::Ansi,
                KeywordCase::Upper
            ),
            rendered
        );
    }

    #[test]
    fn lower_recases_only_keywords() {
        // `Col`/`Tbl` are identifiers (mixed case): they must survive untouched while
        // the SELECT/FROM/WHERE/IS/NOT/NULL keywords lowercase.
        assert_eq!(
            apply(
                "SELECT Col FROM Tbl WHERE Col IS NOT NULL".to_owned(),
                "",
                BuiltinDialect::Ansi,
                KeywordCase::Lower
            ),
            "select Col from Tbl where Col is not null"
        );
    }

    #[test]
    fn lower_leaves_quoted_identifiers_and_strings_untouched() {
        // "FROM" as a quoted identifier and 'SELECT' as a string must not recase.
        assert_eq!(
            apply(
                "SELECT \"FROM\" FROM t WHERE x = 'SELECT'".to_owned(),
                "",
                BuiltinDialect::Ansi,
                KeywordCase::Lower
            ),
            "select \"FROM\" from t where x = 'SELECT'"
        );
    }

    #[test]
    fn preserve_follows_dominant_lowercase_source() {
        assert_eq!(
            apply(
                "SELECT Col FROM Tbl".to_owned(),
                "select col from tbl where col = 1",
                BuiltinDialect::Ansi,
                KeywordCase::Preserve
            ),
            "select Col from Tbl"
        );
    }

    #[test]
    fn preserve_follows_dominant_uppercase_source() {
        assert_eq!(
            apply(
                "SELECT Col FROM Tbl".to_owned(),
                "SELECT col FROM tbl WHERE col = 1",
                BuiltinDialect::Ansi,
                KeywordCase::Preserve
            ),
            "SELECT Col FROM Tbl"
        );
    }
}
