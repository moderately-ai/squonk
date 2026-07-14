// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The recursive-descent + Pratt parser engine.
//!
//! `m1-engine` laid the central integration: the [`Parser`] engine and its seams
//! (`engine`), the owned [`Parsed`] root (`parsed`), the thin [`Dialect`]
//! trait, and the [`parse_with`] entry point. The grammar is split by family over
//! those seams: the Pratt expression core (`expr`, `m1-pratt-expr`)
//! and the query grammar — statement dispatch and query-level clauses (`query`),
//! the SELECT body (`select`), and the `FROM` relations and joins (`from`),
//! together `m1-select-grammar`.
//!
//! # Parser module ownership
//!
//! The grammar is filed by *grammar family*, and the load-bearing rule is
//! that **each family's dispatcher lives with its helpers**. The prior art's split
//! regressed precisely because a family's dispatcher stayed behind in the central
//! module while its helpers moved out, so the central module re-grew toward the
//! 21k-line monolith. Here the central `parse_statement` router in `query` only
//! recognizes a statement's leading token and delegates to a family entry; it never
//! parses a family's body inline. Each family entry is a `pub(super)` method on
//! `Parser<D>` defined in — and unit-tested from — its owning module, so the
//! dispatch boundary stays small and every family stays a unit-testable seam:
//!
//! - `query` — statement dispatch (the router) plus the query-level grammar: set
//!   operations, `ORDER BY`, `LIMIT`/`OFFSET`, `VALUES`, and the `WITH`/CTE clause.
//! - `select` — the SELECT body: projection, `WHERE`, `GROUP BY`, `HAVING`.
//! - `from` — the `FROM` relation grammar: table factors, joins, qualified names.
//! - `expr` — the Pratt expression core over the one binding-power table.
//! - `ddl` — `CREATE` (table/schema/view/index), `ALTER`, and `DROP`.
//! - `dml` — `INSERT`, `UPDATE`, and `DELETE`.
//! - `dcl` — session configuration (`SET`/`RESET`/`SHOW`) and access control
//!   (`GRANT`/`REVOKE`).
//! - `tcl` — transaction control (`BEGIN`/`START`/`COMMIT`/`ROLLBACK`/`SAVEPOINT`/
//!   `RELEASE`/`SET TRANSACTION`).
//! - `util` — the utility statements `COPY` and `EXPLAIN`.
//!
//! `ty` (type names) and `window` (the `OVER` clause) are cross-family
//! sub-grammars, not statement dispatchers: they own no dispatch arm and are reached
//! from whichever family needs them. A new statement family adds one arm to the
//! `query` router and its dispatcher-plus-helpers as a new module — never a body
//! of family-specific parsing in `query` itself.

mod body;
mod clause_marks;
mod dcl;
mod ddl;
mod dml;
mod engine;
mod expr;
mod from;
mod match_recognize;
mod parsed;
mod pivot;
mod query;
mod recovery;
mod select;
mod signal;
mod streaming;
mod tcl;
mod ty;
mod util;
mod window;

/// Recursion-depth guard acceptance tests (DoS-safety); tests only.
#[cfg(test)]
mod recursion;

/// Whole-tree `NodeId` allocation invariant; tests only.
#[cfg(test)]
mod node_id;

/// Extension operator precedence through typed hooks; tests only.
#[cfg(test)]
mod extension_operators;

/// The dynamic `DynExt` extension hatch end-to-end; tests only.
#[cfg(test)]
mod dyn_extension;

pub use clause_marks::{ClauseKw, ClauseMark, ClauseMarkIndex};
pub use engine::{Checkpoint, DEFAULT_RECURSION_LIMIT, Parser};
pub use parsed::{Parsed, StockParsed};
pub use recovery::{Recovered, parse_recovering, parse_recovering_with};
pub use streaming::Statements;

use std::rc::Rc;
use std::sync::Arc;

use crate::ast::dialect::FeatureSet;
use crate::ast::precedence::BindingPower;
use crate::ast::{
    ColumnOption, DataType, Expr, Extension, SourceStore, Statement, TableConstraint, TableFactor,
};
use crate::error::{ParseError, ParseResult};
use crate::tokenizer::{LexError, Token};

/// Typed 3-state result returned by dialect hooks.
///
/// Hooks can claim a production with [`Handled`](Self::Handled), decline it with
/// [`NotHandled`](Self::NotHandled), or surface a parser-owned diagnostic with
/// [`Err`](Self::Err). Parser call sites must branch on all three states, which
/// keeps dialect extension points explicit and avoids boolean-soup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookResult<T> {
    /// The dialect parsed this production and returned its AST node.
    Handled(T),
    /// The dialect does not handle the production at the current cursor.
    ///
    /// A hook that returns this state must leave the token cursor unchanged.
    /// Consumed input means the hook recognized enough syntax to return either
    /// [`Handled`](Self::Handled) or [`Err`](Self::Err).
    NotHandled,
    /// The dialect recognized the production but parsing it failed.
    Err(ParseError),
}

