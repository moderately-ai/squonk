// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Tier-2 fallible rendering for dialect targets.
//!
//! The AST crate owns Tier-1 canonical rendering: it is dependency-free and
//! infallible for the supported neutral SQL surface. This module adds the parser
//! crate's dialect-target layer. A target first validates whether it can express
//! a statement, then delegates spelling to Tier-1 for constructs it accepts.
//!
//! Validation is data-driven: the target's [`FeatureSet`] gates which
//! canonical constructs it can spell, so a single walk rejects, for example, a
//! PostgreSQL `LATERAL` / table-function / `TABLESAMPLE` / `ONLY` source when the
//! ANSI target cannot express it, or a `$n` parameter for a target without
//! positional placeholders — loudly, rather than emitting invalid SQL.

use std::fmt::{self, Write as _};

pub use crate::ast::render::{
    FragmentRender, RenderConfig, RenderError, RenderErrorKind, RenderMode, RenderResult,
    RenderSpelling,
};

use crate::ast::dialect::FeatureSet;
use crate::ast::generated::visit::{
    Visit, walk_data_type, walk_expr, walk_join_constraint, walk_on_conflict, walk_query,
    walk_returning, walk_statement, walk_table_factor, walk_upsert,
};
use crate::ast::render::{Render, RenderCtx, RenderExt};
use crate::ast::{
    DataType, DerivedSpelling, Expr, Extension, JoinConstraint, NoExt, NodeId, OnConflict,
    ParameterKind, ParameterSigil, Query, RelationInheritance, Resolver, Returning,
    SessionVariableKind, SourceStore, Span, Statement, TableFactor, Upsert,
};
use crate::error::ParseError;
use crate::parser::{Dialect, Parsed};

/// Dialect-target validation hook for fallible rendering.
///
/// The default [`validate_statement`](RenderDialect::validate_statement) is
/// data-driven: it reads the target's own
/// [`render_features`](RenderDialect::render_features) and rejects any construct
/// the canonical AST can hold but that [`FeatureSet`] does not accept — e.g. a
/// PostgreSQL `LATERAL` / table-function / `TABLESAMPLE` / `ONLY` source rendered
/// to the ANSI target, or a `$n` parameter rendered to a target without
/// positional placeholders. So the stock [`Ansi`](crate::dialect::Ansi) and
/// `Postgres` targets need only supply their feature data; a custom target may
/// still override this hook for rules its `FeatureSet` does not capture.
pub trait RenderDialect {
    /// The target feature set this dialect renders for.
    fn render_features(&self) -> FeatureSet {
        FeatureSet::ANSI
    }

    /// Validate one statement before Tier-1 rendering spells it.
    ///
    /// Rejects the first construct the target [`FeatureSet`] cannot express
    /// (Tier-2: loudly reject, never mis-render), and accepts everything
    /// the target's feature data allows.
    fn validate_statement(&self, statement: &Statement<NoExt>) -> RenderResult<()> {
        validate_target_support(&self.render_features(), statement)
    }
}

// The stock `RenderDialect` impls live with their dialect's other impls in
// `crate::dialect::{ansi, postgres}` (each gated with its feature), so this module
// owns only the dialect-agnostic trait and validation machinery.

/// Reject the first construct `statement` holds that a target with `features`
/// cannot spell.
///
/// This is the data-driven core of the default [`RenderDialect::validate_statement`]
/// (the [`FeatureSet`] gates acceptance, not tree shape). It walks the
/// canonical AST with the generated [`Visit`] traversal and maps the target's
/// table-expression and parameter feature flags to the constructs they gate, so
/// every dialect — stock or custom — is validated purely from its feature data.
fn validate_target_support(
    features: &FeatureSet,
    statement: &Statement<NoExt>,
) -> RenderResult<()> {
    let mut support = TargetSupport {
        features,
        rejection: None,
    };
    support.visit_statement(statement);
    match support.rejection {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Collects the first unsupported construct found while walking a statement for a
/// render target. Recording stops at the first rejection so the diagnostic points
/// at the outermost offending node in visit order.
struct TargetSupport<'a> {
    features: &'a FeatureSet,
    rejection: Option<RenderError>,
}

impl TargetSupport<'_> {
    /// Record the first target-incompatible construct; later ones are ignored so
    /// the reported span stays the outermost one.
    fn reject(&mut self, span: Span, message: &str) {
        if self.rejection.is_none() {
            self.rejection = Some(RenderError::unsupported(Some(span), message));
        }
    }
}

