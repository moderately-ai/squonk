// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! SQL rendering: the `Render` trait, `RenderCtx` / `RenderConfig` / `RenderMode`,
//! the `Displayed` wrapper, and the Tier-1 canonical renderer.
//!
//! Nodes are non-self-describing: an `Ident` is a `Symbol`, a literal is a byte
//! range. So `std::fmt::Display`, whose `fmt` takes no extra
//! arguments, cannot reach the resolver or source needed to spell a node. The
//! renderer therefore threads a [`RenderCtx`] through a dedicated [`Render`]
//! trait, and [`Displayed`] re-enables `Display` at any boundary by pairing a
//! node with its context.
//!
//! That canonical path is exact but strict: it assumes the resolver and source
//! match the node's parse, and panics on a symbol they cannot back. For the narrow
//! case of debugging a *detached* node whose context may not match, [`RenderCtx::debug`]
//! and the [`DebugSql`] wrapper (via [`RenderExt::debug_sql`]) add an opt-in,
//! resolver-explicit path that tolerates unresolvable symbols and spans with a
//! placeholder instead of panicking — the debug-SQL mitigation, realized as
//! an explicit argument rather than a hidden thread-local, so
//! normal rendering keeps no hidden global behaviour.

use crate::ast::{DataType, Expr, Extension, Query, Statement};
use crate::dialect::FeatureSet;
use crate::precedence::BindingPower;
use crate::vocab::{NodeId, Resolver, Span, Symbol};
use std::fmt;

mod dyn_ext;
mod nodes;

pub use dyn_ext::{DynAstExt, DynExt};
pub use nodes::{render_extension_infix, render_extension_prefix};

#[cfg(test)]
mod tests;

/// Render an AST node to SQL text using the resolver, source, and config carried
/// by `ctx`.
pub trait Render {
    /// Return the render for this value.
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result;

    /// The binding power this node contributes when it appears as an operand, or
    /// `None` (the default) for a self-delimiting node — an atom, call, or
    /// constructor — that never needs parentheses.
    ///
    /// An extension operator node (the `X` inside [`Expr::Other`]) overrides this to
    /// report its own precedence, so a custom-operator tree is parenthesized by the
    /// same binding-power rule as the built-in operators and round-trips through the
    /// renderer. The value MUST equal the binding power the dialect's
    /// `peek_infix_operator_hook` reported for the same operator, so parse-time
    /// climbing and render-time grouping stay one source of truth.
    ///
    /// [`Expr::Other`]: crate::ast::Expr::Other
    fn operand_binding_power(&self) -> Option<BindingPower> {
        None
    }
}

/// Fallible render result used by Tier-2 dialect-target renderers.
pub type RenderResult<T> = Result<T, RenderError>;

/// Why a fallible render operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderError {
    kind: RenderErrorKind,
    span: Option<Span>,
    message: String,
}

/// Stable category for render failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderErrorKind {
    /// The target dialect cannot express the AST construct being rendered.
    Unsupported,
    /// The underlying formatter reported an error.
    Format,
}

impl RenderError {
    /// Build an unsupported-target diagnostic.
    pub fn unsupported(span: Option<Span>, message: impl Into<String>) -> Self {
        Self {
            kind: RenderErrorKind::Unsupported,
            span,
            message: message.into(),
        }
    }

    /// The coarse render error category.
    pub fn kind(&self) -> RenderErrorKind {
        self.kind
    }

    /// The source span of the unsupported construct, when known.
    pub fn span(&self) -> Option<Span> {
        self.span
    }