/// A SQL dialect: const feature data plus the AST extension it produces.
///
/// A dialect is *data*: [`features`](Self::features) returns a const
/// [`FeatureSet`] the parser reads field by field, which const-folds under the
/// monomorphized `Parser<D>`. [`Ext`](Self::Ext) is the custom node type a
/// dialect's `Other(X)` variants carry; the stock dialects use
/// [`NoExt`](crate::ast::NoExt).
///
/// Hooks are static functions over `Parser<Self>` so grammar code can invoke
/// them without borrowing a dialect field out of the parser while also passing
/// the parser mutably. Dialects that do not extend a production return
/// [`HookResult::NotHandled`] without consuming input.
///
/// # Example: a custom infix operator
///
/// A dialect adds an infix operator over a lexeme the core grammar leaves free (`~`,
/// `^`, `&`, `|`, or a bare word) through the typed operator hooks:
/// [`peek_infix_operator_hook`](Self::peek_infix_operator_hook) only *recognizes* the
/// operator and reports its [`BindingPower`], and the parser owns the precedence climb
/// — so a custom operator cannot mis-bind its operands. The node lands in
/// [`Expr::Other`] and round-trips
/// through its [`Render`](crate::ast::render::Render) impl.
///
/// ```
/// use std::fmt;
/// use squonk::ast::precedence::{Assoc, BindingPower};
/// use squonk::ast::render::{Render, RenderCtx, render_extension_infix};
/// use squonk::ast::{Expr, Span, Spanned};
/// use squonk::error::ParseResult;
/// use squonk::parser::{Dialect, Parser};
/// use squonk::tokenizer::{Operator, Token, TokenKind};
/// use squonk::{ParseConfig, parse_with};
///
/// // A left-associative `^` "match" operator.
/// const MATCH_BP: BindingPower = BindingPower { left: 64, right: 65, assoc: Assoc::Left };
///
/// #[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// struct Match {
///     left: Box<Expr<Match>>,
///     right: Box<Expr<Match>>,
///     span: Span,
/// }
///
/// impl Spanned for Match {
///     fn span(&self) -> Span {
///         self.span
///     }
/// }
/// impl Render for Match {
///     fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         render_extension_infix(ctx, f, MATCH_BP, (&self.left, &self.right), |f| f.write_str(" ^ "))
///     }
///     fn operand_binding_power(&self) -> Option<BindingPower> {
///         Some(MATCH_BP)
///     }
/// }
///
/// #[derive(Clone, Copy)]
/// struct MatchDialect;
/// impl Dialect for MatchDialect {
///     type Ext = Match;
///     fn features(&self) -> &squonk::ast::dialect::FeatureSet {
///         &squonk::ast::dialect::FeatureSet::ANSI
///     }
///     // Recognize `^` at the cursor and report its binding power; the parser climbs
///     // the operands and hands them back to `build_infix_operator`.
///     fn peek_infix_operator_hook<'a>(
///         parser: &mut Parser<'a, Self>,
///     ) -> ParseResult<Option<BindingPower>> {
///         Ok(match parser.peek()? {
///             Some(token) if token.kind == TokenKind::Operator(Operator::Caret) => Some(MATCH_BP),
///             _ => None,
///         })
///     }
///     fn build_infix_operator<'a>(
///         parser: &mut Parser<'a, Self>,
///         _op: Token,
///         left: Expr<Match>,
///         right: Expr<Match>,
///     ) -> ParseResult<Expr<Match>> {
///         let span = left.span().union(right.span());
///         let ext = Match { left: Box::new(left), right: Box::new(right), span };
///         let meta = parser.make_meta(span);
///         Ok(Expr::Other { ext, meta })
///     }
/// }
///
/// // `^` parses and round-trips through the custom node's Render impl.
/// let parsed = parse_with("SELECT a ^ b", ParseConfig::new(MatchDialect)).expect("custom operator parses");
/// assert_eq!(parsed.to_string(), "SELECT a ^ b");
/// ```
pub trait Dialect {
    /// The custom AST node type produced by this dialect's `Other(X)` variants.
    type Ext: Extension;

    /// The dialect's const feature data.
    fn features(&self) -> &FeatureSet;

    /// Optionally parse a statement at the current cursor.
    fn parse_statement_hook<'a>(_parser: &mut Parser<'a, Self>) -> HookResult<Statement<Self::Ext>>
    where
        Self: Sized,
    {
        HookResult::NotHandled
    }

    /// Optionally parse a prefix expression or primary at the current cursor.
    fn parse_prefix_expr_hook<'a>(_parser: &mut Parser<'a, Self>) -> HookResult<Expr<Self::Ext>>
    where
        Self: Sized,
    {
        HookResult::NotHandled
    }

    /// Optionally parse a table factor at the current cursor.
    fn parse_table_factor_hook<'a>(
        _parser: &mut Parser<'a, Self>,
    ) -> HookResult<TableFactor<Self::Ext>>
    where
        Self: Sized,
    {
        HookResult::NotHandled
    }

    /// Optionally parse a column option at the current cursor.
    fn parse_column_option_hook<'a>(
        _parser: &mut Parser<'a, Self>,
    ) -> HookResult<ColumnOption<Self::Ext>>
    where
        Self: Sized,
    {
        HookResult::NotHandled
    }

    /// Optionally parse a table constraint at the current cursor.
    fn parse_table_constraint_hook<'a>(
        _parser: &mut Parser<'a, Self>,
    ) -> HookResult<TableConstraint<Self::Ext>>
    where
        Self: Sized,
    {
        HookResult::NotHandled
    }

    /// Optionally parse a *data type* at the current cursor, producing a host-owned
    /// [`DataType::Other`] the stock grammar does not spell.
    ///
    /// The type analogue of the whole-node hooks above, consulted at the head of the
    /// parser's (crate-internal) `parse_data_type` — the single entry every type
    /// position funnels through (a `CAST` target, a column type, a `RETURNS`/parameter
    /// type, a composite field, a `MAP` key/value, an array element). So the same hook
    /// serves every type production, and a returned node lands in the appropriate
    /// [`DataType::Other`] arm of the enclosing tree.
    ///
    /// Same 3-state contract as the other hooks: [`Handled`](HookResult::Handled) claims
    /// the production and returns the node, [`NotHandled`](HookResult::NotHandled) leaves
    /// the cursor untouched so the built-in type grammar runs (so a custom type name that
    /// the host declines still falls through to a stock spelling or
    /// [`UserDefined`](DataType::UserDefined) — never *forced* into it), and
    /// [`Err`](HookResult::Err) surfaces a parser-owned diagnostic. A dialect that does
    /// not extend the type grammar returns `NotHandled` (the default) and pays nothing.
    fn parse_data_type_hook<'a>(_parser: &mut Parser<'a, Self>) -> HookResult<DataType<Self::Ext>>
    where
        Self: Sized,
    {
        HookResult::NotHandled
    }

    /// Recognize a custom infix operator at the cursor and report its binding power,
    /// **without consuming any input** — return `None` when no custom
    /// operator starts here.
    ///
    /// This is the typed hook required so an extension operator cannot
    /// ignore precedence the way the prior art's `parse_infix` did. The contract
    /// keeps the parser, not the hook, in charge of the precedence climb: the parser
    /// gates on the reported `left` binding power (a looser operator is left for an
    /// enclosing expression), consumes the operator token, climbs the right operand
    /// at the reported `right` binding power, and only then calls
    /// [`build_infix_operator`](Self::build_infix_operator) with the finished
    /// operands. Because the hook never parses the right operand itself, it has no
    /// `right` binding power to ignore — the mis-bind is structurally impossible,
    /// exactly as for the built-in operators.
    ///
    /// The hook may peek arbitrarily far ([`peek_nth`](Parser::peek_nth)) but must
    /// not advance the cursor; a custom operator is a single token reusing a lexeme
    /// the core grammar leaves free in infix position (`~`, `^`, `&`, `|`, or a bare
    /// word) — an operator *sequence* has no seam here (see the ticket
    /// `extension-operator-follow-ups`). The reported [`BindingPower::assoc`] drives
    /// both render-time grouping and parse-time chaining: a `NonAssoc` custom operator
    /// rejects an unparenthesized chain (`a OP b OP c`) the way `a < b < c` is
    /// rejected, provided the dialect also reports the node's precedence through
    /// [`extension_operand_binding_power`](Self::extension_operand_binding_power) and
    /// encodes the operator with `left < right`.
    fn peek_infix_operator_hook<'a>(
        _parser: &mut Parser<'a, Self>,
    ) -> ParseResult<Option<BindingPower>>
    where
        Self: Sized,
    {
        Ok(None)
    }

    /// Build the extension node for the custom infix operator
    /// [`peek_infix_operator_hook`](Self::peek_infix_operator_hook) recognized, from
    /// the operands the parser climbed.
    ///
    /// `op` is the operator token the parser consumed (its kind/text identifies which
    /// custom operator, for a dialect with more than one); `left` and `right` are the
    /// operands, with `right` already parsed at the operator's own right binding
    /// power. A dialect that recognizes an infix operator must implement this.
    fn build_infix_operator<'a>(
        _parser: &mut Parser<'a, Self>,
        _op: Token,
        _left: Expr<Self::Ext>,
        _right: Expr<Self::Ext>,
    ) -> ParseResult<Expr<Self::Ext>>
    where
        Self: Sized,
    {
        unreachable!(
            "peek_infix_operator_hook recognized a custom infix operator, so the dialect \
             must implement build_infix_operator to construct its node"
        )
    }

    /// Recognize a custom prefix operator at the cursor and report its prefix binding
    /// power, **without consuming any input** — return `None` when no custom prefix
    /// operator starts here.
    ///
    /// The prefix analogue of [`peek_infix_operator_hook`](Self::peek_infix_operator_hook):
    /// the parser consumes the operator token and climbs the operand at the reported
    /// binding power, then calls [`build_prefix_operator`](Self::build_prefix_operator),
    /// so the operand cannot be parsed at the wrong precedence. Distinct from
    /// [`parse_prefix_expr_hook`](Self::parse_prefix_expr_hook), which returns a whole
    /// custom *primary* and parses its own operands; use this when the construct is an
    /// operator whose operand precedence must follow the binding-power table.
    fn peek_prefix_operator_hook<'a>(_parser: &mut Parser<'a, Self>) -> ParseResult<Option<u8>>
    where
        Self: Sized,
    {
        Ok(None)
    }

    /// Build the extension node for the custom prefix operator
    /// [`peek_prefix_operator_hook`](Self::peek_prefix_operator_hook) recognized, from
    /// the operand the parser climbed at the reported binding power. `op` is the
    /// operator token the parser consumed.
    fn build_prefix_operator<'a>(
        _parser: &mut Parser<'a, Self>,
        _op: Token,
        _operand: Expr<Self::Ext>,
    ) -> ParseResult<Expr<Self::Ext>>
    where
        Self: Sized,
    {
        unreachable!(
            "peek_prefix_operator_hook recognized a custom prefix operator, so the dialect \
             must implement build_prefix_operator to construct its node"
        )
    }

    /// Report the binding power of a custom operator node this dialect built, so the
    /// parser can reject a non-associative *chain* (`a OP b OP c` for a `NonAssoc`
    /// custom `OP`) the same way it rejects `a < b < c` for the built-in
    /// comparisons. Return `None` (the default) for a self-delimiting extension node —
    /// an atom, or an operator that is `Left`/`Right`-associative and so never chains
    /// illegally.
    ///
    /// The parser cannot read a binding power off an [`Expr::Other`] node itself: the
    /// [`Extension`] bound has a blanket impl and so cannot
    /// carry an overridable accessor. It therefore asks the dialect — which knows the
    /// concrete `Self::Ext` — to map the node to its precedence, exactly as
    /// [`build_infix_operator`](Self::build_infix_operator) maps operands to a node.
    /// The returned value MUST equal the binding power the operator's
    /// `peek_*_operator_hook` reported (and that the node returns from
    /// `Render::operand_binding_power`), so parse-time chain rejection and render-time
    /// grouping stay one source of truth.
    ///
    /// A `NonAssoc` custom operator must be encoded with `left < right` — like the
    /// built-in comparisons (`40`/`41`) — so a second occurrence of the operator is
    /// left for the enclosing precedence climb, where the chain check runs, rather
    /// than being folded (and silently re-associated) by the inner climb.
    ///
    /// [`Expr::Other`]: crate::ast::Expr::Other
    fn extension_operand_binding_power(_ext: &Self::Ext) -> Option<BindingPower>
    where
        Self: Sized,
    {
        None
    }
}