impl<'ast> Visit<'ast> for TargetSupport<'_> {
    fn visit_table_factor(&mut self, node: &'ast TableFactor<NoExt>) {
        if self.rejection.is_some() {
            return;
        }
        let exprs = &self.features.table_expressions;
        let factors = &self.features.table_factor_syntax;
        match node {
            TableFactor::Table {
                inheritance,
                sample,
                meta,
                ..
            } => {
                if matches!(inheritance, RelationInheritance::Only(_)) && !exprs.only {
                    self.reject(
                        meta.span,
                        "target does not support ONLY inheritance suppression",
                    );
                } else if matches!(inheritance, RelationInheritance::Descendants) && !exprs.only {
                    self.reject(
                        meta.span,
                        "target does not support the descendant-table `*` marker",
                    );
                } else if sample.is_some() && !exprs.table_sample {
                    self.reject(meta.span, "target does not support TABLESAMPLE");
                }
            }
            TableFactor::Derived {
                lateral,
                spelling,
                meta,
                ..
            } => {
                if *lateral && !factors.lateral {
                    self.reject(meta.span, "target does not support LATERAL derived tables");
                } else if matches!(spelling, DerivedSpelling::BareValues) && !factors.from_values {
                    // A bare `FROM VALUES (…) AS t` cannot be re-emitted unparenthesized
                    // for a target that lacks the form — rendering it would produce
                    // `FROM VALUES …` the target parse-rejects.
                    self.reject(
                        meta.span,
                        "target does not support bare `FROM VALUES` table factors",
                    );
                }
            }
            TableFactor::Function {
                lateral,
                with_ordinality,
                meta,
                ..
            } => {
                if !factors.table_functions {
                    self.reject(meta.span, "target does not support table functions in FROM");
                } else if *with_ordinality && !factors.table_function_ordinality {
                    self.reject(meta.span, "target does not support WITH ORDINALITY");
                } else if *lateral && !factors.lateral {
                    self.reject(meta.span, "target does not support LATERAL table functions");
                }
            }
            TableFactor::RowsFrom {
                lateral,
                with_ordinality,
                meta,
                ..
            } => {
                if !factors.rows_from {
                    self.reject(
                        meta.span,
                        "target does not support ROWS FROM table functions",
                    );
                } else if *with_ordinality && !factors.table_function_ordinality {
                    self.reject(meta.span, "target does not support WITH ORDINALITY");
                } else if *lateral && !factors.lateral {
                    self.reject(meta.span, "target does not support LATERAL table functions");
                }
            }
            TableFactor::Unnest {
                lateral,
                with_offset,
                with_ordinality,
                meta,
                ..
            } => {
                if !factors.unnest {
                    self.reject(meta.span, "target does not support the UNNEST table factor");
                } else if *with_ordinality && !factors.table_function_ordinality {
                    self.reject(meta.span, "target does not support WITH ORDINALITY");
                } else if *with_offset && !factors.unnest_with_offset {
                    self.reject(meta.span, "target does not support UNNEST ... WITH OFFSET");
                } else if *lateral && !factors.lateral {
                    self.reject(meta.span, "target does not support LATERAL table functions");
                }
            }
            TableFactor::Pivot { meta, .. } => {
                if !factors.pivot {
                    self.reject(meta.span, "target does not support the PIVOT operator");
                }
            }
            TableFactor::Unpivot { meta, .. } => {
                if !factors.unpivot {
                    self.reject(meta.span, "target does not support the UNPIVOT operator");
                }
            }
            TableFactor::MatchRecognize { meta, .. } => {
                if !factors.match_recognize {
                    self.reject(
                        meta.span,
                        "target does not support the MATCH_RECOGNIZE table factor",
                    );
                }
            }
            TableFactor::ShowRef { meta, .. } => {
                if !factors.show_ref {
                    self.reject(
                        meta.span,
                        "target does not support DESCRIBE/SHOW/SUMMARIZE as a table source",
                    );
                }
            }
            TableFactor::JsonTable {
                json_table, meta, ..
            } => {
                if !factors.json_table {
                    self.reject(
                        meta.span,
                        "target does not support the JSON_TABLE table factor",
                    );
                } else if json_table.lateral && !factors.lateral {
                    self.reject(meta.span, "target does not support LATERAL table functions");
                }
            }
            TableFactor::XmlTable {
                xml_table, meta, ..
            } => {
                if !factors.xml_table {
                    self.reject(
                        meta.span,
                        "target does not support the XMLTABLE table factor",
                    );
                } else if xml_table.lateral && !factors.lateral {
                    self.reject(meta.span, "target does not support LATERAL table functions");
                }
            }
            TableFactor::OpenJson { meta, .. } => {
                if !factors.open_json {
                    self.reject(
                        meta.span,
                        "target does not support the OPENJSON table factor",
                    );
                }
            }
            TableFactor::TableExpr { meta, .. } => {
                if !factors.table_expr_factor {
                    self.reject(
                        meta.span,
                        "target does not support the TABLE(<expr>) table factor",
                    );
                }
            }
            // Unconstrained, like the sibling `Expr::SpecialFunction` at the
            // expression level: no target FeatureSet gates the special-function
            // keyword family (`CURRENT_DATE`, ...) today, in either position.
            TableFactor::SpecialFunction { .. }
            | TableFactor::NestedJoin { .. }
            | TableFactor::Other { .. } => {}
        }
        if self.rejection.is_some() {
            return;
        }
        walk_table_factor(self, node);
    }

    fn visit_join_constraint(&mut self, node: &'ast JoinConstraint<NoExt>) {
        if self.rejection.is_some() {
            return;
        }
        if let JoinConstraint::Using {
            alias: Some(_),
            meta,
            ..
        } = node
        {
            if !self.features.table_expressions.join_using_alias {
                self.reject(
                    meta.span,
                    "target does not support JOIN ... USING (...) AS alias",
                );
            }
        }
        if self.rejection.is_some() {
            return;
        }
        walk_join_constraint(self, node);
    }

    fn visit_expr(&mut self, node: &'ast Expr<NoExt>) {
        if self.rejection.is_some() {
            return;
        }
        if let Expr::Parameter { kind, meta } = node {
            let params = &self.features.parameters;
            let (supported, message) = match kind {
                ParameterKind::Positional(_) => (
                    params.positional_dollar,
                    "target does not support positional $n parameters",
                ),
                ParameterKind::Numbered(_) => (
                    params.numbered_question,
                    "target does not support numbered ?n parameters",
                ),
                ParameterKind::Anonymous => (
                    params.anonymous_question,
                    "target does not support anonymous ? parameters",
                ),
                ParameterKind::Named {
                    sigil: ParameterSigil::Colon,
                    ..
                } => (
                    params.named_colon,
                    "target does not support named :name parameters",
                ),
                ParameterKind::Named {
                    sigil: ParameterSigil::At,
                    ..
                } => (
                    params.named_at,
                    "target does not support named @name parameters",
                ),
                ParameterKind::Named {
                    sigil: ParameterSigil::Dollar,
                    ..
                } => (
                    params.named_dollar,
                    "target does not support named $name parameters",
                ),
            };
            if !supported {
                self.reject(meta.span, message);
            }
        }
        // A session variable renders back only where the target lexes its sigil, so gate
        // it like the parameter forms above (a MySQL `@x`/`@@x` cannot round-trip under a
        // target that reads `@` as a stray byte or a different operator).
        if let Expr::SessionVariable { kind, meta, .. } = node {
            let vars = &self.features.session_variables;
            let (supported, message) = match kind {
                SessionVariableKind::User => (
                    vars.user_variables,
                    "target does not support @name user variables",
                ),
                SessionVariableKind::System
                | SessionVariableKind::SystemGlobal
                | SessionVariableKind::SystemSession => (
                    vars.system_variables,
                    "target does not support @@name system variables",
                ),
            };
            if !supported {
                self.reject(meta.span, message);
            }
        }
        if self.rejection.is_some() {
            return;
        }
        walk_expr(self, node);
    }

    fn visit_returning(&mut self, node: &'ast Returning<NoExt>) {
        if self.rejection.is_some() {
            return;
        }
        if !self.features.mutation_syntax.returning {
            self.reject(node.meta.span, "target does not support RETURNING");
            return;
        }
        walk_returning(self, node);
    }

    fn visit_upsert(&mut self, node: &'ast Upsert<NoExt>) {
        if self.rejection.is_some() {
            return;
        }
        // The PostgreSQL `OnConflict` arm is gated by `visit_on_conflict` once the
        // walk descends into it; only the MySQL arm needs its own acceptance gate
        // here, since it carries no further gated child node.
        if let Upsert::OnDuplicateKeyUpdate { meta, .. } = node {
            if !self.features.mutation_syntax.on_duplicate_key_update {
                self.reject(meta.span, "target does not support ON DUPLICATE KEY UPDATE");
                return;
            }
        }
        walk_upsert(self, node);
    }

    fn visit_on_conflict(&mut self, node: &'ast OnConflict<NoExt>) {
        if self.rejection.is_some() {
            return;
        }
        if !self.features.mutation_syntax.on_conflict {
            self.reject(node.meta.span, "target does not support ON CONFLICT");
            return;
        }
        walk_on_conflict(self, node);
    }
}