    /// Human-readable diagnostic text.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.span {
            Some(span) if !span.is_synthetic() => {
                write!(
                    f,
                    "{} at bytes {}..{}",
                    self.message,
                    span.start(),
                    span.end()
                )
            }
            _ => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<fmt::Error> for RenderError {
    fn from(_error: fmt::Error) -> Self {
        Self {
            kind: RenderErrorKind::Format,
            span: None,
            message: "render formatter failed".to_owned(),
        }
    }
}

/// How a [`Render`] impl spells a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RenderMode {
    /// Round-trippable SQL with the minimal parentheses the binding-power table
    /// requires.
    #[default]
    Canonical,
    /// Every binary/unary subexpression fully parenthesized: the precedence
    /// oracle used by testing.
    Parenthesized,
    /// Identifier and literal *content* replaced by fixed placeholders — `id` for
    /// every identifier, `?` for every literal — while query *shape* is preserved,
    /// yielding a stable, PII-free query fingerprint.
    ///
    /// Two statements produce the **same** redacted string exactly when they differ
    /// only along a dimension masking erases:
    /// - identifier spelling: every name (column, table, alias, function, and each
    ///   qualified part) becomes `id`, so `a` and `b` coincide;
    /// - identifier quoting: the delimiters are dropped before the symbol is
    ///   resolved, so `a`, `"a"`, `` `a` ``, and `[a]` all become `id` (a keyword
    ///   spelled as an identifier masks the same way);
    /// - literal value: every literal becomes `?`, so `1` and `999`, `'x'` and a
    ///   secret string, `TRUE` and `NULL` coincide;
    /// - keyword casing: canonical rendering upper-cases keywords, so `select` and
    ///   `SELECT` coincide.
    ///
    /// Query shape is **kept**, so the fingerprint still separates statements that
    /// differ in clause structure, projection/list arity, qualified-name arity
    /// (`id` vs `id.id`), operators or keywords (`=` vs `<`, `AND` vs `OR`, `SELECT`
    /// vs `SELECT DISTINCT`), or the parentheses the binding-power table requires.
    ///
    /// The result is a fingerprint, **not** guaranteed re-parseable — only
    /// [`Canonical`](RenderMode::Canonical) round-trips. A masked literal renders as
    /// `?`, the anonymous-parameter sigil, which only lexes under a dialect that
    /// enables it (neither ANSI nor PostgreSQL do) and, even then, would re-parse as
    /// a *parameter* rather than the literal it replaced. An identifier-only
    /// redaction happens to re-parse, but that is incidental, not a promise.
    Redacted,
}

/// How renderable surface tags are spelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RenderSpelling {
    /// Preserve the AST's source spelling tags while emitting normalized uppercase SQL.
    #[default]
    PreserveSource,
    /// Prefer spellings for the explicit target [`FeatureSet`].
    TargetDialect,
}

/// Tunable rendering options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderConfig {
    /// Mode selected by this syntax.
    pub mode: RenderMode,
    /// Object targeted by this syntax.
    pub target: FeatureSet,
    /// Exact source spelling retained for faithful rendering.
    pub spelling: RenderSpelling,
}

impl RenderConfig {
    /// The canonical default, as a `const` so a borrow of it (`&RenderConfig::DEFAULT`)
    /// promotes to `'static` (rvalue static promotion) instead of living only as long
    /// as some local binding. [`RenderExt::debug_sql`] needs exactly that: it builds a
    /// [`RenderCtx`] with no caller-supplied config to borrow, and the [`DebugSql`] it
    /// returns must keep that ctx alive past the constructing call, so a local
    /// `RenderConfig::default()` temporary would not outlive the borrow.
    const DEFAULT: RenderConfig = RenderConfig {
        mode: RenderMode::Canonical,
        target: FeatureSet::ANSI,
        spelling: RenderSpelling::PreserveSource,
    };
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Placeholder spelled for a [`Symbol`] the debug renderer cannot resolve.
///
/// Deliberately not a valid bare identifier (the angle brackets are operators), so
/// a foreign or unknown symbol reads as an obvious debug artifact rather than a
/// plausible-but-wrong name. Only the opt-in [`RenderCtx::debug`] path emits it;
/// canonical rendering panics on an unresolvable symbol instead.
const UNRESOLVED_SYMBOL: &str = "<unresolved>";

/// Whether a [`RenderCtx`] takes the canonical render path or the opt-in debug path.
///
/// The distinction is confined to [`RenderCtx::resolve`] and [`RenderCtx::slice`]:
/// every [`Render`] body is written once and is unaware of it. Canonical is the
/// default; the debug path is reached only through [`RenderCtx::debug`] /
/// [`RenderExt::debug_sql`], so normal rendering keeps no hidden global behaviour.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextKind {
    /// Canonical rendering: resolution is strict — a symbol foreign to
    /// the resolver is a caller bug, so it panics — and literals slice their exact
    /// source spelling.
    Canonical,
    /// Opt-in debug rendering: resolution is tolerant — a symbol the resolver does
    /// not know renders the `UNRESOLVED_SYMBOL` placeholder instead of panicking — and literals
    /// never slice source (a detached node's spans may index a *different* string),
    /// so they fall back to their kind-based spelling.
    Debug,
}

