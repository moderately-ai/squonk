// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Row pattern recognition — the `<source> MATCH_RECOGNIZE (…)` table factor
//! (SQL:2016 `<row pattern recognition clause>`; Snowflake / Oracle).
//!
//! `MATCH_RECOGNIZE` runs a regular expression over the *rows* of a partition, in a
//! defined order, and projects a row (or one row per match) from the matched runs.
//! It is modelled as a table-factor suffix ([`TableFactor::MatchRecognize`]) — the
//! `PIVOT`/`UNPIVOT` precedent — carrying one [`MatchRecognize`] operator core:
//!
//! ```text
//! <source> MATCH_RECOGNIZE (
//!     [ PARTITION BY <expr>, … ]
//!     [ ORDER BY <sort>, … ]
//!     [ MEASURES <expr> AS <name>, … ]
//!     [ ONE ROW PER MATCH | ALL ROWS PER MATCH [ SHOW EMPTY MATCHES
//!                                              | OMIT EMPTY MATCHES
//!                                              | WITH UNMATCHED ROWS ] ]
//!     [ AFTER MATCH SKIP { PAST LAST ROW | TO NEXT ROW
//!                        | TO FIRST <sym> | TO LAST <sym> } ]
//!     PATTERN ( <row pattern> )
//!     [ SUBSET <name> = ( <sym>, … ), … ]
//!     [ DEFINE <sym> AS <condition>, … ]
//! )
//! ```
//!
//! # Parity with, and deviations from, sqlparser-rs
//!
//! The shape follows sqlparser-rs's `MatchRecognize` family (`Measure`,
//! `RowsPerMatch`, `EmptyMatchesMode`, `AfterMatchSkip`, `SymbolDefinition`,
//! `MatchRecognizePattern`, `RepetitionQuantifier`), reshaped per ADR-0011 where
//! their model is lossy:
//!
//! - **Anchors are inlined into [`MatchRecognizePattern`].** sqlparser-rs nests them
//!   in a separate `MatchRecognizeSymbol { Named, Start, End }` reached through
//!   `Pattern::Symbol`/`Exclude`/`Permute`; we hoist [`Start`](MatchRecognizePattern::Start)
//!   and [`End`](MatchRecognizePattern::End) to sibling pattern variants so the tree
//!   is one recursive enum, not two.
//! - **[`Exclude`](MatchRecognizePattern::Exclude) and
//!   [`Permute`](MatchRecognizePattern::Permute) carry full sub-patterns**, not the
//!   bare symbols sqlparser-rs restricts them to — the SQL:2016 grammar allows a
//!   pattern inside `{- … -}` and each `PERMUTE(…)` argument, so the symbol-only
//!   shape is lossy.
//! - **[`Concat`](MatchRecognizePattern::Concat) is added** as an explicit
//!   sequence node (sqlparser-rs has it too), and a single-element sequence/branch is
//!   flattened to its inner pattern so `PATTERN (A)` is a bare
//!   [`Symbol`](MatchRecognizePattern::Symbol), never a one-element `Concat`.
//! - **`SUBSET` is modelled** ([`SubsetDefinition`]) — the Oracle union-variable
//!   clause sqlparser-rs omits entirely.
//! - We keep sqlparser-rs's [`RepetitionQuantifier`] arms verbatim (no *reluctant*
//!   `?` suffix): the reluctant marker needs a `?` token that the eager
//!   context-free tokenizer (ADR-0005) cannot produce in pattern position (it is the
//!   anonymous placeholder / a stray byte), so there is nothing to round-trip.
//!
//! Both operators are gated on
//! [`TableFactorSyntax::match_recognize`](crate::dialect::TableFactorSyntax) (Snowflake
//! and Lenient). See the parser's `match_recognize` module for the lexer-reachability
//! notes on the `$` anchor and `?` quantifier.

use super::{Expr, Extension, Ident, NoExt, OrderByExpr, TableFactor};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// The `MATCH_RECOGNIZE (…)` operator core — the row-pattern-recognition clause
/// applied to a [`source`](Self::source) relation, hosted by
/// [`TableFactor::MatchRecognize`](crate::ast::TableFactor) (which owns the trailing
/// `AS <alias>`).
///
/// `source` is boxed to break the `TableFactor` → `MatchRecognize` → `TableFactor`
/// type cycle (the [`Pivot`](crate::ast::Pivot) precedent), and the whole core is
/// boxed again inside the table-factor variant to keep that hot enum lean (ADR-0007).
/// Every clause but [`pattern`](Self::pattern) is optional; the field order matches
/// the fixed clause order the grammar requires.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct MatchRecognize<X: Extension = NoExt> {
    /// Input source for this syntax.
    pub source: Box<TableFactor<X>>,
    /// `PARTITION BY <expr>, …` — the partitions matched independently; empty when
    /// absent (the whole input is one partition).
    pub partition_by: ThinVec<Expr<X>>,
    /// `ORDER BY <sort>, …` — the row order the pattern matches against; empty when
    /// absent.
    pub order_by: ThinVec<OrderByExpr<X>>,
    /// `MEASURES <expr> AS <name>, …` — the columns computed from a match; empty when
    /// absent.
    pub measures: ThinVec<Measure<X>>,
    /// `ONE ROW PER MATCH` / `ALL ROWS PER MATCH [ … ]`; `None` leaves the engine
    /// default (`ONE ROW PER MATCH`) unwritten.
    pub rows_per_match: Option<RowsPerMatch>,
    /// `AFTER MATCH SKIP …` — where matching resumes after a match; `None` leaves the
    /// default (`AFTER MATCH SKIP PAST LAST ROW`) unwritten.
    pub after_match_skip: Option<AfterMatchSkip>,
    /// `PATTERN ( <row pattern> )` — the row-pattern regular expression. Mandatory.
    pub pattern: MatchRecognizePattern,
    /// `SUBSET <name> = ( <sym>, … ), …` — union pattern variables (Oracle); empty
    /// when absent.
    pub subsets: ThinVec<SubsetDefinition>,
    /// `DEFINE <sym> AS <condition>, …` — the boolean conditions defining each
    /// pattern variable; empty when absent (an undefined variable matches every row).
    pub define: ThinVec<SymbolDefinition<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `MEASURES` item: `<expr> AS <name>` — an expression computed over a match,
