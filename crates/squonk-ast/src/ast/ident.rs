// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Identifier AST nodes: `Ident`, its `QuoteStyle`, and the dotted `ObjectName`.

use crate::vocab::{Meta, Symbol};
use thin_vec::ThinVec;

/// An interned identifier plus the quote style used in source.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Ident {
    /// The interned identifier text.
    pub sym: Symbol,
    /// Delimiter used to quote the source token.
    pub quote: QuoteStyle,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Surface quote spelling for an identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum QuoteStyle {
    /// Unquoted — a bare identifier.
    None,
    /// Single quotes `'…'`. Not an identifier delimiter in standard SQL — this records
    /// a MySQL string literal used as a column alias (`SELECT 1 AS 'x'`), whose value is
    /// interned as the identifier and rendered back single-quoted so it round-trips.
    Single,
    /// Double-quoted `"…"` — the standard SQL delimited identifier.
    Double,
    /// PostgreSQL / SQL-standard Unicode-escaped delimited identifier `U&"…"`, optionally
    /// followed by a `UESCAPE 'c'` clause. Like [`Double`](Self::Double) the delimiter is a
    /// double quote, but the body carries `\XXXX` / `\+XXXXXX` escapes decoded against the
    /// default `\` (or the `UESCAPE` override). The interned [`sym`](Ident::sym) holds the
    /// *decoded* value — target-neutral, so a `TargetDialect` re-spell and the redacted
    /// fingerprint emit the plain `"…"` form — while a source-fidelity render replays the
    /// exact `U&"…" [UESCAPE 'c']` spelling from the node's span. The distinct variant is
    /// what tells the renderer the decoded value and the source spelling differ (a plain
    /// `Double` ident's value already *is* its spelling).
    UnicodeDouble,
    /// Backtick-quoted `` `…` `` (MySQL).
    Backtick,
    /// Bracket-quoted `[…]` (T-SQL).
    Bracket,
}

/// A qualified object name such as `catalog.schema.table`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ObjectName(pub ThinVec<Ident>);