/// Resolver, source, and config threaded through every [`Render`] call.
///
/// The [`Resolver`] trait (not the concrete interner) lives in this crate, so the
/// renderer stays independent of parser internals. `config` is *borrowed*, not
/// owned: a multi-statement render builds one [`RenderCtx`] per statement (or per
/// render — see the parser crate's `Parsed::to_sql`), and `RenderConfig` is large
/// enough — it embeds the whole target [`FeatureSet`], keyword/precedence tables and
/// all — that owning a fresh copy each time was a real per-statement memcpy of data
/// that never changes across a render.
pub struct RenderCtx<'a> {
    resolver: &'a dyn Resolver,
    source: &'a str,
    config: &'a RenderConfig,
    kind: ContextKind,
}

impl<'a> RenderCtx<'a> {
    /// Build a canonical context from a resolver, the original source text, and
    /// config.
    ///
    /// This is the canonical render path: the resolver and source must
    /// come from the *same* parse as the node, because resolution is strict (a
    /// foreign symbol panics) and literals slice their exact source spelling. Prefer
    /// it — and the [`Displayed`] wrapper or the parser crate's directly-`Display`
    /// `Parsed` root over it — whenever the matched context is in hand. For a
    /// *detached* node whose context may not match, reach for the tolerant
    /// [`debug`](Self::debug) sibling instead.
    ///
    /// `config` is borrowed, so building a ctx per statement (a multi-statement
    /// render's loop) costs copying a pointer, never a `RenderConfig` copy.
    pub fn new(resolver: &'a dyn Resolver, source: &'a str, config: &'a RenderConfig) -> Self {
        Self {
            resolver,
            source,
            config,
            kind: ContextKind::Canonical,
        }
    }

    /// Build the opt-in *debug* context: an explicitly-supplied resolver with
    /// tolerant resolution and no source slicing (the debug-SQL mitigation).
    ///
    /// Use this — or the [`RenderExt::debug_sql`] convenience over it — to render a
    /// *detached* or *synthesized* node for debugging, where the canonical
    /// [`new`](Self::new) path's guarantees may not hold:
    ///
    /// - The resolver is an **explicit argument**, never a hidden global or default,
    ///   so debug rendering can never *silently* pick the wrong resolver — the choice
    ///   is always visible at the call site. (A resolver from a *different* parse can
    ///   still mis-resolve a numerically-colliding symbol; only the canonical path,
    ///   which travels with its own resolver, rules that out. See [`debug_sql`].)
    /// - A symbol the resolver does not know renders the `UNRESOLVED_SYMBOL` placeholder instead of
    ///   panicking, so a partially-detached tree renders to completion.
    /// - Source is deliberately **not** an argument: debug rendering never slices it,
    ///   so a literal always uses its kind-based spelling and can never emit
    ///   unrelated bytes from a mismatched source. For exact literal text, use the
    ///   canonical path, which owns the matched source.
    ///
    /// [`debug_sql`]: RenderExt::debug_sql
    pub fn debug(resolver: &'a dyn Resolver, config: &'a RenderConfig) -> Self {
        Self {
            resolver,
            // Debug rendering never slices source (see `slice`); the field is unused
            // on this path, so an empty borrow keeps source out of the debug API.
            source: "",
            config,
            kind: ContextKind::Debug,
        }
    }