/// Configuration shared by every SQL parsing result shape.
///
/// The universal combining form for entry points that return a [`Parsed`] tree:
/// same-shape knobs are fields here rather than new `parse_with_x` names. Pass it
/// to [`parse_with`] or [`parse_recovering_with`]. [`ParseConfig::default`]
/// reproduces the behaviour of [`parse`](crate::parse) and [`statements`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseConfig<D = crate::dialect::Ansi> {
    /// Dialect implementation used to interpret the input.
    pub dialect: D,
    /// Maximum recursive-descent nesting depth before a parse fails with a
    /// [`ParseErrorKind::RecursionLimitExceeded`](crate::error::ParseErrorKind::RecursionLimitExceeded)
    /// error instead of recursing further and risking a stack overflow. Defaults
    /// to [`DEFAULT_RECURSION_LIMIT`].
    pub recursion_limit: usize,
    /// Capture out-of-band comment/whitespace trivia alongside the statements, recoverable afterwards through [`Parsed::trivia`]/[`Parsed::trivia_in`]/[`Parsed::trivia_before`]. Defaults to `false`, the zero-allocation-when-off path [`parse_with`] uses.
    ///
    /// Honoured by [`parse_with`] and [`parse_recovering_with`], whose results both embed an owned [`Parsed`] root to hang the index on. The streaming [`statements`]/[`statements_with`] iterator does **not** read this field: it yields bare statements one at a time with no owned root to carry a trivia index, and staying trivia-free is precisely what keeps it bounded-memory rather than a `Parsed`-shaped alternative — so trivia there would need a new accessor on [`Statements`] (a different shape), not a flag this struct can honour. This is a deliberate, documented boundary rather than a half-plumbed option.
    pub capture_trivia: bool,
    /// Classify a fractional or scientific numeric literal as
    /// [`LiteralKind::Decimal`](crate::ast::LiteralKind::Decimal) rather than
    /// [`LiteralKind::Float`](crate::ast::LiteralKind::Float). Defaults to `false`, so the
    /// default parse paths ([`parse_with`], [`statements`]) never produce the `Decimal`
    /// tag and their numeric-literal classification is byte-for-byte the historical one.
    ///
    /// This is a *consumer request*, not a dialect grammar rule, which is why it rides
    /// [`ParseConfig`] rather than the [`Dialect`]: the same
    /// SQL text is grammatically identical to the parser either way — the source spelling
    /// round-trips unchanged and only the AST classification tag differs — so the choice
    /// belongs to the caller who knows whether their downstream planner distinguishes
    /// exact `DECIMAL`/`NUMERIC` from binary floating point (the sqlparser-rs
    /// `parse_float_as_decimal` / BigQuery-planner distinction). Honoured by every
    /// `Parsed`-returning entry point and by the streaming
    /// [`statements_with`] iterator (unlike [`capture_trivia`](Self::capture_trivia),
    /// it is classification metadata on each literal, carrying no owned-root cost, so the
    /// bounded-memory iterator honours it too).
    pub parse_float_as_decimal: bool,
}