/// Fallible dialect-target renderer.
#[derive(Clone, Debug)]
pub struct Renderer<D> {
    dialect: D,
    config: RenderConfig,
}

impl<D: RenderDialect> Renderer<D> {
    /// Build a renderer using the target's preferred Tier-1 spelling.
    // See the builder convention note at `Parser::recursion_limit`: a
    // `-> Self` builder is `#[must_use]` so a discarded construction cannot silently
    // no-op.
    #[must_use]
    pub fn new(dialect: D) -> Self {
        Self::with_config(dialect, RenderConfig::default())
    }

    /// Build a renderer with an explicit Tier-1 render config.
    #[must_use]
    pub fn with_config(dialect: D, mut config: RenderConfig) -> Self {
        config.target = dialect.render_features();
        config.spelling = RenderSpelling::TargetDialect;
        Self { dialect, config }
    }

    /// The target dialect validator.
    pub fn dialect(&self) -> &D {
        &self.dialect
    }

    /// The Tier-1 render config used after validation succeeds.
    pub fn config(&self) -> &RenderConfig {
        &self.config
    }
    /// Render a parsed root, separating statements with `; `.
    pub fn render_parsed<S: SourceStore>(&self, parsed: &Parsed<S>) -> RenderResult<String> {
        // Canonical output tracks source length closely (a 276-byte source measured
        // 270 rendered bytes), so reserving it up front renders in one allocation
        // instead of walking `String::new()`'s empty-start doubling chain into it.
        let mut out = String::with_capacity(parsed.source().len());
        for (i, statement) in parsed.statements().iter().enumerate() {
            if i > 0 {
                out.push_str("; ");
            }
            self.render_statement_into(statement, parsed.resolver(), parsed.source(), &mut out)?;
        }
        Ok(out)
    }

    /// Render one statement with an explicit resolver/source pair.
    pub fn render_statement(
        &self,
        statement: &Statement<NoExt>,
        resolver: &dyn Resolver,
        source: &str,
    ) -> RenderResult<String> {
        // Same pre-sizing rationale as `render_parsed`: this statement's canonical
        // rendering tracks its own source slice's length just as closely.
        let mut out = String::with_capacity(source.len());
        self.render_statement_into(statement, resolver, source, &mut out)?;
        Ok(out)
    }

    fn render_statement_into(
        &self,
        statement: &Statement<NoExt>,
        resolver: &dyn Resolver,
        source: &str,
        out: &mut String,
    ) -> RenderResult<()> {
        self.dialect.validate_statement(statement)?;
        // Borrowed, not cloned: `RenderConfig` embeds the target `FeatureSet` by
        // value (~848 bytes — keyword sets and binding-power tables), and this runs
        // once per statement from `render_parsed`'s loop. A clone here was an
        // N-statement memcpy of data that never changes across the render.
        let ctx = RenderCtx::new(resolver, source, &self.config);
        write!(out, "{}", statement.displayed(&ctx)).map_err(RenderError::from)
    }
}

/// The failure of a [`transpile`] call: the source parse or the target render.
///
/// A thin sum over the two independent failure modes of the parse-then-render
/// pipeline, mirroring the crate's other structured errors ([`ParseError`],
/// [`RenderError`]): [`Debug`](std::fmt::Debug)/[`Display`](std::fmt::Display)/[`Error`](std::error::Error) with a
/// [`source`](std::error::Error::source) forwarding to the wrapped error, plus
/// `From` both ways so a hand-rolled `parse_with(..)?` then `render_parsed(..)?`
/// pipeline can bubble into it with `?`. Each arm keeps the wrapped error whole —
/// its byte [`Span`] included — so a caller matches for the exact diagnostic (a
/// syntax error's expected/found, or a render rejection's
/// [`RenderErrorKind`]/span).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranspileError {
    /// `sql` did not parse under the source dialect.
    Parse(ParseError),
    /// `sql` parsed, but the target dialect cannot spell some construct
    /// (reject rather than mis-render).
    Render(RenderError),
}