    /// The resolver used to turn [`Symbol`]s back into identifier text.
    pub fn resolver(&self) -> &dyn Resolver {
        self.resolver
    }

    /// The original source text that literals are sliced from.
    pub fn source(&self) -> &str {
        self.source
    }

    /// The active render configuration.
    pub fn config(&self) -> &RenderConfig {
        self.config
    }

    /// The active render mode.
    pub fn mode(&self) -> RenderMode {
        self.config.mode
    }

    /// The active target dialect feature set.
    pub fn target(&self) -> &FeatureSet {
        &self.config.target
    }

    /// The active spelling policy.
    pub fn spelling(&self) -> RenderSpelling {
        self.config.spelling
    }

    /// Resolve `sym` to its identifier text.
    ///
    /// # Panics
    ///
    /// On the canonical path ([`new`](Self::new)), panics if `sym` was not produced
    /// by this context's resolver; a node and its resolver must come from the same
    /// parse. The debug path ([`debug`](Self::debug)) never panics — an unknown
    /// symbol resolves to the `UNRESOLVED_SYMBOL` placeholder instead.
    fn resolve(&self, sym: Symbol) -> &str {
        match self.kind {
            ContextKind::Canonical => self.resolver.resolve(sym),
            ContextKind::Debug => self.resolver.try_resolve(sym).unwrap_or(UNRESOLVED_SYMBOL),
        }
    }

    /// Slice the source covered by `span`, or `None` when there is no backing text.
    ///
    /// Returns `None` for a synthetic / out-of-range span (a detached or
    /// rewrite-synthesized node has no source text), and always for the debug path
    /// ([`debug`](Self::debug)), which never trusts a detached node's spans against a
    /// possibly-mismatched source. A `None` result is total: [`Literal`] falls back
    /// to a kind-based spelling rather than panicking.
    ///
    /// [`Literal`]: crate::ast::Literal
    fn slice(&self, span: Span) -> Option<&str> {
        if self.kind == ContextKind::Debug || span.is_synthetic() {
            return None;
        }
        self.source.get(span.start() as usize..span.end() as usize)
    }
}

/// `Display` adapter pairing a node with its [`RenderCtx`] so `format!`,
/// `to_string`, and `{}` work at any boundary.
pub struct Displayed<'a, T: Render>(&'a T, &'a RenderCtx<'a>);

impl<T: Render> fmt::Display for Displayed<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.render(self.1, f)
    }
}

/// `Display` adapter for the opt-in debug path: pairs a node with a resolver-only,
/// tolerant [`RenderCtx`] so a *detached* node can be printed for debugging without
/// building a full context (the debug-SQL mitigation).
///
/// Unlike [`Displayed`], this owns its context (built from the resolver alone), so
/// [`RenderExt::debug_sql`] can return it directly. Rendering is tolerant: an
/// unknown symbol becomes the `UNRESOLVED_SYMBOL` placeholder and literals use their kind-based
/// spelling, so it never panics on a symbol or span the context cannot back. See
/// [`debug_sql`](RenderExt::debug_sql) for when to prefer the canonical path.
pub struct DebugSql<'a, T: Render> {
    node: &'a T,
    ctx: RenderCtx<'a>,
}

impl<T: Render> fmt::Display for DebugSql<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.node.render(&self.ctx, f)
    }
}