impl Default for ParseConfig<crate::dialect::Ansi> {
    fn default() -> Self {
        Self::new(crate::dialect::Ansi)
    }
}

impl<D> ParseConfig<D> {
    /// A parse configuration for `dialect` with every tuning knob at its default.
    pub const fn new(dialect: D) -> Self {
        Self {
            dialect,
            recursion_limit: DEFAULT_RECURSION_LIMIT,
            capture_trivia: false,
            parse_float_as_decimal: false,
        }
    }

    /// This configuration retargeted to `dialect` with every tuning knob preserved.
    #[must_use]
    pub fn dialect<E>(self, dialect: E) -> ParseConfig<E> {
        ParseConfig {
            dialect,
            recursion_limit: self.recursion_limit,
            capture_trivia: self.capture_trivia,
            parse_float_as_decimal: self.parse_float_as_decimal,
        }
    }

    /// This configuration with the recursion-depth limit set to `recursion_limit`.
    #[must_use]
    pub const fn recursion_limit(mut self, recursion_limit: usize) -> Self {
        self.recursion_limit = recursion_limit;
        self
    }

    /// This configuration with trivia capture toggled to `capture_trivia`.
    #[must_use]
    pub const fn capture_trivia(mut self, capture_trivia: bool) -> Self {
        self.capture_trivia = capture_trivia;
        self
    }

    /// This configuration with float-as-decimal classification toggled to
    /// `parse_float_as_decimal`.
    #[must_use]
    pub const fn parse_float_as_decimal(mut self, parse_float_as_decimal: bool) -> Self {
        self.parse_float_as_decimal = parse_float_as_decimal;
        self
    }
}

/// Parse `src` into an owned [`Parsed`] tree using `config`.
///
/// Every same-shape parse option is carried here rather than growing a family of
/// option-specific free functions. In particular, `capture_trivia` attaches the
/// out-of-band trivia index to the returned root.
///
/// # Errors
///
/// Returns the first lexical or grammar error. Inputs nested beyond
/// `config.recursion_limit` return
/// [`ParseErrorKind::RecursionLimitExceeded`](crate::error::ParseErrorKind::RecursionLimitExceeded).
pub fn parse_with<D: Dialect>(
    src: &str,
    config: ParseConfig<D>,
) -> ParseResult<Parsed<Arc<str>, D::Ext>> {
    if config.capture_trivia {
        collect_parsed_with_trivia::<Arc<str>, D>(
            src,
            config.dialect,
            config.recursion_limit,
            config.parse_float_as_decimal,
        )
    } else {
        collect_parsed::<Arc<str>, D>(
            src,
            config.dialect,
            config.recursion_limit,
            config.parse_float_as_decimal,
        )
    }
}

/// Parse `src` under ANSI into an `Rc`-rooted [`Parsed`] tree.
///
/// The single-thread ownership tier: an `Rc<str>` source is the
/// cheapest refcount when the parsed tree never crosses a thread boundary,
/// trading the `Send + Sync` of the [`parse_with`] `Arc` default for a
/// non-atomic refcount. Statements and resolver are otherwise identical.
///
/// # Errors
///
/// Identical to [`parse_with`].
pub fn parse_rc(src: &str) -> ParseResult<Parsed<Rc<str>>> {
    parse_rc_with(src, ParseConfig::default())
}

/// Parse `src` into an `Rc`-rooted [`Parsed`] tree using `config`.
///
/// # Errors
///
/// Identical to [`parse_with`].
pub fn parse_rc_with<D: Dialect>(
    src: &str,
    config: ParseConfig<D>,
) -> ParseResult<Parsed<Rc<str>, D::Ext>> {
    if config.capture_trivia {
        collect_parsed_with_trivia::<Rc<str>, D>(
            src,
            config.dialect,
            config.recursion_limit,
            config.parse_float_as_decimal,
        )
    } else {
        collect_parsed::<Rc<str>, D>(
            src,
            config.dialect,
            config.recursion_limit,
            config.parse_float_as_decimal,
        )
    }
}

/// Parse ANSI `src` as a lazy [`Statements`] iterator.
///
/// The bounded-memory streaming counterpart to [`parse_with`]: the entry a
/// caller streaming untrusted SQL reaches for to bound recursion depth.
///
/// Reads `config.recursion_limit` and `config.parse_float_as_decimal`. It does not
/// read `config.capture_trivia`: `Statements` yields bare statements and has no
/// owned root on which to retain a trivia index.
///
/// # Errors
///
/// As [`statements`]; per-statement recursion-limit errors surface as `Err` items
/// from the iterator, like other grammar errors.
pub fn statements(src: &str) -> ParseResult<Statements<'_, crate::dialect::Ansi>> {
    streaming::statements(src, crate::dialect::Ansi)
}

/// Parse `src` as a lazy [`Statements`] iterator using `config`.
pub fn statements_with<D: Dialect>(
    src: &str,
    config: ParseConfig<D>,
) -> ParseResult<Statements<'_, D>> {
    streaming::statements_with_limit(
        src,
        config.dialect,
        config.recursion_limit,
        config.parse_float_as_decimal,
    )
}

/// Shared collecting body behind [`parse_with`] / [`parse_rc_with`], generic over
/// the source store `S` (the ownership tiers).
///
/// Streams statements over the lazy token buffer, then freezes the interner into
/// the resolver shipped on the root. The `for<'a> From<&'a str>` bound is what
/// lets one body materialize either refcounted store (`Arc<str>` / `Rc<str>`)
/// from the borrowed `src`. That higher-ranked bound also reinforces the
/// owned-root contract: a borrowed store (`&'a str`) cannot satisfy `From<&'b str>`
/// for *every* `'b`, so this body is structurally inconstructible with a store that
/// borrows `src` — the public path can never yield a lifetime-bound `Parsed`.
fn collect_parsed<S, D>(
    src: &str,
    dialect: D,
    recursion_limit: usize,
    parse_float_as_decimal: bool,
) -> ParseResult<Parsed<S, D::Ext>>
where
    S: SourceStore + for<'a> From<&'a str>,
    D: Dialect,
{
    // Capture the parse's string-literal syntax before the dialect is moved into the
    // iterator, so the root can materialise string values dialect-correctly (ADR-0006).
    let string_literals = dialect.features().string_literals;
    let mut iter =
        streaming::statements_with_limit(src, dialect, recursion_limit, parse_float_as_decimal)?;
    let mut collected = Vec::new();
    for statement in iter.by_ref() {
        collected.push(statement?);
    }
    let resolver = iter.finish();
    Ok(Parsed::new(
        S::from(src),
        resolver,
        collected,
        string_literals,
    ))
}