impl fmt::Display for TranspileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "transpile could not parse the source: {error}"),
            Self::Render(error) => write!(f, "transpile could not render for the target: {error}"),
        }
    }
}

impl std::error::Error for TranspileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Render(error) => Some(error),
        }
    }
}

impl From<ParseError> for TranspileError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<RenderError> for TranspileError {
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}

/// Parse `sql` under `source` and render it for `target` in one call — the facade
/// convenience for cross-dialect transpilation.
///
/// Composition only: exactly [`parse_with`](crate::parse_with) under `source`
/// followed by [`Renderer::new(target).render_parsed`](Renderer::render_parsed),
/// with the two failure modes joined into [`TranspileError`]. It takes no options
/// by design — a caller who needs a non-default recursion limit, trivia capture, a
/// [redacted](RenderMode::Redacted) mode, or a [`Renderer`] reused across many
/// inputs composes those two pieces directly; this is the terse path for the
/// common case. Multiple statements render separated by `; `, exactly as
/// [`Renderer::render_parsed`] joins them.
///
/// Transpilation here is *syntactic*: the target validates that it
/// can spell every construct and rejects — rather than semantically rewrites — one
/// it lacks (unlike sqlglot's function-family rewriting). A construct with no
/// target spelling yields a [`TranspileError::Render`] carrying the offending
/// [`Span`], never invalid SQL. The bound is `Src: Dialect<Ext = NoExt>` because
/// Tier-2 rendering is defined over the canonical AST; every stock dialect
/// ([`Ansi`](crate::dialect::Ansi) and the feature-gated `Postgres`/`MySql`/`Lenient`)
/// satisfies it.
///
/// # Errors
///
/// [`TranspileError::Parse`] if `sql` does not parse under `source`;
/// [`TranspileError::Render`] if `target` cannot express a parsed construct.
///
/// ```
/// use squonk::dialect::Ansi;
/// use squonk::render::transpile;
///
/// // A single-dialect round-trip normalizes to canonical SQL.
/// assert_eq!(transpile("select 1", Ansi, Ansi).expect("ANSI round-trip"), "SELECT 1");
/// ```
pub fn transpile<Src, Tgt>(sql: &str, source: Src, target: Tgt) -> Result<String, TranspileError>
where
    Src: Dialect<Ext = NoExt>,
    Tgt: RenderDialect,
{
    let parsed = crate::parse_with(sql, crate::ParseConfig::new(source))?;
    Renderer::new(target)
        .render_parsed(&parsed)
        .map_err(Into::into)
}

// --- Fragment rendering: canonical SQL for a single sub-node ------------------
//
// `Parsed`'s `Display`/`to_sql` render the *whole* tree; the entry points below add
// the fragment surface — canonical SQL for just one sub-node — that linters, LSPs,
// rewriters, and query-explainer UIs want. They hang off the parse root because a
// fragment needs the same resolver + source the root owns; a free function could not
// resolve the node's symbols or slice its literals. Unlike the Tier-2 `Renderer`
// above, fragment rendering is infallible for the by-reference path and does no
// target-support validation — it is the Tier-1 canonical walk applied to one node.

/// The failure of a [`Parsed::render_fragment_by_id`] lookup: no
/// standalone-renderable node in the tree carries the requested [`NodeId`].
///
/// This is the *runtime* form of the [`FragmentRender`] compile-time gate: the by-id
/// path cannot reject a context-dependent node at the type level (it is handed only a
/// numeric id), so it declines instead of rendering a fragment that would not stand
/// alone. The id may be absent entirely, or may name a node whose kind is not on the
/// allowlist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FragmentError {
    node_id: NodeId,
}

impl FragmentError {
    /// The node id that could not be rendered as a standalone fragment.
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
}

impl fmt::Display for FragmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no standalone-renderable node with id {}: the fragment API renders only \
             complete expressions, queries, statements, and data types; the id may be \
             absent or may name a context-dependent node (such as a join constraint, \
             select item, or order-by term) that cannot render on its own",
            self.node_id.as_u32(),
        )
    }
}

impl std::error::Error for FragmentError {}

impl<S: SourceStore, X: Extension + Render> Parsed<S, X> {
    /// Render a single sub-node back to canonical SQL, using this root's resolver
    /// and source.
    ///
    /// The node must be one of the standalone-renderable kinds — an [`Expr`],
    /// [`Query`], [`Statement`], or [`DataType`] — enforced at compile time by the
    /// [`FragmentRender`] bound. Rendering uses the canonical default config
    /// ([`RenderSpelling::PreserveSource`]);
    /// for a redacted or dialect-target fragment, use
    /// [`render_fragment_with`](Self::render_fragment_with).
    ///
    /// The node must come from *this* tree: its symbols index this root's resolver
    /// and its literal spans index this root's source, exactly as for whole-tree
    /// [`Display`](std::fmt::Display) (the cross-tree-safety contract on [`Parsed`]).
    pub fn render_fragment<N: FragmentRender>(&self, node: &N) -> String {
        self.render_fragment_with(node, &RenderConfig::default())
    }

    /// Render a single sub-node with an explicit [`RenderConfig`] — the fragment
    /// counterpart of the whole-tree render modes.
    ///
    /// The same allowlist and cross-tree rules as
    /// [`render_fragment`](Self::render_fragment) apply; only the config differs, so a
    /// fragment can be [redacted](RenderMode::Redacted) or spelled for a target
    /// dialect just like the whole tree.
    pub fn render_fragment_with<N: FragmentRender>(
        &self,
        node: &N,
        config: &RenderConfig,
    ) -> String {
        let ctx = RenderCtx::new(self.resolver(), self.source(), config);
        let mut out = String::new();
        write!(out, "{}", node.displayed(&ctx))
            .expect("rendering a fragment to a String cannot fail");
        out
    }