/// Ergonomic constructors for the [`Displayed`] and [`DebugSql`] adapters,
/// blanket-impl'd for every [`Render`] type.
pub trait RenderExt: Render {
    /// Pair this node with an explicit canonical [`RenderCtx`] so `format!`,
    /// `to_string`, and `{}` render it. This is the canonical path: the
    /// `ctx`'s resolver and source must match the node's parse.
    fn displayed<'a>(&'a self, ctx: &'a RenderCtx<'a>) -> Displayed<'a, Self>
    where
        Self: Sized;

    /// Render this node for **debugging** against an explicitly-supplied `resolver`
    /// (the debug-SQL mitigation), returning a `Display` adapter.
    ///
    /// Reach for this only for a *detached* or *synthesized* node — one not behind a
    /// `Parsed` root and possibly holding symbols the `resolver` on hand does not
    /// own. **Prefer the canonical path first:** the `Parsed` root's `Display` and
    /// [`displayed`](Self::displayed) render exact SQL because they travel with the
    /// matched source and resolver, and cannot mismatch.
    ///
    /// This helper trades exactness for safety on a detached node:
    ///
    /// - The `resolver` is an **explicit argument** — never a hidden thread-local,
    ///   global, or default — so it can never *silently* render with the wrong
    ///   resolver: the choice is always written at the call site.
    /// - A symbol the `resolver` does not know renders as `<unresolved>` rather than
    ///   panicking, so a partially-detached tree still prints to completion.
    /// - Literals use their kind-based spelling (`0`, `''`, `NULL`, …): debug
    ///   rendering never slices source, so it cannot emit unrelated bytes.
    ///
    /// **Limitation.** An *explicit* argument cannot detect a resolver from a
    /// *different* parse: a symbol whose numeric id happens to collide with an entry
    /// in that resolver resolves to the *other* parse's text. The helper prevents a
    /// silent *choice* of the wrong resolver and turns *unknown* symbols into a
    /// visible placeholder, but only the canonical path — which owns its own matched
    /// resolver — rules a colliding foreign resolver out entirely.
    fn debug_sql<'a>(&'a self, resolver: &'a dyn Resolver) -> DebugSql<'a, Self>
    where
        Self: Sized,
    {
        DebugSql {
            node: self,
            // `&RenderConfig::DEFAULT` promotes to `'static` (it is a bare `const`
            // path, the case `rustc` always promotes), which is what lets an owned
            // `RenderCtx<'a>` borrow a config here with no caller-supplied place to
            // borrow one from.
            ctx: RenderCtx::debug(resolver, &RenderConfig::DEFAULT),
        }
    }
}

impl<T: Render> RenderExt for T {
    fn displayed<'a>(&'a self, ctx: &'a RenderCtx<'a>) -> Displayed<'a, Self> {
        Displayed(self, ctx)
    }
}

/// The audited allowlist of node kinds that render to a **self-contained** SQL
/// fragment — a marker trait sealed against outside impls so the set stays a
/// closed, reviewable list.
///
/// Every AST node has a [`Render`] impl, but most render only a *piece* of a larger
/// construct that is meaningless on its own: a bare `JoinConstraint` renders `ON a =
/// b` (no join), a `SelectItem` renders `a AS x` (no `SELECT`), an `OrderByExpr`
/// renders `a DESC` (no `ORDER BY`). Handing those to a fragment API would emit
/// plausible-looking but context-dependent SQL a consumer cannot re-parse. The
/// fragment entry points ([`Parsed::render_fragment`]) therefore accept only the
/// kinds in this list, which each stand alone:
///
/// - [`Expr`] — a complete scalar expression;
/// - [`Query`] — a complete `SELECT` / set-operation / `VALUES` query;
/// - [`Statement`] — a complete statement;
/// - [`DataType`] — a complete type spelling (as it appears after `CAST(x AS …)` or
///   in a column definition).
///
/// The gate is compile-time: a call site can only pass one of these types, so a
/// context-dependent node is rejected by the type checker rather than silently
/// rendered. The runtime sibling for language facades, which hold node handles by
/// id, is [`Parsed::render_fragment_by_id`]; it enforces the *same* allowlist
/// dynamically.
///
/// [`Parsed::render_fragment`]: https://docs.rs/squonk
/// [`Parsed::render_fragment_by_id`]: https://docs.rs/squonk
pub trait FragmentRender: Render + fragment_sealed::Sealed {
    /// This node's [`NodeId`], used by the by-id fragment lookup to match a node
    /// handle held by a language facade. Implementation detail of the fragment API —
    /// prefer the node's own [`Meta`](crate::vocab::Meta) `node_id` field directly.
    #[doc(hidden)]
    fn fragment_node_id(&self) -> NodeId;
}

