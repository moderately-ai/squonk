// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Window-function clauses: the `OVER` specification, frame, and the SELECT-level
//! `WINDOW` clause.
//!
//! These nodes hang off [`FunctionCall::over`](super::FunctionCall) (an inline or
//! named `OVER` clause) and [`Select::windows`](super::Select) (the named-window
//! definitions an `OVER name` reference resolves against). Frame *units* and
//! *exclusions* are closed tag enums (one canonical shape per construct);
//! the frame *bounds* carry the optional offset expression they bind.

use super::{Expr, Extension, Ident, NoExt, OrderByExpr};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// The `OVER` clause attached to a window function call.
///
/// `OVER name` references a definition from the query's `WINDOW` clause;
/// `OVER ( … )` carries the definition inline. Boxed in the inline case so the
/// reference case (just an [`Ident`]) stays small.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum WindowSpec<X: Extension = NoExt> {
    /// `OVER window_name` — a reference to a named [`WINDOW`-clause](NamedWindow)
    /// definition.
    Named {
        /// Name referenced by this syntax.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An inline `OVER (…)` window specification.
    Inline {
        /// The inline `OVER (…)` window definition; see [`WindowDefinition`].
        definition: Box<WindowDefinition<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A window definition: the body of `OVER ( … )` or of a `WINDOW name AS ( … )`
/// entry.
///
/// Every part is optional. `existing` is the leading base-window name PostgreSQL
/// allows inside the parens (`OVER (w PARTITION BY …)`), which the definition then
/// extends. Empty `partition_by`/`order_by` and a `None` frame mean the clause was
/// not written.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct WindowDefinition<X: Extension = NoExt> {
    /// A base window name this definition extends, if written first inside the parens.
    pub existing: Option<Ident>,
    /// Expressions that partition the input rows.
    pub partition_by: ThinVec<Expr<X>>,
    /// Ordering terms in source order.
    pub order_by: ThinVec<OrderByExpr<X>>,
    /// The frame clause (`ROWS`/`RANGE`/`GROUPS` …), if written.
    pub frame: Option<WindowFrame<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A window frame: `{ ROWS | RANGE | GROUPS } <extent> [ <exclusion> ]`.
///
/// `end` is `Some` only for the `BETWEEN <start> AND <end>` spelling; a bare
/// `<start>` bound leaves it `None`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct WindowFrame<X: Extension = NoExt> {
    /// Whether the frame is measured in `ROWS`/`RANGE`/`GROUPS`; see [`WindowFrameUnits`].
    pub units: WindowFrameUnits,
    /// The starting frame bound; see [`WindowFrameBound`].
    pub start: WindowFrameBound<X>,
    /// Optional end for this syntax.
    pub end: Option<WindowFrameBound<X>>,
    /// Optional exclusion for this syntax.
    pub exclusion: Option<WindowFrameExclusion>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL window frame units forms represented by the AST.
pub enum WindowFrameUnits {
    /// `ROWS` — the frame is counted in physical rows.
    Rows,
    /// `RANGE` — the frame spans rows with peer ordering-key values.
    Range,
    /// `GROUPS` — the frame is counted in peer groups.
    Groups,
}

/// One bound of a window frame.
///
/// The offset forms carry their `<expr> PRECEDING`/`FOLLOWING` distance; the
/// `UNBOUNDED`/`CURRENT ROW` forms carry their keyword span in `Meta`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum WindowFrameBound<X: Extension = NoExt> {
    /// `CURRENT ROW` — the bound at the current row.
    CurrentRow {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `UNBOUNDED PRECEDING` — the bound at the partition's first row.
    UnboundedPreceding {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `UNBOUNDED FOLLOWING` — the bound at the partition's last row.
    UnboundedFollowing {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<offset> PRECEDING` — a bound the given distance before the current row.
    Preceding {
        /// Row or frame offset selected by this syntax.
        offset: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<offset> FOLLOWING` — a bound the given distance after the current row.
    Following {
        /// Row or frame offset selected by this syntax.
        offset: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL window frame exclusion forms represented by the AST.
pub enum WindowFrameExclusion {
    /// `EXCLUDE CURRENT ROW` — omit the current row from the frame.
    CurrentRow,
    /// `EXCLUDE GROUP` — omit the current row and its peers.
    Group,
    /// `EXCLUDE TIES` — omit the current row's peers but keep the current row.
    Ties,
    /// `EXCLUDE NO OTHERS` — the default; exclude nothing.
    NoOthers,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL named window.
pub struct NamedWindow<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// The window definition bound to the name; see [`WindowDefinition`].
    pub definition: WindowDefinition<X>,
    /// Source location and node identity.
    pub meta: Meta,
}