    /// Render the standalone-renderable node carrying `node_id` back to canonical
    /// SQL, or fail if no such node exists.
    ///
    /// The runtime-gated sibling of [`render_fragment`](Self::render_fragment) for
    /// callers that hold a node by id rather than by reference (the language
    /// facades). It walks the tree and renders the first [`Expr`] / [`Query`] /
    /// [`Statement`] / [`DataType`] whose id matches, enforcing the same allowlist:
    /// an id that is absent, or that names a context-dependent node, yields a
    /// [`FragmentError`] instead of misleading SQL.
    ///
    /// # Errors
    ///
    /// [`FragmentError`] when `node_id` matches no standalone-renderable node.
    pub fn render_fragment_by_id(
        &self,
        node_id: NodeId,
        config: &RenderConfig,
    ) -> Result<String, FragmentError> {
        let mut finder = FragmentFinder {
            resolver: self.resolver(),
            source: self.source(),
            config,
            target: node_id,
            rendered: None,
        };
        for statement in self.statements() {
            if finder.rendered.is_some() {
                break;
            }
            finder.visit_statement(statement);
        }
        finder.rendered.ok_or(FragmentError { node_id })
    }
}

/// Walks the tree looking for the allowlisted node with `target`, rendering the
/// first match into `rendered` and short-circuiting the rest of the walk.
struct FragmentFinder<'a> {
    resolver: &'a dyn Resolver,
    source: &'a str,
    config: &'a RenderConfig,
    target: NodeId,
    rendered: Option<String>,
}

impl FragmentFinder<'_> {
    /// Render the matched node with this finder's ctx and record it.
    fn take<N: FragmentRender>(&mut self, node: &N) {
        let ctx = RenderCtx::new(self.resolver, self.source, self.config);
        let mut out = String::new();
        write!(out, "{}", node.displayed(&ctx))
            .expect("rendering a fragment to a String cannot fail");
        self.rendered = Some(out);
    }
}

impl<'ast, X: Extension + Render> Visit<'ast, X> for FragmentFinder<'_> {
    fn visit_statement(&mut self, node: &'ast Statement<X>) {
        if self.rendered.is_some() {
            return;
        }
        if node.fragment_node_id() == self.target {
            self.take(node);
            return;
        }
        walk_statement(self, node);
    }

    fn visit_query(&mut self, node: &'ast Query<X>) {
        if self.rendered.is_some() {
            return;
        }
        if node.fragment_node_id() == self.target {
            self.take(node);
            return;
        }
        walk_query(self, node);
    }

    fn visit_expr(&mut self, node: &'ast Expr<X>) {
        if self.rendered.is_some() {
            return;
        }
        if node.fragment_node_id() == self.target {
            self.take(node);
            return;
        }
        walk_expr(self, node);
    }

    fn visit_data_type(&mut self, node: &'ast DataType<X>) {
        if self.rendered.is_some() {
            return;
        }
        if node.fragment_node_id() == self.target {
            self.take(node);
            return;
        }
        walk_data_type(self, node);
    }
}

#[cfg(test)]
mod fragment_tests {
    use super::*;
    use crate::ast::render::RenderMode;
    use crate::ast::{SelectItem, SetExpr};
    use crate::dialect::Ansi;
    use crate::parse_with;
    use std::sync::Arc;