/// [`collect_parsed`] with trivia capture enabled, backing [`parse_with`] when
/// [`ParseConfig::capture_trivia`] is set.
///
/// Drives the trivia-recording [`Parser`] directly rather than through the public
/// [`statements`] iterator: the iterator is the bounded-memory path and stays
/// trivia-free, whereas here the cursor accumulates the whole source's trivia for
/// the root. Trivia is taken by `&mut` before [`finish`](Parser::finish) consumes
/// the parser, so the root receives both the resolver and the trivia. Fail-fast is
/// unchanged — the first statement error returns early and no root is built.
fn collect_parsed_with_trivia<S, D>(
    src: &str,
    dialect: D,
    recursion_limit: usize,
    parse_float_as_decimal: bool,
) -> ParseResult<Parsed<S, D::Ext>>
where
    S: SourceStore + for<'a> From<&'a str>,
    D: Dialect,
{
    // Capture the parse's string-literal syntax before the dialect is moved into the
    // parser, so the root can materialise string values dialect-correctly (ADR-0006).
    let string_literals = dialect.features().string_literals;
    // Clause-mark capture rides the same opt-in as trivia (the formatter consumer
    // wants comments *and* the clause-keyword offsets that anchor them), so this one
    // path enables both — the default `collect_parsed` records neither.
    let mut parser = Parser::streaming_with_trivia(src, dialect)?
        .recursion_limit(recursion_limit)
        .parse_float_as_decimal(parse_float_as_decimal)
        .with_clause_mark_capture(true);
    let mut collected = Vec::new();
    while let Some(statement) = parser.parse_next_statement()? {
        collected.push(statement);
    }
    let trivia = parser.take_trivia();
    let clause_marks = parser.take_clause_marks();
    let resolver = parser.finish();
    Ok(
        Parsed::new(S::from(src), resolver, collected, string_literals)
            .with_trivia(trivia)
            .with_clause_marks(clause_marks),
    )
}

/// Reconcile a tokenizer [`LexError`] into a [`ParseError`].
///
/// The token-stream methods ([`Parser::peek`](Parser::peek) etc.) return the
/// cursor's narrow [`LexError`] — a lexical fault is genuinely their only failure
/// mode, and a 12-byte error keeps the per-peek `Result` 16 bytes rather than the
/// 56 bytes `ParseError` would impose on the hottest path in the parser. This
/// `From` widens it to the unified [`ParseError`] only at the `?` boundary, where a
/// grammar method's `ParseResult` demands it — so the widening is paid solely on the
/// rare error path, never on the success path (perf: peek hot path).
///
/// The precise byte span is preserved, and the machine-matchable
/// [`LexErrorKind`](crate::tokenizer::LexErrorKind) rides across the widening on
/// [`ParseErrorKind::Lexical`](crate::error::ParseErrorKind::Lexical) so a
/// lexical fault stays distinguishable from a grammar mismatch (rather than
/// collapsing to `Syntax`). The kind's message becomes the offending `found`,
/// against the generic expectation a well-formed token would have met; the shape
/// is approximate by nature — a lexical fault is not really an "expected X, found
/// Y" — but it keeps the span exact and the message faithful, which is what
/// downstream diagnostics need.
impl From<LexError> for ParseError {
    fn from(error: LexError) -> Self {
        ParseError::lexical(error.span, error.kind)
    }
}

/// A minimal compile-time dialect for parser tests: stock AST, ANSI data.
///
/// Shared by the engine tests below and the Pratt expression tests in
/// [`expr`](super::expr), which reach it as `crate::parser::TestDialect` — a child
/// test module may name a private item of its ancestor `parser` module.
#[cfg(test)]
struct TestDialect;

#[cfg(test)]
impl Dialect for TestDialect {
    type Ext = crate::ast::NoExt;

    fn features(&self) -> &FeatureSet {
        // `&` of an associated const is promoted to `'static`, so a const dialect
        // needs no stored field and its reads const-fold.
        &FeatureSet::ANSI
    }
}

/// A test dialect that is *only* its feature data: stock AST (`NoExt`), no parse or
/// render hooks. Every data-only test dialect is a `const` value of this one type,
/// so the ~15k-LOC parser and renderer engines monomorphize once for the whole
/// family instead of once per feature preset. Presets are `const FeatureSet`s, so
/// `&PRESET` promotes to `'static` and each dialect is a plain const with no
/// leaking. Distinct types remain only where a test needs a non-`NoExt` `Ext` or
/// overrides a parse hook (the `Custom`/`Dyn`/`Op`/`Hook` families).
///
/// This is a compile-time-DX collapse of TEST code only: the product path still
/// dispatches on ZST dialects and const-folds its `FeatureSet` reads (ADR-0011),
/// which this deliberately does not touch.
#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) struct FeatureDialect {
    pub(crate) features: &'static FeatureSet,
}

#[cfg(test)]
impl Dialect for FeatureDialect {
    type Ext = crate::ast::NoExt;

    fn features(&self) -> &FeatureSet {
        self.features
    }
}

#[cfg(test)]
impl crate::render::RenderDialect for FeatureDialect {
    fn render_features(&self) -> FeatureSet {
        self.features.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        ColumnOption, CreateTableBody, Expr, Keyword, Literal, LiteralKind, NoExt, QuoteStyle,
        Resolver as _, SelectItem, SetExpr, Span, Spanned, Statement, TableConstraint,
        TableElement, TableFactor,
    };
    use crate::error::Found;
    use crate::tokenizer::{TokenKind, tokenize};

    #[test]
    fn parses_minimal_select_projection_end_to_end() {
        let src = "SELECT 1, foo, *";
        let parsed = parse_with(src, crate::ParseConfig::new(TestDialect))
            .expect("a well-formed SELECT parses");

        assert_eq!(parsed.statements().len(), 1, "exactly one statement");
        let Statement::Query { query, meta } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        // The statement/query span covers the whole input.
        assert_eq!(meta.span, Span::new(0, src.len() as u32));
        assert_eq!(query.meta.span, Span::new(0, src.len() as u32));

        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        assert_eq!(query.body.span(), Span::new(0, src.len() as u32));
        assert_eq!(select.meta.span, Span::new(0, src.len() as u32));
        // Non-projection clauses are empty in the minimal slice.
        assert!(select.from.is_empty());
        assert!(select.selection.is_none());

        // Projection: [Literal(1), Column(foo), Wildcard].
        assert_eq!(select.projection.len(), 3);
        match &select.projection[0] {
            SelectItem::Expr {
                expr: Expr::Literal { literal: lit, .. },
                alias: None,
                ..
            } => assert_eq!(lit.kind, LiteralKind::Integer),
            other => panic!("item 0 should be an integer literal, got {other:?}"),
        }
        match &select.projection[1] {
            SelectItem::Expr {
                expr: Expr::Column { name, .. },
                alias: None,
                ..
            } => {
                assert_eq!(name.0.len(), 1, "an unqualified column is one part");
                // The resolver round-trips the interned word back to "foo".
                assert_eq!(parsed.resolver().resolve(name.0[0].sym), "foo");
            }
            other => panic!("item 1 should be the column `foo`, got {other:?}"),
        }
        let SelectItem::Wildcard {
            options: None,
            meta,
            ..
        } = &select.projection[2]
        else {
            panic!("item 2 should be the wildcard");
        };
        assert_eq!(meta.span, Span::new(15, 16));
    }