/// Seals [`FragmentRender`] so the standalone-renderable allowlist cannot be widened
/// from outside this crate — only the audited node kinds below implement `Sealed`.
mod fragment_sealed {
    pub trait Sealed {}
}

impl<X: Extension + Render> fragment_sealed::Sealed for Query<X> {}
impl<X: Extension + Render> FragmentRender for Query<X> {
    fn fragment_node_id(&self) -> NodeId {
        self.meta.node_id
    }
}

// `Statement` is `#[non_exhaustive]`, so a cross-crate match would need a `_` arm
// that could not report a new variant's id. Extracting every fragment id here also
// keeps the `Expr` and `DataType` matches exhaustive, so adding any fragment variant
// is a compile error to update rather than a silently unrenderable node (the
// completeness guarantee the generated `NodeIdWalk` relies on for the same reason).
impl<X: Extension + Render> fragment_sealed::Sealed for Statement<X> {}
impl<X: Extension + Render> FragmentRender for Statement<X> {
    fn fragment_node_id(&self) -> NodeId {
        match self {
            Statement::Query { meta, .. }
            | Statement::CreateTable { meta, .. }
            | Statement::AlterTable { meta, .. }
            | Statement::Drop { meta, .. }
            | Statement::CreateSchema { meta, .. }
            | Statement::CreateView { meta, .. }
            | Statement::AlterView { meta, .. }
            | Statement::CreateIndex { meta, .. }
            | Statement::CreateFunction { meta, .. }
            | Statement::CreateProcedure { meta, .. }
            | Statement::AlterRoutine { meta, .. }
            | Statement::CreateEvent { meta, .. }
            | Statement::AlterEvent { meta, .. }
            | Statement::DropEvent { meta, .. }
            | Statement::DropDatabase { meta, .. }
            | Statement::DropIndex { meta, .. }
            | Statement::CreateDatabase { meta, .. }
            | Statement::DropRoutine { meta, .. }
            | Statement::DropTransform { meta, .. }
            | Statement::Truncate { meta, .. }
            | Statement::CommentOn { meta, .. }
            | Statement::Insert { meta, .. }
            | Statement::Update { meta, .. }
            | Statement::Delete { meta, .. }
            | Statement::Merge { meta, .. }
            | Statement::Transaction { meta, .. }
            | Statement::Xa { meta, .. }
            | Statement::Session { meta, .. }
            | Statement::AccessControl { meta, .. }
            | Statement::Copy { meta, .. }
            | Statement::CopyInto { meta, .. }
            | Statement::Export { meta, .. }
            | Statement::Import { meta, .. }
            | Statement::Explain { meta, .. }
            | Statement::Describe { meta, .. }
            | Statement::Show { meta, .. }
            | Statement::Kill { meta, .. }
            | Statement::Handler { meta, .. }
            | Statement::Install { meta, .. }
            | Statement::Uninstall { meta, .. }
            | Statement::Shutdown { meta, .. }
            | Statement::Restart { meta, .. }
            | Statement::Clone { meta, .. }
            | Statement::ImportTable { meta, .. }
            | Statement::Help { meta, .. }
            | Statement::Binlog { meta, .. }
            | Statement::Pragma { meta, .. }
            | Statement::Attach { meta, .. }
            | Statement::Detach { meta, .. }
            | Statement::Checkpoint { meta, .. }
            | Statement::Load { meta, .. }
            | Statement::LoadData { meta, .. }
            | Statement::UpdateExtensions { meta, .. }
            | Statement::Vacuum { meta, .. }
            | Statement::Reindex { meta, .. }
            | Statement::Analyze { meta, .. }
            | Statement::Use { meta, .. }
            | Statement::CreateTrigger { meta, .. }
            | Statement::CreateStoredTrigger { meta, .. }
            | Statement::CreateMacro { meta, .. }
            | Statement::CreateSecret { meta, .. }
            | Statement::DropSecret { meta, .. }
            | Statement::CreateType { meta, .. }
            | Statement::CreateVirtualTable { meta, .. }
            | Statement::CreateSequence { meta, .. }
            | Statement::CreateExtension { meta, .. }
            | Statement::AlterExtension { meta, .. }
            | Statement::CreateTablespace { meta, .. }
            | Statement::AlterTablespace { meta, .. }
            | Statement::DropTablespace { meta, .. }
            | Statement::CreateLogfileGroup { meta, .. }
            | Statement::AlterLogfileGroup { meta, .. }
            | Statement::DropLogfileGroup { meta, .. }
            | Statement::AlterObjectDepends { meta, .. }
            | Statement::AlterSystem { meta, .. }
            | Statement::AlterDatabase { meta, .. }
            | Statement::AlterDatabaseOptions { meta, .. }
            | Statement::CreateServer { meta, .. }
            | Statement::AlterServer { meta, .. }
            | Statement::DropServer { meta, .. }
            | Statement::AlterInstance { meta, .. }
            | Statement::CreateSpatialReferenceSystem { meta, .. }
            | Statement::DropSpatialReferenceSystem { meta, .. }
            | Statement::CreateResourceGroup { meta, .. }
            | Statement::AlterResourceGroup { meta, .. }
            | Statement::DropResourceGroup { meta, .. }
            | Statement::AlterSequence { meta, .. }
            | Statement::AlterObjectSchema { meta, .. }
            | Statement::Pivot { meta, .. }
            | Statement::Unpivot { meta, .. }
            | Statement::ShowRef { meta, .. }
            | Statement::Prepare { meta, .. }
            | Statement::Execute { meta, .. }
            | Statement::PrepareFrom { meta, .. }
            | Statement::ExecuteUsing { meta, .. }
            | Statement::Deallocate { meta, .. }
            | Statement::Call { meta, .. }
            | Statement::Do { meta, .. }
            | Statement::DoExpressions { meta, .. }
            | Statement::LockTables { meta, .. }
            | Statement::UnlockTables { meta, .. }
            | Statement::InstanceLock { meta, .. }
            | Statement::Compound { meta, .. }
            | Statement::If { meta, .. }
            | Statement::Case { meta, .. }
            | Statement::Loop { meta, .. }
            | Statement::While { meta, .. }
            | Statement::Repeat { meta, .. }
            | Statement::Leave { meta, .. }
            | Statement::Iterate { meta, .. }
            | Statement::Return { meta, .. }
            | Statement::OpenCursor { meta, .. }
            | Statement::FetchCursor { meta, .. }
            | Statement::CloseCursor { meta, .. }
            | Statement::TableMaintenance { meta, .. }
            | Statement::CacheIndex { meta, .. }
            | Statement::LoadIndex { meta, .. }
            | Statement::Rename { meta, .. }
            | Statement::Flush { meta, .. }
            | Statement::Purge { meta, .. }
            | Statement::Replication { meta, .. }
            | Statement::CreateUser { meta, .. }
            | Statement::AlterUser { meta, .. }
            | Statement::UserRoleList { meta, .. }
            | Statement::Signal { meta, .. }
            | Statement::Resignal { meta, .. }
            | Statement::GetDiagnostics { meta, .. }
            | Statement::Other { meta, .. } => meta.node_id,
        }
    }
}

