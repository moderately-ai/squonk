// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Transaction-control statement AST nodes (ADR-0012 operational family).

use super::{Ident, Literal};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// A transaction-control statement (SQL:2016 §17).
///
/// The operational statements clients send to bound a transaction: `START
/// TRANSACTION` (and its near-universal `BEGIN` alias), `COMMIT`, `ROLLBACK`
/// (optionally rewinding to a savepoint), `SAVEPOINT`, `RELEASE SAVEPOINT`, and
/// `SET TRANSACTION` characteristics.
///
/// One canonical shape per construct. `BEGIN` and `START TRANSACTION`
/// denote the same operation, so they share the [`Begin`](Self::Begin) shape and a
/// [`TransactionStart`] tag records which spelling was written. The interchangeable
/// `WORK` / `TRANSACTION` block noise words on `BEGIN`/`COMMIT`/`ROLLBACK` carry no
/// meaning; a [`TransactionBlockKeyword`] tag records which (if any) was written so a
/// source-fidelity render replays it, while a target re-spell and the redacted
/// fingerprint drop it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TransactionStatement {
    /// A `BEGIN` / `START TRANSACTION` statement.
    Begin {
        /// Source spelling used for the syntax.
        syntax: TransactionStart,
        /// SQLite's `{DEFERRED | IMMEDIATE | EXCLUSIVE}` transaction-mode modifier,
        /// written between `BEGIN` and the optional `TRANSACTION` keyword. `None`
        /// when the dialect does not admit one (or the statement omits it).
        mode: Option<TransactionModeKind>,
        /// The optional `TRANSACTION` / `WORK` block noise word after `BEGIN`. `None`
        /// for a bare `BEGIN`. Irrelevant (and `None`) under
        /// [`TransactionStart::Start`], whose mandatory `TRANSACTION` is part of the
        /// `START TRANSACTION` keyword.
        block: Option<TransactionBlockKeyword>,
        /// modes in source order.
        modes: ThinVec<TransactionMode>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `COMMIT` statement.
    Commit {
        /// The optional `TRANSACTION` / `WORK` block noise word after `COMMIT`.
        block: Option<TransactionBlockKeyword>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ROLLBACK [WORK | TRANSACTION] [TO [SAVEPOINT] <name>]`.
    ///
    /// `to_savepoint` is `Some` for the savepoint-rewind form and `None` for a
    /// whole-transaction rollback.
    Rollback {
        /// The optional `TRANSACTION` / `WORK` block noise word after `ROLLBACK`.
        block: Option<TransactionBlockKeyword>,
        /// Whether the optional `SAVEPOINT` keyword was written before the savepoint
        /// name (`ROLLBACK TO SAVEPOINT x` vs the bare `ROLLBACK TO x`). Meaningful
        /// only when `to_savepoint` is `Some`; the canonical render emits `SAVEPOINT`.
        savepoint_keyword: bool,
        /// Optional to savepoint for this syntax.
        to_savepoint: Option<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `SAVEPOINT <name>` statement.
    Savepoint {
        /// Name referenced by this syntax.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `RELEASE [SAVEPOINT] <name>` statement.
    Release {
        /// Whether the optional `SAVEPOINT` keyword was written (`RELEASE SAVEPOINT x`
        /// vs the bare `RELEASE x`). Exact-synonym fidelity; the canonical render emits
        /// `SAVEPOINT`.
        savepoint_keyword: bool,
        /// The savepoint name being released.
        savepoint: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET TRANSACTION <mode> [, ...]`: set the current transaction's
    /// characteristics (distinct from the session [`SET`](super::SessionStatement)).
    SetCharacteristics {
        /// modes in source order.
        modes: ThinVec<TransactionMode>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Which spelling started a transaction; the two are exact synonyms, so this is a
/// surface tag, not a separate shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TransactionStart {
    /// `BEGIN` — start a transaction.
    Begin,
    /// `START TRANSACTION` — start a transaction (the standard spelling).
    Start,
}

/// The optional block noise word written after `BEGIN` / `COMMIT` / `ROLLBACK`
/// (`TRANSACTION` or `WORK`): interchangeable synonyms carrying no meaning. The
/// canonical AST records which (if any) the source wrote so a source-fidelity render
/// replays it; a target re-spell and the redacted fingerprint drop it (the enclosing
/// `Option` is `None` for a bare keyword).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TransactionBlockKeyword {
    /// The `TRANSACTION` noise word.
    Transaction,
    /// The `WORK` noise word (a synonym for `TRANSACTION`).
    Work,
}

/// SQLite's `BEGIN` transaction-mode modifier (`sqlite-begin-transaction-modifiers`):
/// selects the locking behaviour SQLite uses to acquire the database lock. Distinct
/// from [`TransactionMode`] (the ANSI/PostgreSQL `START TRANSACTION`/`SET TRANSACTION`
/// isolation-level and access-mode list) — SQLite's three keywords are their own
/// closed, position-fixed vocabulary (immediately after `BEGIN`, before `TRANSACTION`),
/// not a mode list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TransactionModeKind {
    /// `BEGIN DEFERRED` — acquire the database lock lazily (SQLite).
    Deferred,
    /// `BEGIN IMMEDIATE` — acquire a reserved lock immediately (SQLite).
    Immediate,
    /// `BEGIN EXCLUSIVE` — acquire an exclusive lock immediately (SQLite).
    Exclusive,
}