/// projected under an output name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Measure<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// Alias assigned by this syntax.
    pub alias: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `ONE ROW PER MATCH` / `ALL ROWS PER MATCH` output cardinality.
///
/// A tag enum (no spanned children) — [`AllRows`](Self::AllRows) carries the optional
/// empty-match mode that only the `ALL ROWS` form admits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RowsPerMatch {
    /// `ONE ROW PER MATCH` — one summary row per match.
    OneRow,
    /// `ALL ROWS PER MATCH [ SHOW EMPTY MATCHES | OMIT EMPTY MATCHES
    /// | WITH UNMATCHED ROWS ]` — one row per matched (and optionally unmatched) input
    /// row.
    AllRows(Option<EmptyMatchesMode>),
}

/// The `ALL ROWS PER MATCH` empty/unmatched-row treatment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum EmptyMatchesMode {
    /// `SHOW EMPTY MATCHES`.
    Show,
    /// `OMIT EMPTY MATCHES`.
    Omit,
    /// `WITH UNMATCHED ROWS`.
    WithUnmatched,
}

/// The `AFTER MATCH SKIP …` clause — where the matcher resumes after a match.
///
/// A spanned node (every variant carries `meta`) because the
/// [`ToFirst`](Self::ToFirst)/[`ToLast`](Self::ToLast) forms name a pattern variable.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AfterMatchSkip {
    /// `AFTER MATCH SKIP PAST LAST ROW` (the engine default).
    PastLastRow {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AFTER MATCH SKIP TO NEXT ROW`.
    ToNextRow {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AFTER MATCH SKIP TO FIRST <sym>`.
    ToFirst {
        /// Pattern symbol referenced by this syntax.
        symbol: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AFTER MATCH SKIP TO LAST <sym>`.
    ToLast {
        /// Pattern symbol referenced by this syntax.
        symbol: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `SUBSET` union-variable definition: `<name> = ( <member>, … )` — a pattern
/// variable standing for the union of the listed variables' rows (Oracle).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct SubsetDefinition {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// members in source order.
    pub members: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `DEFINE` clause: `<sym> AS <condition>` — the boolean condition a row must
/// satisfy to be classified as the pattern variable `symbol`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct SymbolDefinition<X: Extension = NoExt> {
    /// Interned source spelling.
    pub symbol: Ident,
    /// The boolean condition a row must satisfy to be classified as this variable.
    pub definition: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The row-pattern regular expression inside `PATTERN ( … )` — a recursive grammar of
/// pattern variables, anchors, grouping, alternation, concatenation, exclusion,
/// permutation, and quantifiers.
///
/// Non-generic in the extension parameter: a pattern references only variable names
/// ([`Ident`]) and structural operators, never an [`Expr`]. Every variant carries
/// `meta`, so the whole tree is addressable (ADR-0002); the recursion is bounded by
/// the parser's shared depth guard.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum MatchRecognizePattern {
    /// A pattern variable reference (`A`, `STRT`).
    Symbol {
        /// Pattern symbol referenced by this syntax.
        symbol: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The partition-start anchor `^`.
    Start {
        /// Source location and node identity.
        meta: Meta,
    },
    /// The partition-end anchor `$`.
    End {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A concatenation (sequence) `a b c` — matched in order.
    Concat {
        /// patterns in source order.
        patterns: ThinVec<MatchRecognizePattern>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An alternation `a | b | c` — matched as the first branch that succeeds.
    Alternation {
        /// patterns in source order.
        patterns: ThinVec<MatchRecognizePattern>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A parenthesized group `( … )`.
    Group {
        /// The grouped sub-pattern.
        pattern: Box<MatchRecognizePattern>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An exclusion `{- … -}` — matched but omitted from `ALL ROWS PER MATCH` output.
    Exclude {
        /// The excluded sub-pattern.
        pattern: Box<MatchRecognizePattern>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PERMUTE ( p1, p2, … )` — matches the arguments in any order.
    Permute {
        /// patterns in source order.
        patterns: ThinVec<MatchRecognizePattern>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A quantified pattern `p*`, `p+`, `p{2,3}` — repeat `pattern` per `quantifier`.
    Repetition {
        /// The repeated sub-pattern.
        pattern: Box<MatchRecognizePattern>,
        /// The repetition count or range (`*`/`+`/`?`/`{m,n}`); see [`RepetitionQuantifier`].
        quantifier: RepetitionQuantifier,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A row-pattern quantifier — the repetition count applied to a pattern factor.
///
/// A tag enum (its bounds are plain `u32`s, no spanned children). Mirrors
/// sqlparser-rs verbatim. The `AtMostOne` (`?`) form has no lexer-reachable parser
/// path — see the module docs — but is retained for parity and lossless rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RepetitionQuantifier {
    /// `*` — zero or more.
    ZeroOrMore,
    /// `+` — one or more.
    OneOrMore,
    /// `?` — zero or one.
    AtMostOne,
    /// `{n}` — exactly `n`.
    Exactly(u32),
    /// `{n,}` — at least `n`.
    AtLeast(u32),
    /// `{,m}` — at most `m`.
    AtMost(u32),
    /// `{n,m}` — between `n` and `m` (inclusive).
    Range(u32, u32),
}