    #[test]
    fn quoted_canonical_keyword_shares_the_keyword_symbol() {
        // A quoted identifier is lexed as `QuotedIdent` with no keyword
        // classification, so it interns through the full keyword-checking
        // `intern_text` path — the one the settled-kind fast paths deliberately leave
        // untouched. Its text must therefore still resolve to the keyword's fixed
        // slot: a quoted `"select"` and a bare `select` keyword-token share one
        // `Symbol`, keeping the same-text-same-symbol identity invariant intact.
        let parsed =
            parse_with(r#"SELECT "select""#, crate::ParseConfig::new(TestDialect)).expect("valid");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let SelectItem::Expr {
            expr: Expr::Column { name, .. },
            ..
        } = &select.projection[0]
        else {
            panic!("expected a single quoted-identifier column");
        };
        assert_eq!(name.0.len(), 1, "an unqualified column is one part");
        let ident = &name.0[0];
        // It really went through the quoted path...
        assert_eq!(ident.quote, QuoteStyle::Double);
        // ...yet lands on the canonical keyword's fixed low slot.
        assert_eq!(ident.sym, Keyword::Select.symbol());
        // ...and still round-trips its exact source text.
        assert_eq!(parsed.resolver().resolve(ident.sym), "select");
    }

    #[test]
    fn float_and_integer_literals_are_classified_by_spelling() {
        let parsed = parse_with("SELECT 42, 3.14, 1e9", crate::ParseConfig::new(TestDialect))
            .expect("valid");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT");
        };