/// One transaction mode in a `START TRANSACTION` / `SET TRANSACTION` mode list
/// (and in the session [`SET SESSION CHARACTERISTICS`](super::SessionStatement::SetSessionCharacteristics)).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TransactionMode {
    /// `ISOLATION LEVEL <level>` — set the transaction's isolation level.
    IsolationLevel {
        /// Which isolation level; see [`IsolationLevel`].
        level: IsolationLevel,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `READ ONLY` / `READ WRITE` — set the transaction's access mode.
    AccessMode {
        /// Which access mode; see [`TransactionAccessMode`].
        access: TransactionAccessMode,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DEFERRABLE` / `NOT DEFERRABLE`: the two spellings are one mode toggled by
    /// a flag; `deferrable` is `false` only for the `NOT` spelling.
    Deferrable {
        /// Whether the deferrable form was present in the source.
        deferrable: bool,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL isolation level forms represented by the AST.
pub enum IsolationLevel {
    /// `READ UNCOMMITTED` — the weakest isolation; permits dirty reads.
    ReadUncommitted,
    /// `READ COMMITTED` — only committed rows are visible.
    ReadCommitted,
    /// `REPEATABLE READ` — reads are stable for the transaction's duration.
    RepeatableRead,
    /// `SERIALIZABLE` — the strongest isolation; transactions appear fully serial.
    Serializable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL transaction access mode forms represented by the AST.
pub enum TransactionAccessMode {
    /// `READ ONLY` — the transaction may not modify data.
    ReadOnly,
    /// `READ WRITE` — the transaction may modify data (the default).
    ReadWrite,
}

/// A MySQL `XA` distributed-transaction statement — the X/Open XA two-phase-commit
/// verbs (`xa_transactions`; `sql_yacc.yy` `xa:`), a family distinct from the ANSI
/// [`TransactionStatement`] control statements above.
///
/// Every verb but [`Recover`](Self::Recover) names an [`Xid`] transaction-branch
/// identifier. The forms are grammar-recognized but not preparable over the wire
/// (live mysql:8.4.10 answers `ER_UNSUPPORTED_PS` 1295 to each), so they carry no
/// planner semantics here beyond their parsed shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XaStatement {
    /// `XA {START | BEGIN} xid [JOIN | RESUME]` — start (or JOIN/RESUME) a transaction
    /// branch. `START` and `BEGIN` are exact synonyms (`begin_or_start`), recorded by a
    /// [`XaStartKeyword`] surface tag.
    Start {
        /// Which of the `START` / `BEGIN` synonyms was written.
        keyword: XaStartKeyword,
        /// The transaction-branch identifier; see [`Xid`].
        xid: Xid,
        /// The optional `JOIN` / `RESUME` branch-association mode (`opt_join_or_resume`);
        /// `None` for a plain start.
        association: Option<XaAssociation>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `XA END xid [SUSPEND [FOR MIGRATE]]` — end (or suspend) the branch's active work.
    End {
        /// The transaction-branch identifier; see [`Xid`].
        xid: Xid,
        /// The optional `SUSPEND` / `SUSPEND FOR MIGRATE` mode (`opt_suspend`); `None`
        /// for a plain end.
        suspend: Option<XaSuspend>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `XA PREPARE xid` — prepare the branch for two-phase commit.
    Prepare {
        /// The transaction-branch identifier; see [`Xid`].
        xid: Xid,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `XA COMMIT xid [ONE PHASE]` — commit the branch (`ONE PHASE` commits without a
    /// prior `XA PREPARE`).
    Commit {
        /// The transaction-branch identifier; see [`Xid`].
        xid: Xid,
        /// Whether the `ONE PHASE` optimisation was written (`opt_one_phase`).
        one_phase: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `XA ROLLBACK xid` — roll the branch back.
    Rollback {
        /// The transaction-branch identifier; see [`Xid`].
        xid: Xid,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `XA RECOVER [CONVERT XID]` — list the branches the resource manager has prepared.
    Recover {
        /// Whether the `CONVERT XID` modifier was written (`opt_convert_xid`), which
        /// reports each `gtrid`/`bqual` as raw hexadecimal bytes.
        convert_xid: bool,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Which of the interchangeable `XA START` / `XA BEGIN` spellings began a transaction
/// branch. The two are exact synonyms (`begin_or_start`), so this is a surface tag, not
/// a separate shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XaStartKeyword {
    /// `XA START` — the standard spelling.
    Start,
    /// `XA BEGIN` — the synonym.
    Begin,
}

/// The optional `JOIN` / `RESUME` branch-association mode on `XA START` / `XA BEGIN`
/// (`opt_join_or_resume`): a closed two-keyword axis, valid only on the branch-start
/// verb (the engine rejects it on `END`/`COMMIT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XaAssociation {
    /// `JOIN` — join the named existing branch.
    Join,
    /// `RESUME` — resume the named suspended branch.
    Resume,
}

/// The optional `SUSPEND` mode on `XA END` (`opt_suspend`'s two non-empty forms):
/// `SUSPEND` alone, or `SUSPEND FOR MIGRATE`. `FOR MIGRATE` is valid only after
/// `SUSPEND`, so the two travel as one closed axis rather than an independent flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XaSuspend {
    /// `SUSPEND` — suspend the branch's work.
    Suspend,
    /// `SUSPEND FOR MIGRATE` — suspend for migration to another connection.
    SuspendForMigrate,
}

/// An XA transaction-branch identifier: `gtrid [, bqual [, formatID]]` (`sql_yacc.yy`
/// `xid`).
///
/// `gtrid` (the global transaction id) and `bqual` (the branch qualifier) are byte-string
/// constants — a character-string literal (`'…'`) or a hexadecimal / binary literal
/// (`0x…` / `X'…'` / `0b…` / `B'…'`), the `text_string` production; a bare decimal number
/// is not accepted. `format_id` is a non-negative numeric literal (`ulong_num`). Each part
/// rides its own [`Literal`] span, so the exact spelling round-trips and no owned value is
/// interned.
///
/// The engine additionally caps `gtrid`/`bqual` at 64 bytes and rejects a `format_id`
/// above `LONG_MAX` at parse time; those are value-length/range limits on the decoded
/// bytes (charset-dependent for a character string), not grammar shape, so they are left
/// to a binding pass rather than enforced here — the same treatment the parser gives other
/// literal magnitude limits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Xid {
    /// The global transaction id (`gtrid`) — a string or hex/binary literal.
    pub gtrid: Literal,
    /// The optional branch qualifier (`bqual`) — a string or hex/binary literal; `None`
    /// for a one-part xid.
    pub bqual: Option<Literal>,
    /// The optional format identifier (`formatID`) — a non-negative numeric literal;
    /// `None` unless a `bqual` was also written (the grammar admits it only in the
    /// three-part form).
    pub format_id: Option<Literal>,
    /// Source location and node identity.
    pub meta: Meta,
}