    /// Pull the first projection expression out of a single-`SELECT` parse.
    fn first_projection_expr<X: Extension>(parsed: &Parsed<Arc<str>, X>) -> &Expr<X> {
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!("expected an expression projection");
        };
        expr
    }

    #[test]
    fn render_fragment_renders_an_expression_standalone() {
        let parsed = parse_with("SELECT a + 1", crate::ParseConfig::new(Ansi)).expect("parses");
        let expr = first_projection_expr(&parsed);
        // Just the sub-tree, canonically — not the owning `SELECT`.
        assert_eq!(parsed.render_fragment(expr), "a + 1");
    }

    #[test]
    fn render_fragment_by_id_matches_the_by_reference_path() {
        let parsed = parse_with(
            "SELECT a + 1 FROM t WHERE b > 2",
            crate::ParseConfig::new(Ansi),
        )
        .expect("parses");
        let expr = first_projection_expr(&parsed);
        let id = expr.fragment_node_id();
        let by_ref = parsed.render_fragment(expr);
        let by_id = parsed
            .render_fragment_by_id(id, &RenderConfig::default())
            .expect("the expression id resolves");
        assert_eq!(by_ref, by_id);
        assert_eq!(by_id, "a + 1");
    }

    #[test]
    fn render_fragment_by_id_renders_a_whole_query_and_statement() {
        let parsed = parse_with(
            "SELECT a FROM t WHERE b IN (SELECT c FROM u)",
            crate::ParseConfig::new(Ansi),
        )
        .expect("parses");
        let Statement::Query { query, meta, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        // The outer statement id renders the whole statement.
        assert_eq!(
            parsed
                .render_fragment_by_id(meta.node_id, &RenderConfig::default())
                .expect("statement id resolves"),
            "SELECT a FROM t WHERE b IN (SELECT c FROM u)",
        );
        // The outer query id renders the query body (here identical to the statement).
        assert_eq!(
            parsed
                .render_fragment_by_id(query.meta.node_id, &RenderConfig::default())
                .expect("query id resolves"),
            "SELECT a FROM t WHERE b IN (SELECT c FROM u)",
        );
    }

    #[test]
    fn render_fragment_by_id_rejects_a_context_dependent_node() {
        let parsed = parse_with("SELECT a FROM t", crate::ParseConfig::new(Ansi)).expect("parses");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        // A `Select` is not on the allowlist (it is not one of the four standalone
        // kinds), so by-id declines rather than rendering a bare `SELECT`-clause body.
        let error = parsed
            .render_fragment_by_id(select.meta.node_id, &RenderConfig::default())
            .expect_err("a Select node is not standalone-renderable");
        assert_eq!(error.node_id(), select.meta.node_id);
        assert!(error.to_string().contains("standalone-renderable"));
    }

    #[test]
    fn render_fragment_by_id_rejects_an_absent_id() {
        let parsed = parse_with("SELECT 1", crate::ParseConfig::new(Ansi)).expect("parses");
        let absent = NodeId::new(u32::MAX).expect("non-zero id");
        parsed
            .render_fragment_by_id(absent, &RenderConfig::default())
            .expect_err("no node carries this id");
    }

    #[test]
    fn render_fragment_with_honours_redacted_mode() {
        let parsed =
            parse_with("SELECT secret + 42", crate::ParseConfig::new(Ansi)).expect("parses");
        let expr = first_projection_expr(&parsed);
        let config = RenderConfig {
            mode: RenderMode::Redacted,
            ..RenderConfig::default()
        };
        // Redaction masks the identifier and the literal, exactly as whole-tree redaction does.
        assert_eq!(parsed.render_fragment_with(expr, &config), "id + ?");
    }

    #[test]
    fn expression_fragment_reparses_inside_a_select() {
        // An `Expr` fragment is a parseable unit once placed in expression position:
        // re-parsing `SELECT <fragment>` and re-rendering the projection reproduces it.
        let parsed = parse_with("SELECT (a + 1) * b - c / 2", crate::ParseConfig::new(Ansi))
            .expect("parses");
        let expr = first_projection_expr(&parsed);
        let fragment = parsed.render_fragment(expr);

        let reparsed = parse_with(&format!("SELECT {fragment}"), crate::ParseConfig::new(Ansi))
            .expect("the expression fragment re-parses in projection position");
        let reparsed_expr = first_projection_expr(&reparsed);
        assert_eq!(reparsed.render_fragment(reparsed_expr), fragment);
    }

    #[test]
    fn query_fragment_reparses_as_a_statement() {
        // A `Query` fragment is a complete statement: re-parsing it standalone and
        // re-rendering reproduces it.
        let parsed = parse_with(
            "SELECT a FROM t WHERE b IN (SELECT c FROM u WHERE d > 1)",
            crate::ParseConfig::new(Ansi),
        )
        .expect("parses");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        // Grab the inner subquery (a nested Query) and render it standalone.
        let Some(Expr::InSubquery { subquery, .. }) = &select.selection else {
            panic!("expected an IN (subquery) predicate");
        };
        let fragment = parsed.render_fragment(subquery.as_ref());
        assert_eq!(fragment, "SELECT c FROM u WHERE d > 1");

        let reparsed = parse_with(&fragment, crate::ParseConfig::new(Ansi))
            .expect("the query fragment re-parses as a standalone statement");
        assert_eq!(reparsed.to_sql(), fragment);
    }

    #[test]
    fn data_type_fragment_reparses_inside_a_cast() {
        // A `DataType` fragment is a parseable unit in a `CAST(... AS <type>)` position.
        let parsed = parse_with(
            "SELECT CAST(x AS DECIMAL(10, 2))",
            crate::ParseConfig::new(Ansi),
        )
        .expect("parses");
        let expr = first_projection_expr(&parsed);
        let Expr::Cast { data_type, .. } = expr else {
            panic!("expected a CAST expression");
        };
        let fragment = parsed.render_fragment(data_type.as_ref());
        assert_eq!(fragment, "DECIMAL(10, 2)");

        let reparsed = parse_with(
            &format!("SELECT CAST(x AS {fragment})"),
            crate::ParseConfig::new(Ansi),
        )
        .expect("the data-type fragment re-parses in a CAST");
        let reparsed_expr = first_projection_expr(&reparsed);
        let Expr::Cast { data_type, .. } = reparsed_expr else {
            panic!("expected a CAST expression");
        };
        assert_eq!(reparsed.render_fragment(data_type.as_ref()), fragment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::dialect::{FeatureDelta, OperatorSyntax, ParameterSyntax};
    use crate::ast::{CteBody, Query, SetExpr, Span};
    // `Postgres` is gated; these Tier-2 render tests reach it through the workspace's
    // dialect-feature unification (the `full`-enabled dev crates), like the parser's
    // other cross-dialect tests.
    use crate::dialect::{Ansi, Postgres, parse};
    use crate::parse_with;
    use crate::parser::FeatureDialect;

    #[test]
    fn stock_renderer_delegates_to_tier1_canonical_rendering() {
        let parsed = parse("SELECT TRUE, NULL").expect("query parses");
        let rendered = Renderer::new(Ansi)
            .render_parsed(&parsed)
            .expect("stock ANSI target renders");

        assert_eq!(rendered, "SELECT TRUE, NULL");
    }

    #[test]
    fn renderer_keeps_tier1_modes_after_target_validation() {
        let parsed = parse("SELECT TRUE, NULL").expect("query parses");
        let renderer = Renderer::with_config(
            Ansi,
            RenderConfig {
                mode: RenderMode::Redacted,
                ..RenderConfig::default()
            },
        );

        assert_eq!(
            renderer.render_parsed(&parsed).expect("redacted renders"),
            "SELECT ?, ?",
        );
    }

    #[test]
    fn renderer_uses_dialect_target_type_spellings() {
        assert_eq!(Renderer::new(Ansi).config().target, FeatureSet::ANSI);
        assert_eq!(
            Renderer::new(Postgres).config().target,
            FeatureSet::POSTGRES
        );
        assert_eq!(
            Renderer::new(Postgres).config().spelling,
            RenderSpelling::TargetDialect,
        );

        let ansi = parse("SELECT CAST(a AS VARCHAR(5))").expect("ANSI cast parses");
        assert_eq!(
            Renderer::new(Ansi)
                .render_parsed(&ansi)
                .expect("ANSI target renders"),
            "SELECT CAST(a AS CHARACTER VARYING(5))",
        );

        let postgres = crate::parse_with(
            "SELECT CAST(a AS TIMESTAMP(3) WITH TIME ZONE)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("PostgreSQL cast parses");
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&postgres)
                .expect("PostgreSQL target renders"),
            "SELECT CAST(a AS TIMESTAMPTZ(3))",
        );
    }

    #[test]
    fn renderer_handles_create_table_statements() {
        let parsed = parse("CREATE TABLE t (id INT PRIMARY KEY, name TEXT NOT NULL DEFAULT 'x')")
            .expect("CREATE TABLE parses");

        assert_eq!(
            Renderer::new(Ansi)
                .render_parsed(&parsed)
                .expect("CREATE TABLE renders"),
            "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL DEFAULT 'x')",
        );
    }

    #[test]
    fn renderer_handles_create_table_as_select() {
        let parsed =
            parse("CREATE TEMP TABLE IF NOT EXISTS t (id) ON COMMIT DROP AS SELECT 1 WITH NO DATA")
                .expect("CTAS parses");

        assert_eq!(
            Renderer::new(Ansi)
                .render_parsed(&parsed)
                .expect("CTAS renders"),
            "CREATE TEMP TABLE IF NOT EXISTS t (id) ON COMMIT DROP AS SELECT 1 WITH NO DATA",
        );
    }

    #[test]
    fn renderer_handles_insert_statements() {
        let parsed = parse(
            "WITH src AS (SELECT 1) INSERT INTO t AS target (id) OVERRIDING USER VALUE SELECT * FROM src",
        )
        .expect("INSERT parses");

        assert_eq!(
            Renderer::new(Ansi)
                .render_parsed(&parsed)
                .expect("INSERT renders"),
            "WITH src AS (SELECT 1) INSERT INTO t AS target (id) OVERRIDING USER VALUE SELECT * FROM src",
        );
    }

    #[test]
    fn renderer_handles_update_and_delete_statements() {
        let parsed = parse(
            "UPDATE t target SET a = 1, b = DEFAULT FROM u WHERE target.id = u.id; \
             WITH src AS (SELECT 1) DELETE FROM t target USING u WHERE target.id = u.id",
        )
        .expect("UPDATE and DELETE parse");

        assert_eq!(
            Renderer::new(Ansi)
                .render_parsed(&parsed)
                .expect("UPDATE and DELETE render"),
            "UPDATE t AS target SET a = 1, b = DEFAULT FROM u WHERE target.id = u.id; \
             WITH src AS (SELECT 1) DELETE FROM t AS target USING u WHERE target.id = u.id",
        );
    }

    #[test]
    fn renderer_handles_returning_and_on_conflict_statements() {
        // PostgreSQL mutation extensions round-trip through the PostgreSQL target:
        // RETURNING on each mutation statement, and the ON CONFLICT arbiter/action
        // forms (index columns with a predicate, DO UPDATE, named constraint, DO NOTHING).
        let parsed = parse_with(
            "INSERT INTO t (id, n) VALUES (1, 2) ON CONFLICT (id) WHERE id > 0 DO UPDATE SET n = excluded.n WHERE t.n < excluded.n RETURNING id; \
             INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT t_pkey DO NOTHING; \
             UPDATE t SET a = 1 WHERE id = 2 RETURNING a, id; \
             DELETE FROM t WHERE id = 1 RETURNING *",
            crate::ParseConfig::new(Postgres),
        )
        .expect("RETURNING and ON CONFLICT parse under PostgreSQL");

        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("RETURNING and ON CONFLICT render"),
            "INSERT INTO t (id, n) VALUES (1, 2) ON CONFLICT (id) WHERE id > 0 DO UPDATE SET n = excluded.n WHERE t.n < excluded.n RETURNING id; \
             INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT t_pkey DO NOTHING; \
             UPDATE t SET a = 1 WHERE id = 2 RETURNING a, id; \
             DELETE FROM t WHERE id = 1 RETURNING *",
        );
    }

    #[test]
    fn renderer_reports_unsupported_target_constructs() {
        #[derive(Clone, Copy, Debug)]
        struct NoValuesTarget;

        impl RenderDialect for NoValuesTarget {
            fn validate_statement(&self, statement: &Statement<NoExt>) -> RenderResult<()> {
                if let Some(span) = first_values_span(statement) {
                    return Err(RenderError::unsupported(
                        Some(span),
                        "target does not support VALUES query bodies",
                    ));
                }
                Ok(())
            }
        }

        let parsed = parse("VALUES (1)").expect("VALUES query parses");
        let error = Renderer::new(NoValuesTarget)
            .render_parsed(&parsed)
            .expect_err("target rejects VALUES");

        assert_eq!(error.kind(), RenderErrorKind::Unsupported);
        assert_eq!(error.span(), Some(Span::new(0, 10)));
        assert!(error.message().contains("VALUES"));
        assert_eq!(
            error.to_string(),
            "target does not support VALUES query bodies at bytes 0..10",
        );
    }

    #[test]
    fn ansi_target_rejects_postgres_only_constructs() {
        // Each construct parses under PostgreSQL but ANSI cannot spell it, so the
        // Tier-2 ANSI target rejects it with an unsupported-construct diagnostic
        // (ADR-0010) instead of emitting invalid SQL; PostgreSQL renders each. The
        // gate is the target FeatureSet, not a hand-coded per-dialect list (ADR-0011).
        let cases = [
            ("SELECT * FROM ONLY t", "ONLY"),
            ("SELECT * FROM t TABLESAMPLE BERNOULLI (10)", "TABLESAMPLE"),
            ("SELECT * FROM generate_series(1, 5)", "table functions"),
            ("SELECT * FROM LATERAL (SELECT 1) AS x", "LATERAL"),
            ("SELECT * FROM t1 JOIN t2 USING (id) AS j", "USING"),
            ("SELECT $1", "positional"),
            ("INSERT INTO t VALUES (1) RETURNING id", "RETURNING"),
            (
                "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING",
                "ON CONFLICT",
            ),
        ];
        for (sql, needle) in cases {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} parses under PostgreSQL: {err:?}"));

            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("PostgreSQL target renders {sql:?}: {err}"));

            let error = Renderer::new(Ansi)
                .render_parsed(&parsed)
                .expect_err("ANSI target rejects the PostgreSQL-only construct");
            assert_eq!(error.kind(), RenderErrorKind::Unsupported, "{sql:?}");
            assert!(error.span().is_some(), "{sql:?} carries a span");
            assert!(
                error.message().contains(needle),
                "{sql:?}: {:?} should mention {needle:?}",
                error.message(),
            );
        }
    }

    #[test]
    fn postgres_target_rejects_anonymous_parameter_placeholders() {
        // `?` parses only under a dialect that enables anonymous placeholders; the
        // stock PostgreSQL target has positional `$n` only, so it rejects a `?`
        // source while the custom target that allows `?` renders it. This exercises
        // the parameter axis and the custom-target path of the data-driven gate.
        // Enabling the anonymous `?` placeholder on the PostgreSQL base must vacate the
        // `jsonb` operators, which claim the same `?` trigger
        // (`LexicalConflict::JsonbKeyExistsVersusAnonymousParameter`), or the set is
        // lexically inconsistent.
        const ANON_PARAM: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY
                .parameters(ParameterSyntax {
                    anonymous_question: true,
                    ..ParameterSyntax::POSTGRES
                })
                .operator_syntax(OperatorSyntax {
                    jsonb_operators: false,
                    ..OperatorSyntax::POSTGRES
                }),
        );

        const ANON_PARAM_DIALECT: FeatureDialect = FeatureDialect {
            features: &ANON_PARAM,
        };

        let parsed = parse_with("SELECT ?", crate::ParseConfig::new(ANON_PARAM_DIALECT))
            .expect("anonymous-parameter dialect parses ?");

        assert_eq!(
            Renderer::new(ANON_PARAM_DIALECT)
                .render_parsed(&parsed)
                .expect("custom target renders ?"),
            "SELECT ?",
        );

        let error = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .expect_err("PostgreSQL has no ? placeholder");
        assert_eq!(error.kind(), RenderErrorKind::Unsupported);
        assert!(
            error.message().contains("anonymous"),
            "{:?}",
            error.message(),
        );
    }

    #[test]
    fn transpile_renders_cross_dialect_type_spellings() {
        // A PostgreSQL source parses, then the ANSI target prefers the standard
        // type spelling — the point of a cross-dialect transpile.
        assert_eq!(
            transpile("SELECT CAST(a AS VARCHAR(5))", Postgres, Ansi)
                .expect("ANSI can spell this cast"),
            "SELECT CAST(a AS CHARACTER VARYING(5))",
        );
    }

    #[test]
    fn transpile_passes_through_parse_errors_with_their_span() {
        // `FROM` cannot begin a statement: the source parse fails, and transpile
        // surfaces that error whole, span preserved.
        let error = transpile("FROM t", Ansi, Ansi).expect_err("FROM is not a statement");
        let TranspileError::Parse(parse_error) = error else {
            panic!("expected a parse failure, got {error:?}");
        };
        assert_eq!(parse_error.span, Span::new(0, 4));
    }

    #[test]
    fn transpile_passes_through_render_rejections_with_kind_and_span() {
        // `$1` parses under PostgreSQL but ANSI cannot spell it, so the target
        // rejects it — transpile surfaces the render error's kind and span, never
        // invalid SQL.
        let error = transpile("SELECT $1", Postgres, Ansi).expect_err("ANSI has no $n");
        let TranspileError::Render(render_error) = error else {
            panic!("expected a render rejection, got {error:?}");
        };
        assert_eq!(render_error.kind(), RenderErrorKind::Unsupported);
        assert!(render_error.span().is_some());
    }

    #[test]
    fn transpile_joins_multiple_statements_with_semicolons() {
        // Multi-statement input round-trips through `render_parsed`'s `; ` join.
        assert_eq!(
            transpile("SELECT 1; SELECT 2", Ansi, Ansi).expect("both statements render"),
            "SELECT 1; SELECT 2",
        );
    }

    fn first_values_span(statement: &Statement<NoExt>) -> Option<Span> {
        first_values_span_query(statement.as_query()?)
    }

    fn first_values_span_query(query: &Query<NoExt>) -> Option<Span> {
        query
            .with
            .as_ref()
            .and_then(|with| {
                // Only query CTE bodies can hold a `Values` body; the DML arms'
                // `VALUES` rows are `InsertValues`, a different node.
                with.ctes.iter().find_map(|cte| match &cte.body {
                    CteBody::Query { query, .. } => first_values_span_query(query),
                    _ => None,
                })
            })
            .or_else(|| first_values_span_set(&query.body))
    }

    fn first_values_span_set(set: &SetExpr<NoExt>) -> Option<Span> {
        match set {
            SetExpr::Values { meta, .. } => Some(meta.span),
            SetExpr::Query { query, .. } => first_values_span_query(query),
            SetExpr::SetOperation { left, right, .. } => {
                first_values_span_set(left).or_else(|| first_values_span_set(right))
            }
            SetExpr::Select { .. } | SetExpr::Pivot { .. } | SetExpr::Unpivot { .. } => None,
        }
    }
}