        let kinds: Vec<&LiteralKind> = select
            .projection
            .iter()
            .map(|item| match item {
                SelectItem::Expr {
                    expr: Expr::Literal { literal: lit, .. },
                    ..
                } => &lit.kind,
                other => panic!("expected a literal, got {other:?}"),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                &LiteralKind::Integer,
                &LiteralKind::Float,
                &LiteralKind::Float,
            ],
        );
    }

    #[test]
    fn leading_non_select_word_is_a_parse_error_at_the_word() {
        let err = parse_with("from t", crate::ParseConfig::new(TestDialect))
            .expect_err("FROM is not a supported statement");
        // The error points at the offending leading word `from`.
        assert_eq!(err.span, Span::new(0, 4));
    }

    #[test]
    fn trailing_comma_in_projection_errors_at_end_of_input() {
        let err = parse_with("SELECT 1,", crate::ParseConfig::new(TestDialect))
            .expect_err("a dangling comma has no item");
        // The missing item is at end of input: an empty span past the last token.
        assert_eq!(err.span, Span::new(9, 9));
        assert_eq!(err.found, Found::EndOfInput);
    }

    #[test]
    fn non_item_token_in_projection_errors_at_that_token() {
        // A bare comma where an item is required reports against the comma, not EOF.
        let err = parse_with("SELECT ,", crate::ParseConfig::new(TestDialect))
            .expect_err("`,` is not a select item");
        assert_eq!(err.span, Span::new(7, 8));
        assert_eq!(err.found, Found::from(","));
    }

    #[test]
    fn lexical_errors_surface_through_the_parse_channel_with_their_span() {
        // Unterminated string: the tokenizer fails, mapped into a ParseError that
        // keeps the literal's byte span.
        let err = parse_with("SELECT 'oops", crate::ParseConfig::new(TestDialect))
            .expect_err("the string never closes");
        assert_eq!(err.span, Span::new(7, 12));
    }

    #[test]
    fn checkpoint_then_rewind_restores_the_token_cursor() {
        // Drive the speculation seam directly: `m1-pratt-expr` backtracks on it.
        // `d`/`b` are not keywords in the full inventory, so they lex as words.
        let src = "d , b";
        let tokens = tokenize(src).expect("clean");
        let mut parser = Parser::new(src, &tokens, TestDialect);

        let start = parser.checkpoint();
        assert!(parser.advance().expect("first token").is_some());
        assert!(parser.advance().expect("second token").is_some());
        parser.rewind(start);

        // Back at the first token; spans and ids are unaffected by backtracking.
        assert_eq!(
            parser.peek().expect("peek").map(|token| token.kind),
            Some(TokenKind::Word)
        );
        assert_eq!(parser.current_span().expect("span"), Span::new(0, 1));
    }

    #[test]
    fn empty_input_parses_to_zero_statements() {
        let parsed = parse_with("   ; ;  ", crate::ParseConfig::new(TestDialect))
            .expect("only separators and trivia");
        assert!(parsed.statements().is_empty());
        assert_eq!(parsed.source(), "   ; ;  ");
    }

    #[derive(Clone, Copy)]
    struct HookDialect;

    impl Dialect for HookDialect {
        type Ext = NoExt;

        fn features(&self) -> &FeatureSet {
            &FeatureSet::ANSI
        }

        fn parse_statement_hook<'a>(parser: &mut Parser<'a, Self>) -> HookResult<Statement<NoExt>> {
            match parser.peek() {
                Ok(Some(token)) if matches!(token.kind, TokenKind::Keyword(Keyword::Create)) => {
                    HookResult::Err(parser.unexpected("a hook-provided statement"))
                }
                Ok(Some(_)) => HookResult::NotHandled,
                Ok(None) => HookResult::NotHandled,
                Err(error) => HookResult::Err(error.into()),
            }
        }

        fn parse_prefix_expr_hook<'a>(parser: &mut Parser<'a, Self>) -> HookResult<Expr<NoExt>> {
            let token = match parser.peek() {
                Ok(Some(token)) => token,
                Ok(None) => return HookResult::NotHandled,
                Err(error) => return HookResult::Err(error.into()),
            };
            let kind = match token.kind {
                TokenKind::Keyword(Keyword::True) => LiteralKind::Boolean(true),
                TokenKind::Keyword(Keyword::False) => LiteralKind::Boolean(false),
                _ => return HookResult::NotHandled,
            };
            if let Err(error) = parser.advance() {
                return HookResult::Err(error.into());
            }
            let meta = parser.make_meta(token.span);
            HookResult::Handled(Expr::Literal {
                literal: Literal { kind, meta },
                meta,
            })
        }
    }

    #[test]
    fn typed_prefix_hook_can_handle_a_production() {
        let parsed = parse_with("SELECT TRUE", crate::ParseConfig::new(HookDialect))
            .expect("hook handles TRUE");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT");
        };
        let SelectItem::Expr {
            expr: Expr::Literal { literal, .. },
            ..
        } = &select.projection[0]
        else {
            panic!("expected the hook-provided boolean literal");
        };

        assert_eq!(literal.kind, LiteralKind::Boolean(true));
    }

    #[test]
    fn typed_statement_hook_can_decline_or_error() {
        parse_with("SELECT 1", crate::ParseConfig::new(HookDialect))
            .expect("NotHandled falls back to built-in SELECT");

        let err = parse_with("CREATE", crate::ParseConfig::new(HookDialect))
            .expect_err("hook returns its parse error");
        assert_eq!(err.expected.as_str(), "a hook-provided statement");
        assert_eq!(err.span, Span::new(0, 6));
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum CustomKind {
        Statement,
        Expr,
        TableFactor,
        ColumnOption,
        TableConstraint,
        DataType,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct CustomExt {
        kind: CustomKind,
        span: Span,
    }

    impl Spanned for CustomExt {
        fn span(&self) -> Span {
            self.span
        }
    }

    #[derive(Clone, Copy)]
    struct CustomDialect;

    #[derive(Clone, Copy)]
    struct CustomHookSpec {
        good: &'static str,
        bad: &'static str,
        kind: CustomKind,
        expected: &'static str,
    }

    fn custom_hook<T>(
        parser: &mut Parser<'_, CustomDialect>,
        spec: CustomHookSpec,
        wrap: impl FnOnce(CustomExt, crate::ast::Meta) -> T,
    ) -> HookResult<T> {
        let token = match parser.peek() {
            Ok(Some(token)) => token,
            Ok(None) => return HookResult::NotHandled,
            Err(error) => return HookResult::Err(error.into()),
        };
        if !matches!(token.kind, TokenKind::Word | TokenKind::Keyword(_)) {
            return HookResult::NotHandled;
        }

        let text = parser.span_text(token.span);
        if text.eq_ignore_ascii_case(spec.bad) {
            return HookResult::Err(parser.unexpected(spec.expected));
        }
        if !text.eq_ignore_ascii_case(spec.good) {
            return HookResult::NotHandled;
        }

        if let Err(error) = parser.advance() {
            return HookResult::Err(error.into());
        }
        let meta = parser.make_meta(token.span);
        HookResult::Handled(wrap(
            CustomExt {
                kind: spec.kind,
                span: token.span,
            },
            meta,
        ))
    }

    impl Dialect for CustomDialect {
        type Ext = CustomExt;

        fn features(&self) -> &FeatureSet {
            &FeatureSet::ANSI
        }

        fn parse_statement_hook<'a>(
            parser: &mut Parser<'a, Self>,
        ) -> HookResult<Statement<Self::Ext>> {
            custom_hook(
                parser,
                CustomHookSpec {
                    good: "custom_statement",
                    bad: "bad_statement",
                    kind: CustomKind::Statement,
                    expected: "a hook-provided statement",
                },
                |ext, meta| Statement::Other { ext, meta },
            )
        }

        fn parse_prefix_expr_hook<'a>(
            parser: &mut Parser<'a, Self>,
        ) -> HookResult<Expr<Self::Ext>> {
            custom_hook(
                parser,
                CustomHookSpec {
                    good: "custom_expr",
                    bad: "bad_expr",
                    kind: CustomKind::Expr,
                    expected: "a hook-provided expression",
                },
                |ext, meta| Expr::Other { ext, meta },
            )
        }

        fn parse_table_factor_hook<'a>(
            parser: &mut Parser<'a, Self>,
        ) -> HookResult<TableFactor<Self::Ext>> {
            custom_hook(
                parser,
                CustomHookSpec {
                    good: "custom_table",
                    bad: "bad_table",
                    kind: CustomKind::TableFactor,
                    expected: "a hook-provided table factor",
                },
                |ext, meta| TableFactor::Other { ext, meta },
            )
        }

        fn parse_column_option_hook<'a>(
            parser: &mut Parser<'a, Self>,
        ) -> HookResult<ColumnOption<Self::Ext>> {
            custom_hook(
                parser,
                CustomHookSpec {
                    good: "custom_column_option",
                    bad: "bad_column_option",
                    kind: CustomKind::ColumnOption,
                    expected: "a hook-provided column option",
                },
                |ext, meta| ColumnOption::Other { ext, meta },
            )
        }

        fn parse_table_constraint_hook<'a>(
            parser: &mut Parser<'a, Self>,
        ) -> HookResult<TableConstraint<Self::Ext>> {
            custom_hook(
                parser,
                CustomHookSpec {
                    good: "custom_table_constraint",
                    bad: "bad_table_constraint",
                    kind: CustomKind::TableConstraint,
                    expected: "a hook-provided table constraint",
                },
                |ext, meta| TableConstraint::Other { ext, meta },
            )
        }

        fn parse_data_type_hook<'a>(
            parser: &mut Parser<'a, Self>,
        ) -> HookResult<DataType<Self::Ext>> {
            custom_hook(
                parser,
                CustomHookSpec {
                    good: "custom_type",
                    bad: "bad_type",
                    kind: CustomKind::DataType,
                    expected: "a hook-provided data type",
                },
                |ext, meta| DataType::Other { ext, meta },
            )
        }
    }

    #[test]
    fn custom_statement_hook_returns_custom_parsed_root() {
        let parsed: Parsed<_, CustomExt> =
            parse_with("custom_statement", crate::ParseConfig::new(CustomDialect))
                .expect("custom statement parses");

        assert_eq!(parsed.source(), "custom_statement");
        let [Statement::Other { ext, .. }] = parsed.statements() else {
            panic!("expected one custom statement");
        };
        assert_eq!(ext.kind, CustomKind::Statement);
        assert_eq!(ext.span(), Span::new(0, 16));
    }

    #[test]
    fn custom_expr_hook_keeps_source_and_resolver_on_parsed_root() {
        let parsed: Parsed<_, CustomExt> = parse_with(
            "SELECT custom_expr AS marker",
            crate::ParseConfig::new(CustomDialect),
        )
        .expect("custom expression parses");
        let [Statement::Query { query, .. }] = parsed.statements() else {
            panic!("expected one query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let SelectItem::Expr {
            expr: Expr::Other { ext, .. },
            alias: Some(alias),
            ..
        } = &select.projection[0]
        else {
            panic!("expected custom expression with alias");
        };

        assert_eq!(parsed.source(), "SELECT custom_expr AS marker");
        assert_eq!(ext.kind, CustomKind::Expr);
        assert_eq!(ext.span(), Span::new(7, 18));
        assert_eq!(parsed.resolver().resolve(alias.sym), "marker");
    }

    #[test]
    fn custom_table_factor_hook_returns_other_table_factor() {
        let parsed: Parsed<_, CustomExt> = parse_with(
            "SELECT * FROM custom_table",
            crate::ParseConfig::new(CustomDialect),
        )
        .expect("custom table factor parses");
        let [Statement::Query { query, .. }] = parsed.statements() else {
            panic!("expected one query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let TableFactor::Other { ext, .. } = &select.from[0].relation else {
            panic!("expected custom table factor");
        };

        assert_eq!(ext.kind, CustomKind::TableFactor);
        assert_eq!(ext.span(), Span::new(14, 26));
    }

    #[test]
    fn custom_data_type_hook_accepts_a_custom_type_and_declines_to_stock() {
        // Accept: the hook claims `custom_type` in a CAST target and lands a
        // `DataType::Other`, so a host-owned type need not masquerade as `UserDefined`.
        let parsed: Parsed<_, CustomExt> = parse_with(
            "SELECT CAST(x AS custom_type)",
            crate::ParseConfig::new(CustomDialect),
        )
        .expect("custom data type parses");
        let [Statement::Query { query, .. }] = parsed.statements() else {
            panic!("expected one query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let SelectItem::Expr {
            expr: Expr::Cast { data_type, .. },
            ..
        } = &select.projection[0]
        else {
            panic!("expected a CAST projection");
        };
        let DataType::Other { ext, .. } = data_type.as_ref() else {
            panic!("expected the hook-provided custom data type");
        };
        assert_eq!(ext.kind, CustomKind::DataType);
        assert_eq!(ext.span(), Span::new(17, 28));

        // Decline: a stock type name in the same position falls through to the built-in
        // grammar (the hook returned `NotHandled` without consuming input).
        let parsed: Parsed<_, CustomExt> = parse_with(
            "SELECT CAST(x AS INT)",
            crate::ParseConfig::new(CustomDialect),
        )
        .expect("stock data type parses");
        let [Statement::Query { query, .. }] = parsed.statements() else {
            panic!("expected one query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let SelectItem::Expr {
            expr: Expr::Cast { data_type, .. },
            ..
        } = &select.projection[0]
        else {
            panic!("expected a CAST projection");
        };
        assert!(matches!(data_type.as_ref(), DataType::Integer { .. }));
    }

    #[test]
    fn custom_ddl_hooks_return_other_column_options_and_table_constraints() {
        let parsed: Parsed<_, CustomExt> = parse_with(
            "CREATE TABLE t (id INT custom_column_option, \
             custom_table_constraint, \
             CONSTRAINT named_custom custom_table_constraint)",
            crate::ParseConfig::new(CustomDialect),
        )
        .expect("custom DDL extension nodes parse");
        let [Statement::CreateTable { create, .. }] = parsed.statements() else {
            panic!("expected one CREATE TABLE statement");
        };
        let CreateTableBody::Definition { elements, .. } = &create.body else {
            panic!("expected a table definition");
        };
        assert_eq!(elements.len(), 3);

        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a custom column option on the first element");
        };
        let ColumnOption::Other { ext: option, .. } = &column.constraints[0].option else {
            panic!("expected a custom column option");
        };
        assert_eq!(option.kind, CustomKind::ColumnOption);
        assert_eq!(option.span(), Span::new(23, 43));

        let TableElement::Constraint {
            constraint: unnamed,
            ..
        } = &elements[1]
        else {
            panic!("expected an unnamed custom table constraint");
        };
        assert!(unnamed.name.is_none());
        let TableConstraint::Other {
            ext: unnamed_constraint,
            ..
        } = &unnamed.constraint
        else {
            panic!("expected custom unnamed table constraint");
        };
        assert_eq!(unnamed_constraint.kind, CustomKind::TableConstraint);

        let TableElement::Constraint {
            constraint: named, ..
        } = &elements[2]
        else {
            panic!("expected a named custom table constraint");
        };
        assert_eq!(
            parsed
                .resolver()
                .resolve(named.name.as_ref().expect("named constraint").sym),
            "named_custom",
        );
        let TableConstraint::Other {
            ext: named_constraint,
            ..
        } = &named.constraint
        else {
            panic!("expected custom named table constraint");
        };
        assert_eq!(named_constraint.kind, CustomKind::TableConstraint);
    }

    #[test]
    fn custom_hooks_decline_to_stock_grammar() {
        let parsed: Parsed<_, CustomExt> =
            parse_with("SELECT a FROM t", crate::ParseConfig::new(CustomDialect))
                .expect("stock SELECT still parses");
        let [Statement::Query { query, .. }] = parsed.statements() else {
            panic!("expected one query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        assert!(matches!(
            &select.projection[0],
            SelectItem::Expr {
                expr: Expr::Column { .. },
                alias: None,
                ..
            },
        ));
        assert!(matches!(
            &select.from[0].relation,
            TableFactor::Table { .. },
        ));

        let parsed: Parsed<_, CustomExt> = parse_with(
            "CREATE TABLE t (id INT NOT NULL, PRIMARY KEY (id))",
            crate::ParseConfig::new(CustomDialect),
        )
        .expect("stock CREATE TABLE still parses");
        let [Statement::CreateTable { create, .. }] = parsed.statements() else {
            panic!("expected one CREATE TABLE statement");
        };
        let CreateTableBody::Definition { elements, .. } = &create.body else {
            panic!("expected a table definition");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a stock column");
        };
        assert!(matches!(
            &column.constraints[0].option,
            ColumnOption::NotNull { .. }
        ));
        let TableElement::Constraint { constraint, .. } = &elements[1] else {
            panic!("expected a stock table constraint");
        };
        assert!(matches!(
            &constraint.constraint,
            TableConstraint::PrimaryKey { .. },
        ));
    }

    #[test]
    fn custom_hooks_can_error_at_every_open_family() {
        for (sql, expected, span) in [
            (
                "bad_statement",
                "a hook-provided statement",
                Span::new(0, 13),
            ),
            (
                "SELECT bad_expr",
                "a hook-provided expression",
                Span::new(7, 15),
            ),
            (
                "SELECT * FROM bad_table",
                "a hook-provided table factor",
                Span::new(14, 23),
            ),
            (
                "CREATE TABLE t (id INT bad_column_option)",
                "a hook-provided column option",
                Span::new(23, 40),
            ),
            (
                "CREATE TABLE t (bad_table_constraint)",
                "a hook-provided table constraint",
                Span::new(16, 36),
            ),
            (
                "SELECT CAST(x AS bad_type)",
                "a hook-provided data type",
                Span::new(17, 25),
            ),
        ] {
            let err = parse_with(sql, crate::ParseConfig::new(CustomDialect))
                .expect_err("hook returns its parse error");
            assert_eq!(err.expected.as_str(), expected, "{sql}");
            assert_eq!(err.span, span, "{sql}");
        }
    }
}