impl<X: Extension + Render> fragment_sealed::Sealed for Expr<X> {}
impl<X: Extension + Render> FragmentRender for Expr<X> {
    fn fragment_node_id(&self) -> NodeId {
        match self {
            Expr::Column { meta, .. }
            | Expr::Literal { meta, .. }
            | Expr::BinaryOp { meta, .. }
            | Expr::UnaryOp { meta, .. }
            | Expr::Function { meta, .. }
            | Expr::Case { meta, .. }
            | Expr::Extract { meta, .. }
            | Expr::Cast { meta, .. }
            | Expr::IsNull { meta, .. }
            | Expr::IsTruth { meta, .. }
            | Expr::IsNormalized { meta, .. }
            | Expr::Between { meta, .. }
            | Expr::Like { meta, .. }
            | Expr::InList { meta, .. }
            | Expr::InSubquery { meta, .. }
            | Expr::InExpr { meta, .. }
            | Expr::Exists { meta, .. }
            | Expr::QuantifiedComparison { meta, .. }
            | Expr::QuantifiedList { meta, .. }
            | Expr::QuantifiedLike { meta, .. }
            | Expr::Subquery { meta, .. }
            | Expr::Parameter { meta, .. }
            | Expr::PositionalColumn { meta, .. }
            | Expr::SessionVariable { meta, .. }
            | Expr::Subscript { meta, .. }
            | Expr::SemiStructuredAccess { meta, .. }
            | Expr::Collate { meta, .. }
            | Expr::AtTimeZone { meta, .. }
            | Expr::Interval { meta, .. }
            | Expr::Array { meta, .. }
            | Expr::Struct { meta, .. }
            | Expr::StructConstructor { meta, .. }
            | Expr::Map { meta, .. }
            | Expr::Row { meta, .. }
            | Expr::FieldSelection { meta, .. }
            | Expr::NamedOperator { meta, .. }
            | Expr::PrefixOperator { meta, .. }
            | Expr::PostfixOperator { meta, .. }
            | Expr::Lambda { meta, .. }
            | Expr::Columns { meta, .. }
            | Expr::SpecialFunction { meta, .. }
            | Expr::JsonFunc { meta, .. }
            | Expr::JsonObject { meta, .. }
            | Expr::JsonArray { meta, .. }
            | Expr::JsonAggregate { meta, .. }
            | Expr::JsonConstructor { meta, .. }
            | Expr::IsJson { meta, .. }
            | Expr::XmlFunc { meta, .. }
            | Expr::IsDocument { meta, .. }
            | Expr::StringFunc { meta, .. }
            | Expr::Other { meta, .. } => meta.node_id,
        }
    }
}

impl<X: Extension + Render> fragment_sealed::Sealed for DataType<X> {}
impl<X: Extension + Render> FragmentRender for DataType<X> {
    fn fragment_node_id(&self) -> NodeId {
        match self {
            DataType::Boolean { meta, .. }
            | DataType::TinyInt { meta, .. }
            | DataType::SmallInt { meta, .. }
            | DataType::MediumInt { meta, .. }
            | DataType::Integer { meta, .. }
            | DataType::BigInt { meta, .. }
            | DataType::Decimal { meta, .. }
            | DataType::Float { meta, .. }
            | DataType::Real { meta, .. }
            | DataType::Double { meta, .. }
            | DataType::Text { meta, .. }
            | DataType::Blob { meta, .. }
            | DataType::Character { meta, .. }
            | DataType::Binary { meta, .. }
            | DataType::Bit { meta, .. }
            | DataType::Json { meta, .. }
            | DataType::Uuid { meta, .. }
            | DataType::Date { meta, .. }
            | DataType::Time { meta, .. }
            | DataType::Timestamp { meta, .. }
            | DataType::Interval { meta, .. }
            | DataType::Enum { meta, .. }
            | DataType::Set { meta, .. }
            | DataType::NumericModifier { meta, .. }
            | DataType::Array { meta, .. }
            | DataType::Struct { meta, .. }
            | DataType::Union { meta, .. }
            | DataType::Map { meta, .. }
            | DataType::Wrapped { meta, .. }
            | DataType::FixedString { meta, .. }
            | DataType::DateTime64 { meta, .. }
            | DataType::Nested { meta, .. }
            | DataType::FixedWidthInt { meta, .. }
            | DataType::UserDefined { meta, .. }
            | DataType::Liberal { meta, .. }
            | DataType::Other { meta, .. } => meta.node_id,
        }
    }
}
