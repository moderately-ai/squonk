// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Renderer tests.
//!
//! The parser lives in another crate, so every AST here is built by hand and a
//! tiny `Vec`-backed [`Resolver`] stands in for the interner. Literals carry a
//! span into a supplied `source` string, mirroring how the real pipeline slices
//! literal text verbatim.

use super::{
    Render, RenderConfig, RenderCtx, RenderError, RenderErrorKind, RenderExt, RenderMode,
    RenderSpelling,
};
use crate::ast::{
    AliasSpelling, AlterColumnAction, AlterColumnTarget, AlterTable, AlterTableAction, AlterView,
    ApplyKind, ArgSyntax, ArrayExpr, ArraySpelling, AsOfJoinKind, AtTimeZoneExpr, BinaryOperator,
    BlobTypeName, CaseExpr, CastSyntax, CharacterTypeName, CollateExpr, ColumnConstraint,
    ColumnDef, ColumnOption, ConflictAction, ConflictTarget, CreateIndex, CreateSchema,
    CreateTable, CreateTableBody, CreateTableOption, CreateTableOptionKind, CreateView, Cte,
    CteBody, DataType, DefaultValue, Definer, Delete, DerivedSpelling, DmlSelection, DmlTarget,
    DoubleTypeName, DropBehavior, DropObjectKind, DropStatement, EqualsSpelling, Expr, ExtractExpr,
    FetchSpelling, FieldSelectionExpr, FieldSelector, FilterWhereSpelling, ForeignKeyMatch,
    ForeignKeyRef, FunctionArg, FunctionCall, GroupByAllSpelling, GroupByItem, Ident, IndexColumn,
    Insert, InsertSource, InsertTarget, InsertVerb, IntegerTypeName, Join, JoinConstraint,
    JoinOperator, Limit, LimitPercent, LimitSyntax, Literal, LiteralKind, MapEntry, MapExpr, NoExt,
    NullTestSpelling, ObjectName, OnCommitAction, OnConflict, OnlySyntax, OrderByExpr,
    ParameterKind, ParameterSigil, Quantifier, Query, QuoteStyle, ReferentialAction,
    RelationInheritance, Returning, RowExpr, Select, SelectDistinct, SelectItem, SelectSpelling,
    SemiAntiSide, SetExpr, SetOperator, SetQuantifier, Signedness, SqlSecurityContext, Statement,
    StringFunc, StructExpr, StructField, StructKeySpelling, SubscriptExpr, SubscriptKind,
    TableAlias, TableConstraint, TableConstraintDef, TableElement, TableFactor, TableSample,
    TableWithJoins, TemporaryTableKind, TextTypeName, TimeTypeName, TimeZone, TimestampTypeName,
    TruthValue, UnaryOperator, Update, UpdateAssignment, UpdateTupleSource, UpdateValue, Upsert,
    Values, ValuesItem, ViewAlgorithm, ViewCheckOption, ViewOptions, WhenClause, With,
};
use crate::dialect::{FeatureDelta, FeatureSet};
use crate::precedence::{Assoc, BindingPower};
use crate::vocab::{Meta, NodeId, Resolver, Span, Symbol};
use thin_vec::{ThinVec, thin_vec};

// --- test harness ----------------------------------------------------------

/// Resolves one-based [`Symbol`]s to entries of a string table.
struct VecResolver(Vec<&'static str>);

impl Resolver for VecResolver {
    fn try_resolve(&self, sym: Symbol) -> Option<&str> {
        self.0.get(sym.index()).copied()
    }
}

fn render_with(
    node: &impl Render,
    resolver: &dyn Resolver,
    source: &str,
    mode: RenderMode,
) -> String {
    let config = RenderConfig {
        mode,
        ..RenderConfig::default()
    };
    let ctx = RenderCtx::new(resolver, source, &config);
    node.displayed(&ctx).to_string()
}

fn render_config(node: &impl Render, config: RenderConfig) -> String {
    let resolver = r_abc();
    let ctx = RenderCtx::new(&resolver, "", &config);
    node.displayed(&ctx).to_string()
}

fn render_target(node: &impl Render, target: FeatureSet) -> String {
    render_config(
        node,
        RenderConfig {
            target,
            spelling: RenderSpelling::TargetDialect,
            ..RenderConfig::default()
        },
    )
}

const LEFT_ASSOC_COMPARISON_FEATURES: FeatureSet = FeatureSet::ANSI.with(
    FeatureDelta::EMPTY.binding_powers(FeatureSet::ANSI.binding_powers.with_binary(
        &BinaryOperator::Lt,
        BindingPower {
            left: 40,
            right: 41,
            assoc: Assoc::Left,
        },
    )),
);

const HIGH_UNION_SET_OPERATION_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.set_operation_powers(
        FeatureSet::ANSI.set_operation_powers.with_set_operator(
            &SetOperator::Union,
            BindingPower {
                left: 30,
                right: 31,
                assoc: Assoc::Left,
            },
        ),
    ));

/// `a`, `b`, `c` -> symbols 1, 2, 3.
fn r_abc() -> VecResolver {
    VecResolver(vec!["a", "b", "c"])
}

fn canon(node: &impl Render) -> String {
    render_with(node, &r_abc(), "", RenderMode::Canonical)
}

fn paren(node: &impl Render) -> String {
    render_with(node, &r_abc(), "", RenderMode::Parenthesized)
}

#[test]
fn match_recognize_pattern_renders_lexer_unreachable_forms() {
    use crate::ast::{MatchRecognizePattern, RepetitionQuantifier};
    // The `$` end anchor and `?` (AtMostOne) quantifier have no lexer-reachable parser
    // path — the eager context-free tokenizer cannot emit `$`/`?` in pattern position —
    // but the AST models them for sqlparser-rs parity, and their render arms must stay
    // lossless (a programmatic builder can still produce them).
    let end = MatchRecognizePattern::End { meta: meta(0, 1) };
    assert_eq!(canon(&end), "$");

    let optional = MatchRecognizePattern::Repetition {
        pattern: Box::new(MatchRecognizePattern::Symbol {
            symbol: ident(1, QuoteStyle::None),
            meta: meta(0, 1),
        }),
        quantifier: RepetitionQuantifier::AtMostOne,
        meta: meta(0, 2),
    };
    assert_eq!(canon(&optional), "a?");
}

#[test]
fn render_error_reports_kind_message_and_span() {
    let error = RenderError::unsupported(Some(Span::new(2, 7)), "unsupported target syntax");

    assert_eq!(error.kind(), RenderErrorKind::Unsupported);
    assert_eq!(error.span(), Some(Span::new(2, 7)));
    assert_eq!(error.message(), "unsupported target syntax");
    assert_eq!(error.to_string(), "unsupported target syntax at bytes 2..7",);
}

#[test]
fn render_config_makes_mode_and_target_dialect_explicit() {
    let config = RenderConfig::default();

    assert_eq!(config.mode, RenderMode::Canonical);
    assert_eq!(config.target, FeatureSet::ANSI);
    assert_eq!(config.spelling, RenderSpelling::PreserveSource);
}

// --- node builders ---------------------------------------------------------

fn meta(start: u32, end: u32) -> Meta {
    Meta::new(
        Span::new(start, end),
        NodeId::new(1).expect("non-zero node id"),
    )
}

fn sym(id: u32) -> Symbol {
    Symbol::new(id).expect("non-zero symbol")
}

fn ident(id: u32, quote: QuoteStyle) -> Ident {
    Ident {
        sym: sym(id),
        quote,
        meta: meta(0, 0),
    }
}

fn name(id: u32) -> ObjectName {
    ObjectName(thin_vec![ident(id, QuoteStyle::None)])
}

fn table_alias(id: u32, columns: &[u32]) -> Box<TableAlias> {
    Box::new(TableAlias {
        name: ident(id, QuoteStyle::None),
        columns: columns
            .iter()
            .map(|id| ident(*id, QuoteStyle::None))
            .collect(),
        spelling: AliasSpelling::As,
        meta: meta(0, 0),
    })
}

/// A plain, unaliased, unsampled `TableFactor::Table` for `name(id)`.
fn plain_table(id: u32) -> TableFactor {
    TableFactor::Table {
        name: name(id),
        inheritance: RelationInheritance::Plain,
        json_path: ThinVec::new(),
        version: None,
        partition: ThinVec::new(),
        alias: None,
        indexed_by: None,
        index_hints: ThinVec::new(),
        sample: None,
        table_hints: ThinVec::new(),
        meta: meta(0, 0),
    }
}

fn col(id: u32) -> Expr {
    Expr::Column {
        name: name(id),
        meta: meta(0, 0),
    }
}

/// A bare-column `IndexColumn` (no `COLLATE`/`ASC`/`DESC`) — the `PRIMARY KEY`/`UNIQUE`
/// constraint-column shape for a plain name.
fn key_col(id: u32) -> IndexColumn {
    IndexColumn {
        expr: col(id),
        asc: None,
        nulls_first: None,
        meta: meta(0, 0),
    }
}

/// A positional function argument wrapping `value`.
fn pos_arg(value: Expr) -> FunctionArg {
    FunctionArg {
        name: None,
        variadic: false,
        syntax: ArgSyntax::Positional,
        value,
        meta: meta(0, 0),
    }
}

fn values_item(expr: Expr) -> ValuesItem {
    ValuesItem::Expr {
        expr,
        meta: meta(0, 0),
    }
}

fn values_default_item() -> ValuesItem {
    ValuesItem::Default {
        default: DefaultValue { meta: meta(0, 0) },
        meta: meta(0, 0),
    }
}

fn bin(left: Expr, op: BinaryOperator, right: Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
        meta: meta(0, 0),
    }
}

fn un(op: UnaryOperator, expr: Expr) -> Expr {
    Expr::UnaryOp {
        op,
        expr: Box::new(expr),
        meta: meta(0, 0),
    }
}

fn is_null(expr: Expr) -> Expr {
    Expr::IsNull {
        expr: Box::new(expr),
        negated: false,
        spelling: NullTestSpelling::Is,
        meta: meta(0, 0),
    }
}

fn int(start: u32, end: u32) -> Expr {
    Expr::Literal {
        literal: Literal {
            kind: LiteralKind::Integer,
            meta: meta(start, end),
        },
        meta: meta(start, end),
    }
}

fn select_one(id: u32) -> Select {
    Select {
        distinct: None,
        straight_join: false,
        projection: thin_vec![SelectItem::Expr {
            expr: col(id),
            alias: None,
            alias_spelling: AliasSpelling::As,
            meta: meta(0, 0),
        }],
        into: None,
        from: ThinVec::new(),
        lateral_views: ThinVec::new(),
        connect_by: None,
        selection: None,
        group_by: ThinVec::new(),
        group_by_quantifier: None,
        group_by_all: None,
        having: None,
        windows: ThinVec::new(),
        qualify: None,
        sample: None,
        spelling: SelectSpelling::Select,
        meta: meta(0, 0),
    }
}

fn query_one(id: u32) -> Query {
    Query {
        with: None,
        body: SetExpr::Select {
            select: Box::new(select_one(id)),
            meta: meta(0, 0),
        },
        order_by: ThinVec::new(),
        order_by_all: None,
        limit_by: None,
        limit: None,
        settings: ThinVec::new(),
        format: None,
        locking: ThinVec::new(),
        pipe_operators: ThinVec::new(),
        for_clause: None,
        meta: meta(0, 0),
    }
}

fn set_select_one(id: u32) -> SetExpr {
    SetExpr::Select {
        select: Box::new(select_one(id)),
        meta: meta(0, 0),
    }
}

fn set_op(op: SetOperator, left: SetExpr, right: SetExpr) -> SetExpr {
    SetExpr::SetOperation {
        op,
        all: false,
        by_name: false,
        left: Box::new(left),
        right: Box::new(right),
        meta: meta(0, 0),
    }
}

// --- precedence / parenthesization ----------------------------------------

#[test]
fn canonical_uses_minimal_parens() {
    // `a + b * c`: `*` binds tighter, so no parens are needed.
    assert_eq!(
        canon(&bin(
            col(1),
            BinaryOperator::Plus,
            bin(col(2), BinaryOperator::Multiply, col(3))
        )),
        "a + b * c"
    );
    // `(a + b) * c`: the looser-binding left child must be parenthesized.
    assert_eq!(
        canon(&bin(
            bin(col(1), BinaryOperator::Plus, col(2)),
            BinaryOperator::Multiply,
            col(3)
        )),
        "(a + b) * c"
    );
}

#[test]
fn canonical_respects_associativity() {
    // Left-associative `-`: a left child of equal precedence needs no parens...
    assert_eq!(
        canon(&bin(
            bin(col(1), BinaryOperator::Minus, col(2)),
            BinaryOperator::Minus,
            col(3)
        )),
        "a - b - c"
    );
    // ...but an equal-precedence right child does, to preserve grouping.
    assert_eq!(
        canon(&bin(
            col(1),
            BinaryOperator::Minus,
            bin(col(2), BinaryOperator::Minus, col(3))
        )),
        "a - (b - c)"
    );
    // Non-associative comparison parenthesizes either equal-precedence side.
    assert_eq!(
        canon(&bin(
            bin(col(1), BinaryOperator::Lt, col(2)),
            BinaryOperator::Eq(EqualsSpelling::Single),
            col(3)
        )),
        "(a < b) = c"
    );
    assert_eq!(
        canon(&bin(
            col(1),
            BinaryOperator::Eq(EqualsSpelling::Single),
            bin(col(2), BinaryOperator::Lt, col(3))
        )),
        "a = (b < c)"
    );
}

#[test]
fn canonical_reads_target_associativity_for_comparisons() {
    let left_nested = bin(
        bin(col(1), BinaryOperator::Lt, col(2)),
        BinaryOperator::Lt,
        col(3),
    );
    let right_nested = bin(
        col(1),
        BinaryOperator::Lt,
        bin(col(2), BinaryOperator::Lt, col(3)),
    );
    let render_left_assoc = |expr: &Expr| {
        render_config(
            expr,
            RenderConfig {
                target: LEFT_ASSOC_COMPARISON_FEATURES,
                ..RenderConfig::default()
            },
        )
    };

    assert_eq!(canon(&left_nested), "(a < b) < c");
    assert_eq!(render_left_assoc(&left_nested), "a < b < c");
    assert_eq!(render_left_assoc(&right_nested), "a < (b < c)");
}

#[test]
fn canonical_ranks_string_concat_between_additive_and_comparison() {
    // PostgreSQL rank: `||` is looser than `+`, so `a || b + c` keeps no parens.
    assert_eq!(
        canon(&bin(
            col(1),
            BinaryOperator::StringConcat,
            bin(col(2), BinaryOperator::Plus, col(3))
        )),
        "a || b + c"
    );
    // But `||` under `+` must be parenthesized to keep the explicit grouping.
    assert_eq!(
        canon(&bin(
            bin(col(1), BinaryOperator::StringConcat, col(2)),
            BinaryOperator::Plus,
            col(3)
        )),
        "(a || b) + c"
    );
}

#[test]
fn parenthesized_mode_is_the_precedence_oracle() {
    // The two distinct trees that both linearize to `a + b * c` in canonical
    // form become visibly different groupings under full parenthesization.
    let right_heavy = bin(
        col(1),
        BinaryOperator::Plus,
        bin(col(2), BinaryOperator::Multiply, col(3)),
    );
    let left_heavy = bin(
        bin(col(1), BinaryOperator::Plus, col(2)),
        BinaryOperator::Multiply,
        col(3),
    );

    assert_eq!(paren(&right_heavy), "(a + (b * c))");
    assert_eq!(paren(&left_heavy), "((a + b) * c)");
    assert_ne!(paren(&right_heavy), paren(&left_heavy));
}

#[test]
fn unary_parenthesization_follows_binding_powers() {
    // `NOT` (30) binds looser than `=` (40): no parens needed.
    assert_eq!(
        canon(&un(
            UnaryOperator::Not,
            bin(col(1), BinaryOperator::Eq(EqualsSpelling::Single), col(2))
        )),
        "NOT a = b"
    );
    // ...but tighter than `AND` (20): the operand must be parenthesized.
    assert_eq!(
        canon(&un(
            UnaryOperator::Not,
            bin(col(1), BinaryOperator::And, col(2))
        )),
        "NOT (a AND b)"
    );
    // Prefix `-` (80) over an additive operand needs parens.
    assert_eq!(canon(&un(UnaryOperator::Minus, col(1))), "-a");
    assert_eq!(
        canon(&un(
            UnaryOperator::Minus,
            bin(col(1), BinaryOperator::Plus, col(2))
        )),
        "-(a + b)"
    );
    // A prefix operator as the left operand of a tighter binary operator.
    assert_eq!(
        canon(&bin(
            un(UnaryOperator::Not, col(1)),
            BinaryOperator::Multiply,
            col(2)
        )),
        "(NOT a) * b"
    );
    assert_eq!(
        canon(&bin(
            un(UnaryOperator::Minus, col(1)),
            BinaryOperator::Multiply,
            col(2)
        )),
        "-a * b"
    );
    // Nested signs are parenthesized so the spelling never produces `--`.
    assert_eq!(
        canon(&un(UnaryOperator::Minus, un(UnaryOperator::Minus, col(1)))),
        "-(-a)"
    );
    // Full-parens mode wraps the unary node too.
    assert_eq!(paren(&un(UnaryOperator::Not, col(1))), "(NOT a)");
}

// --- literals and identifiers ---------------------------------------------

#[test]
fn literal_renders_source_slice_verbatim() {
    let empty = VecResolver(Vec::new());
    // Hex spelling and string quotes survive exactly, byte-for-byte.
    assert_eq!(
        render_with(&int(0, 4), &empty, "0xFF", RenderMode::Canonical),
        "0xFF"
    );

    let string: Expr = Expr::Literal {
        literal: Literal {
            kind: LiteralKind::String,
            meta: meta(0, 4),
        },
        meta: meta(0, 4),
    };
    assert_eq!(
        render_with(&string, &empty, "'hi'", RenderMode::Canonical),
        "'hi'"
    );
}

#[test]
fn redacted_masks_identifier_and_literal_keeping_structure() {
    let resolver = VecResolver(vec!["a"]);
    // `a = 42` over source where the literal spans bytes 4..6.
    let expr = bin(
        col(1),
        BinaryOperator::Eq(EqualsSpelling::Single),
        int(4, 6),
    );

    assert_eq!(
        render_with(&expr, &resolver, "a = 42", RenderMode::Canonical),
        "a = 42"
    );
    // The column name and the literal are both masked, yet the `=` operator and
    // its spacing — the predicate shape — survive (ADR-0010).
    assert_eq!(
        render_with(&expr, &resolver, "a = 42", RenderMode::Redacted),
        "id = ?"
    );
}

#[test]
fn redacted_masks_identifier_for_every_quote_style() {
    // A spelling that cannot be confused with the `id` mask token, so the
    // exact-match check doubles as a proof the source text never leaks.
    let resolver = VecResolver(vec!["secret"]);

    for quote in [
        QuoteStyle::None,
        QuoteStyle::Double,
        QuoteStyle::UnicodeDouble,
        QuoteStyle::Backtick,
        QuoteStyle::Bracket,
    ] {
        let rendered = render_with(
            &ident(1, quote.clone()),
            &resolver,
            "",
            RenderMode::Redacted,
        );
        // The mask drops the delimiters too: no quote chars and no source text.
        assert_eq!(
            rendered, "id",
            "{quote:?} identifier should mask to the bare token"
        );
        assert!(
            !rendered.contains("secret"),
            "{quote:?} identifier leaked its name"
        );
    }
}

#[test]
fn redacted_masks_qualified_name_preserving_arity() {
    let resolver = VecResolver(vec!["secret_schema", "secret_table", "secret_column"]);
    let qualified = ObjectName(thin_vec![
        ident(1, QuoteStyle::None),
        ident(2, QuoteStyle::Double),
        ident(3, QuoteStyle::None),
    ]);

    let rendered = render_with(&qualified, &resolver, "", RenderMode::Redacted);
    // Each dotted part masks independently, so the three-part arity (the query
    // shape) survives while none of the part names do.
    assert_eq!(rendered, "id.id.id");
    for name in ["secret_schema", "secret_table", "secret_column"] {
        assert!(!rendered.contains(name), "qualified part {name} leaked");
    }
}

#[test]
fn redacted_masks_keyword_used_as_identifier() {
    // A keyword spelled as an identifier is just an `Ident`, so masking covers it
    // whether or not it was quoted.
    let resolver = VecResolver(vec!["select", "from"]);
    let bare = render_with(
        &ident(1, QuoteStyle::None),
        &resolver,
        "",
        RenderMode::Redacted,
    );
    let quoted = render_with(
        &ident(2, QuoteStyle::Double),
        &resolver,
        "",
        RenderMode::Redacted,
    );

    assert_eq!(bare, "id");
    assert_eq!(quoted, "id");
    assert!(!bare.contains("select"));
    assert!(!quoted.contains("from"));
}

#[test]
fn redacted_fingerprint_is_stable_across_content_differences() {
    // Fingerprint stability (ADR-0010/0015): nodes differing only in masked
    // *content* — identifier spelling, identifier quoting, or literal value — must
    // render to the *same* redacted string. Proven at the render layer so the
    // guarantee holds independently of the parser, which cannot yet produce quoted
    // identifiers for the end-to-end conformance suite.
    let resolver = VecResolver(vec!["alpha", "beta"]);

    // `alpha = 1` and `beta = 999` differ only in name and literal value (distinct
    // symbols, distinct source slices), so both collapse to `id = ?`.
    let lhs = bin(
        col(1),
        BinaryOperator::Eq(EqualsSpelling::Single),
        int(0, 1),
    );
    let rhs = bin(
        col(2),
        BinaryOperator::Eq(EqualsSpelling::Single),
        int(0, 3),
    );
    assert_eq!(
        render_with(&lhs, &resolver, "1", RenderMode::Redacted),
        render_with(&rhs, &resolver, "999", RenderMode::Redacted),
    );
    assert_eq!(
        render_with(&lhs, &resolver, "1", RenderMode::Redacted),
        "id = ?"
    );

    // An unquoted name and a differently quoted, differently spelled name both mask
    // to the bare `id` token: quoting never reaches the fingerprint.
    assert_eq!(
        render_with(
            &ident(1, QuoteStyle::None),
            &resolver,
            "",
            RenderMode::Redacted
        ),
        render_with(
            &ident(2, QuoteStyle::Double),
            &resolver,
            "",
            RenderMode::Redacted
        ),
    );
}

#[test]
fn redacted_fingerprint_distinguishes_query_shape() {
    // The mirror of stability: differences the fingerprint must *keep* so it does
    // not over-collapse — operator, list arity, and qualified-name depth — render
    // to distinct redacted strings (ADR-0010/0015).
    let resolver = r_abc();

    // Operator: `id = id` vs `id < id`.
    assert_ne!(
        render_with(
            &bin(col(1), BinaryOperator::Eq(EqualsSpelling::Single), col(2)),
            &resolver,
            "",
            RenderMode::Redacted,
        ),
        render_with(
            &bin(col(1), BinaryOperator::Lt, col(2)),
            &resolver,
            "",
            RenderMode::Redacted,
        ),
    );

    // List arity: `id IN (id)` vs `id IN (id, id)`.
    let in_one = Expr::InList {
        expr: Box::new(col(1)),
        list: thin_vec![col(2)],
        negated: false,
        meta: meta(0, 0),
    };
    let in_two = Expr::InList {
        expr: Box::new(col(1)),
        list: thin_vec![col(2), col(3)],
        negated: false,
        meta: meta(0, 0),
    };
    assert_ne!(
        render_with(&in_one, &resolver, "", RenderMode::Redacted),
        render_with(&in_two, &resolver, "", RenderMode::Redacted),
    );

    // Qualified-name depth: `id` vs `id.id`.
    let qualified = ObjectName(thin_vec![
        ident(1, QuoteStyle::None),
        ident(2, QuoteStyle::None)
    ]);
    assert_ne!(
        render_with(&name(1), &resolver, "", RenderMode::Redacted),
        render_with(&qualified, &resolver, "", RenderMode::Redacted),
    );
}

#[test]
fn quoted_identifiers_round_trip_their_delimiters() {
    let resolver = VecResolver(vec!["user"]);
    let render = |quote| render_with(&ident(1, quote), &resolver, "", RenderMode::Canonical);

    assert_eq!(render(QuoteStyle::None), "user");
    assert_eq!(render(QuoteStyle::Double), "\"user\"");
    assert_eq!(render(QuoteStyle::Backtick), "`user`");
    assert_eq!(render(QuoteStyle::Bracket), "[user]");
}

#[test]
fn quoted_identifiers_double_embedded_close_delimiter() {
    // Each entry embeds its style's close delimiter; the bracket entry also embeds
    // an open `[` that must survive undoubled (asymmetric escape).
    let resolver = VecResolver(vec!["a\"b", "a`b", "a][b"]);
    let render = |id, quote| render_with(&ident(id, quote), &resolver, "", RenderMode::Canonical);

    assert_eq!(render(1, QuoteStyle::Double), "\"a\"\"b\"");
    assert_eq!(render(2, QuoteStyle::Backtick), "`a``b`");
    assert_eq!(render(3, QuoteStyle::Bracket), "[a]][b]");
}

#[test]
fn object_name_is_dot_joined() {
    let resolver = VecResolver(vec!["schema", "table"]);
    let qualified = ObjectName(thin_vec![
        ident(1, QuoteStyle::None),
        ident(2, QuoteStyle::None)
    ]);
    assert_eq!(
        render_with(&qualified, &resolver, "", RenderMode::Canonical),
        "schema.table"
    );
}

#[test]
fn foreign_key_ref_renders_referential_actions() {
    // A bare reference renders no trailing clauses.
    let bare = ForeignKeyRef {
        table: name(1),
        columns: ThinVec::new(),
        match_type: None,
        on_delete: None,
        on_update: None,
        update_before_delete: false,
        meta: meta(0, 0),
    };
    assert_eq!(canon(&bare), "REFERENCES a");

    // Every clause renders in canonical MATCH -> ON DELETE -> ON UPDATE order, and
    // the `SET NULL` column list is emitted.
    let full = ForeignKeyRef {
        table: name(1),
        columns: thin_vec![ident(2, QuoteStyle::None)],
        match_type: Some(ForeignKeyMatch::Full),
        on_delete: Some(Box::new(ReferentialAction::SetNull {
            columns: thin_vec![ident(1, QuoteStyle::None), ident(2, QuoteStyle::None)],
            meta: meta(0, 0),
        })),
        on_update: Some(Box::new(ReferentialAction::Cascade { meta: meta(0, 0) })),
        update_before_delete: false,
        meta: meta(0, 0),
    };
    assert_eq!(
        canon(&full),
        "REFERENCES a (b) MATCH FULL ON DELETE SET NULL (a, b) ON UPDATE CASCADE"
    );
}

// --- larger expressions ----------------------------------------------------

#[test]
fn functions_and_casts_render() {
    let resolver = VecResolver(vec!["f", "a", "b"]);
    let call = Expr::Function {
        call: Box::new(FunctionCall {
            name: name(1),
            quantifier: None,
            args: thin_vec![pos_arg(col(2)), pos_arg(col(3))],
            wildcard: false,
            order_by: ThinVec::new(),
            separator: None,
            within_group: None,
            filter: None,
            filter_where: FilterWhereSpelling::Where,
            over: None,
            null_treatment: None,
            window_tail: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&call, &resolver, "", RenderMode::Canonical),
        "f(a, b)"
    );

    let cast = Expr::Cast {
        expr: Box::new(col(2)),
        data_type: Box::new(DataType::Character {
            spelling: CharacterTypeName::Varchar,
            size: Some(255),
            charset: None,
            meta: meta(0, 0),
        }),
        syntax: CastSyntax::Call,
        try_cast: false,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&cast, &resolver, "", RenderMode::Canonical),
        "CAST(a AS VARCHAR(255))"
    );
}

#[test]
fn convert_forms_render() {
    let resolver = VecResolver(vec!["a", "utf8mb4"]);
    // MySQL's comma-form cast folds onto the one `Expr::Cast` shape, tagged `Convert`.
    let convert_cast = Expr::Cast {
        expr: Box::new(col(1)),
        data_type: Box::new(DataType::Character {
            spelling: CharacterTypeName::Varchar,
            size: Some(255),
            charset: None,
            meta: meta(0, 0),
        }),
        syntax: CastSyntax::Convert,
        try_cast: false,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&convert_cast, &resolver, "", RenderMode::Canonical),
        "CONVERT(a, VARCHAR(255))"
    );
    // The transcoding `USING` form is the distinct `StringFunc::ConvertUsing`.
    let convert_using = Expr::StringFunc {
        string_func: Box::new(StringFunc::ConvertUsing {
            expr: Box::new(col(1)),
            charset: ident(2, QuoteStyle::None),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&convert_using, &resolver, "", RenderMode::Canonical),
        "CONVERT(a USING utf8mb4)"
    );
}

#[test]
fn double_colon_cast_renders_with_syntax_tag_and_minimal_parens() {
    let int_ty = || {
        Box::new(DataType::Integer {
            spelling: IntegerTypeName::Int,
            display_width: None,
            meta: meta(0, 0),
        })
    };
    let double_colon = |expr: Expr| Expr::Cast {
        expr: Box::new(expr),
        data_type: int_ty(),
        syntax: CastSyntax::DoubleColon,
        try_cast: false,
        meta: meta(0, 0),
    };

    // An atom operand needs no parens; a looser-binding operand is parenthesized.
    assert_eq!(canon(&double_colon(col(1))), "a::INT");
    assert_eq!(
        canon(&double_colon(bin(col(1), BinaryOperator::Plus, col(2)))),
        "(a + b)::INT",
    );

    // The same construct with the call spelling keeps the standard CAST(...).
    let call = Expr::Cast {
        expr: Box::new(col(1)),
        data_type: int_ty(),
        syntax: CastSyntax::Call,
        try_cast: false,
        meta: meta(0, 0),
    };
    assert_eq!(canon(&call), "CAST(a AS INT)");

    // The `try_cast` flag selects the `TRY_CAST(` lead on the same call shape.
    let try_call = Expr::Cast {
        expr: Box::new(col(1)),
        data_type: int_ty(),
        syntax: CastSyntax::Call,
        try_cast: true,
        meta: meta(0, 0),
    };
    assert_eq!(canon(&try_call), "TRY_CAST(a AS INT)");
}

#[test]
fn duckdb_composite_and_array_types_render() {
    use crate::ast::{ArrayTypeSpelling, Ident, QuoteStyle, StructTypeField, StructTypeSpelling};
    let resolver = VecResolver(vec!["x", "y"]);
    // `VecResolver` indexes by `Symbol::index()` (0-based), so symbol `n` resolves to
    // `self.0[n - 1]`: symbol 1 -> "x", symbol 2 -> "y".
    let sym = |id| Symbol::new(id).expect("non-zero symbol");
    let int = || -> DataType {
        DataType::Integer {
            spelling: IntegerTypeName::Int,
            display_width: None,
            meta: meta(0, 0),
        }
    };
    let field = |name_sym, ty| StructTypeField {
        name: Ident {
            sym: name_sym,
            quote: QuoteStyle::None,
            meta: meta(0, 0),
        },
        ty,
        meta: meta(0, 0),
    };

    // STRUCT vs ROW spelling of the same field-list shape.
    let strukt = DataType::Struct {
        fields: thin_vec::thin_vec![field(sym(1), int())],
        spelling: StructTypeSpelling::Struct,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&strukt, &resolver, "", RenderMode::Canonical),
        "STRUCT(x INT)"
    );
    let row = DataType::Struct {
        fields: thin_vec::thin_vec![field(sym(1), int()), field(sym(2), int())],
        spelling: StructTypeSpelling::Row,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&row, &resolver, "", RenderMode::Canonical),
        "ROW(x INT, y INT)"
    );

    // MAP(K, V).
    let map = DataType::Map {
        key: Box::new(int()),
        value: Box::new(int()),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&map, &resolver, "", RenderMode::Canonical),
        "MAP(INT, INT)"
    );

    // Array-suffix surfaces: bracket vs keyword, sized vs unsized.
    let arr = |size, spelling| DataType::Array {
        element: Box::new(int()),
        size,
        spelling,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(
            &arr(None, ArrayTypeSpelling::Bracket),
            &resolver,
            "",
            RenderMode::Canonical
        ),
        "INT[]"
    );
    assert_eq!(
        render_with(
            &arr(Some(3), ArrayTypeSpelling::Bracket),
            &resolver,
            "",
            RenderMode::Canonical
        ),
        "INT[3]"
    );
    assert_eq!(
        render_with(
            &arr(None, ArrayTypeSpelling::Keyword),
            &resolver,
            "",
            RenderMode::Canonical
        ),
        "INT ARRAY"
    );
    assert_eq!(
        render_with(
            &arr(Some(3), ArrayTypeSpelling::Keyword),
            &resolver,
            "",
            RenderMode::Canonical
        ),
        "INT ARRAY[3]"
    );
}

#[test]
fn clickhouse_wrapped_types_render() {
    use crate::ast::{ArrayTypeSpelling, DecimalTypeName, WrappedTypeKind};
    let resolver = VecResolver(vec![]);
    let int = || -> DataType {
        DataType::Integer {
            spelling: IntegerTypeName::Int,
            display_width: None,
            meta: meta(0, 0),
        }
    };
    let nullable = |inner| DataType::Wrapped {
        kind: WrappedTypeKind::Nullable,
        inner: Box::new(inner),
        meta: meta(0, 0),
    };

    // The wrapper renders its ClickHouse mixed-case keyword and parenthesized inner type.
    assert_eq!(
        render_with(&nullable(int()), &resolver, "", RenderMode::Canonical),
        "Nullable(INT)"
    );

    // The inner type recurses, so a parametrized inner round-trips inside the wrapper.
    let dec = DataType::Decimal {
        spelling: DecimalTypeName::Decimal,
        precision: Some(10),
        scale: Some(2),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&nullable(dec), &resolver, "", RenderMode::Canonical),
        "Nullable(DECIMAL(10, 2))"
    );

    // An array *of* a wrapped type composes — the array suffix wraps the rendered inner.
    let arr_of_nullable = DataType::Array {
        element: Box::new(nullable(int())),
        size: None,
        spelling: ArrayTypeSpelling::Bracket,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&arr_of_nullable, &resolver, "", RenderMode::Canonical),
        "Nullable(INT)[]"
    );

    // The `LowCardinality` sibling shares the wrapper shape and renders its own mixed-case
    // keyword; the canonical `LowCardinality(Nullable(T))` composition nests one wrapper
    // inside the other with no special-casing.
    let low_card = |inner| DataType::Wrapped {
        kind: WrappedTypeKind::LowCardinality,
        inner: Box::new(inner),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&low_card(int()), &resolver, "", RenderMode::Canonical),
        "LowCardinality(INT)"
    );
    assert_eq!(
        render_with(
            &low_card(nullable(int())),
            &resolver,
            "",
            RenderMode::Canonical
        ),
        "LowCardinality(Nullable(INT))"
    );
}

#[test]
fn prefix_typed_literal_renders_type_then_string_constant() {
    // `type 'string'` renders the type name ahead of its string constant, the third
    // spelling of the one canonical cast (ADR-0011). Source bytes 4..8 are the string
    // constant `'42'`, which renders from its span verbatim.
    let empty = VecResolver(Vec::new());
    let typed: Expr = Expr::Cast {
        expr: Box::new(Expr::Literal {
            literal: Literal {
                kind: LiteralKind::String,
                meta: meta(4, 8),
            },
            meta: meta(4, 8),
        }),
        data_type: Box::new(DataType::Integer {
            spelling: IntegerTypeName::Int,
            display_width: None,
            meta: meta(0, 0),
        }),
        syntax: CastSyntax::PrefixTyped,
        try_cast: false,
        meta: meta(0, 8),
    };
    assert_eq!(
        render_with(&typed, &empty, "INT '42'", RenderMode::Canonical),
        "INT '42'"
    );
    // It is a primary, so the Parenthesized oracle mode adds no self-wrapping — the
    // typed constant round-trips identically in both modes.
    assert_eq!(
        render_with(&typed, &empty, "INT '42'", RenderMode::Parenthesized),
        "INT '42'"
    );
}

#[test]
fn postgres_postfix_and_constructor_expressions_render() {
    let index = Expr::Subscript {
        subscript: Box::new(SubscriptExpr {
            base: col(1),
            lower: Some(col(2)),
            upper: None,
            step: None,
            kind: SubscriptKind::Index,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&index), "a[b]");

    let slice = Expr::Subscript {
        subscript: Box::new(SubscriptExpr {
            base: col(1),
            lower: Some(col(2)),
            upper: Some(col(3)),
            step: None,
            kind: SubscriptKind::Slice,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&slice), "a[b:c]");

    // A three-bound stepped slice with the `-` open-upper placeholder round-trips.
    let stepped = Expr::Subscript {
        subscript: Box::new(SubscriptExpr {
            base: col(1),
            lower: Some(col(2)),
            upper: None,
            step: Some(col(3)),
            kind: SubscriptKind::SliceWithStep,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&stepped), "a[b:-:c]");

    let collate = Expr::Collate {
        collate: Box::new(CollateExpr {
            expr: col(1),
            collation: name(2),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&collate), "a COLLATE b");

    let at_tz = Expr::AtTimeZone {
        at_time_zone: Box::new(AtTimeZoneExpr {
            expr: col(1),
            zone: col(2),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&at_tz), "a AT TIME ZONE b");

    let array = Expr::Array {
        array: Box::new(ArrayExpr::Elements {
            elements: thin_vec![col(1), col(2)],
            spelling: ArraySpelling::Keyword,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&array), "ARRAY[a, b]");

    let list = Expr::Array {
        array: Box::new(ArrayExpr::Elements {
            elements: thin_vec![col(1), col(2)],
            spelling: ArraySpelling::Bracket,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&list), "[a, b]");

    let struct_literal = Expr::Struct {
        r#struct: Box::new(StructExpr {
            fields: thin_vec![
                StructField {
                    key: sym(1),
                    key_spelling: StructKeySpelling::SingleQuoted,
                    value: col(2),
                    meta: meta(0, 0),
                },
                StructField {
                    key: sym(2),
                    key_spelling: StructKeySpelling::Bare,
                    value: col(1),
                    meta: meta(0, 0),
                },
                StructField {
                    key: sym(1),
                    key_spelling: StructKeySpelling::DoubleQuoted,
                    value: col(2),
                    meta: meta(0, 0),
                },
            ],
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&struct_literal), "{'a': b, b: a, \"a\": b}");

    let map_literal = Expr::Map {
        map: Box::new(MapExpr {
            entries: thin_vec![MapEntry {
                key: col(1),
                value: col(2),
                meta: meta(0, 0),
            }],
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&map_literal), "MAP {a: b}");

    let empty_map: Expr = Expr::Map {
        map: Box::new(MapExpr {
            entries: thin_vec![],
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&empty_map), "MAP {}");

    let explicit_row = Expr::Row {
        row: Box::new(RowExpr {
            fields: thin_vec![col(1), col(2)],
            explicit: true,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&explicit_row), "ROW(a, b)");

    let implicit_row = Expr::Row {
        row: Box::new(RowExpr {
            fields: thin_vec![col(1), col(2)],
            explicit: false,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&implicit_row), "(a, b)");

    // The field-selection base is always parenthesized so `(a).b` cannot re-parse
    // as the qualified column `a.b`.
    let field = Expr::FieldSelection {
        field_selection: Box::new(FieldSelectionExpr {
            base: col(1),
            selector: FieldSelector::Field {
                field: ident(2, QuoteStyle::None),
                meta: meta(0, 0),
            },
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&field), "(a).b");

    // The `.*` star selector renders the whole-row wildcard, keeping the parens so a
    // value-position `tbl.*` round-trips to the same node.
    let star = Expr::FieldSelection {
        field_selection: Box::new(FieldSelectionExpr {
            base: col(1),
            selector: FieldSelector::Star { meta: meta(0, 0) },
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&star), "(a).*");
}

#[test]
fn function_call_modifiers_render() {
    let resolver = VecResolver(vec!["count", "x", "y", "z"]);

    // An ordered-set, filtered, distinct aggregate.
    let agg = Expr::Function {
        call: Box::new(FunctionCall {
            name: name(1),
            quantifier: Some(SetQuantifier::Distinct),
            args: thin_vec![pos_arg(col(2))],
            wildcard: false,
            order_by: thin_vec![OrderByExpr {
                expr: col(3),
                asc: Some(false),
                using: None,
                nulls_first: None,
                meta: meta(0, 0),
            }],
            separator: None,
            within_group: None,
            filter: Some(Box::new(col(4))),
            filter_where: FilterWhereSpelling::Where,
            over: None,
            null_treatment: None,
            window_tail: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&agg, &resolver, "", RenderMode::Canonical),
        "count(DISTINCT x ORDER BY y DESC) FILTER (WHERE z)"
    );

    // A `*` argument list renders as the star, not an empty argument list.
    let star: Expr = Expr::Function {
        call: Box::new(FunctionCall {
            name: name(1),
            quantifier: None,
            args: ThinVec::new(),
            wildcard: true,
            order_by: ThinVec::new(),
            separator: None,
            within_group: None,
            filter: None,
            filter_where: FilterWhereSpelling::Where,
            over: None,
            null_treatment: None,
            window_tail: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&star, &resolver, "", RenderMode::Canonical),
        "count(*)"
    );

    // A WITHIN GROUP ordered-set aggregate renders the clause after the `)` and, when
    // combined, before FILTER — matching PostgreSQL's grammar order.
    let ordered_set = Expr::Function {
        call: Box::new(FunctionCall {
            name: name(1),
            quantifier: None,
            args: thin_vec![pos_arg(col(2))],
            wildcard: false,
            order_by: ThinVec::new(),
            separator: None,
            within_group: Some(thin_vec![OrderByExpr {
                expr: col(3),
                asc: None,
                using: None,
                nulls_first: None,
                meta: meta(0, 0),
            }]),
            filter: Some(Box::new(col(4))),
            filter_where: FilterWhereSpelling::Where,
            over: None,
            null_treatment: None,
            window_tail: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&ordered_set, &resolver, "", RenderMode::Canonical),
        "count(x) WITHIN GROUP (ORDER BY y) FILTER (WHERE z)"
    );
}

#[test]
fn case_expressions_render() {
    let resolver = VecResolver(vec!["a", "b", "c"]);

    // Searched form with an ELSE.
    let searched: Expr = Expr::Case {
        case: Box::new(CaseExpr {
            operand: None,
            when_clauses: thin_vec![WhenClause {
                condition: col(1),
                result: col(2),
                meta: meta(0, 0),
            }],
            else_result: Some(Box::new(col(3))),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&searched, &resolver, "", RenderMode::Canonical),
        "CASE WHEN a THEN b ELSE c END"
    );

    // Simple form with a compared operand and no ELSE.
    let simple: Expr = Expr::Case {
        case: Box::new(CaseExpr {
            operand: Some(Box::new(col(1))),
            when_clauses: thin_vec![WhenClause {
                condition: col(2),
                result: col(3),
                meta: meta(0, 0),
            }],
            else_result: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&simple, &resolver, "", RenderMode::Canonical),
        "CASE a WHEN b THEN c END"
    );
}

#[test]
fn extract_expression_renders() {
    let resolver = VecResolver(vec!["year", "a"]);
    let extract: Expr = Expr::Extract {
        extract: Box::new(ExtractExpr {
            field: ident(1, QuoteStyle::None),
            source: Box::new(col(2)),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&extract, &resolver, "", RenderMode::Canonical),
        "EXTRACT(year FROM a)"
    );
}

#[test]
fn data_types_preserve_canonical_spelling_and_render_target_forms() {
    let character_varying: DataType = DataType::Character {
        spelling: CharacterTypeName::CharacterVarying,
        size: Some(12),
        charset: None,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_config(&character_varying, RenderConfig::default()),
        "CHARACTER VARYING(12)"
    );
    assert_eq!(
        render_target(&character_varying, FeatureSet::ANSI),
        "CHARACTER VARYING(12)"
    );
    assert_eq!(
        render_target(&character_varying, FeatureSet::POSTGRES),
        "VARCHAR(12)"
    );

    let timestamp_with_time_zone: DataType = DataType::Timestamp {
        spelling: TimestampTypeName::Timestamp,
        precision: Some(3),
        time_zone: TimeZone::WithTimeZone,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_config(&timestamp_with_time_zone, RenderConfig::default()),
        "TIMESTAMP(3) WITH TIME ZONE"
    );
    assert_eq!(
        render_target(&timestamp_with_time_zone, FeatureSet::ANSI),
        "TIMESTAMP(3) WITH TIME ZONE"
    );
    assert_eq!(
        render_target(&timestamp_with_time_zone, FeatureSet::POSTGRES),
        "TIMESTAMPTZ(3)"
    );

    let timetz: DataType = DataType::Time {
        spelling: TimeTypeName::Timetz,
        precision: Some(6),
        time_zone: TimeZone::WithTimeZone,
        meta: meta(0, 0),
    };
    assert_eq!(render_config(&timetz, RenderConfig::default()), "TIMETZ(6)");
    assert_eq!(
        render_target(&timetz, FeatureSet::ANSI),
        "TIME(6) WITH TIME ZONE"
    );
    assert_eq!(render_target(&timetz, FeatureSet::POSTGRES), "TIMETZ(6)");
}

#[test]
fn mysql_data_types_render_their_source_and_target_forms() {
    // Scalar integer widths render under their own name in every target.
    assert_eq!(
        render_config(
            &DataType::<NoExt>::TinyInt {
                display_width: None,
                meta: meta(0, 0),
            },
            RenderConfig::default()
        ),
        "TINYINT"
    );
    assert_eq!(
        render_config(
            &DataType::<NoExt>::MediumInt {
                display_width: None,
                meta: meta(0, 0)
            },
            RenderConfig::default()
        ),
        "MEDIUMINT"
    );

    // The integer display width `(M)` renders as a parenthesized suffix on the type
    // name (MySQL `INT(11)` / `TINYINT(1)` / `BIGINT(20)`), and survives a
    // target-dialect render since it is part of the type's written surface.
    let int_11: DataType = DataType::Integer {
        spelling: IntegerTypeName::Int,
        display_width: Some(11),
        meta: meta(0, 0),
    };
    assert_eq!(render_config(&int_11, RenderConfig::default()), "INT(11)");
    assert_eq!(render_target(&int_11, FeatureSet::ANSI), "INTEGER(11)");
    assert_eq!(
        render_config(
            &DataType::<NoExt>::TinyInt {
                display_width: Some(1),
                meta: meta(0, 0),
            },
            RenderConfig::default()
        ),
        "TINYINT(1)"
    );
    assert_eq!(
        render_config(
            &DataType::<NoExt>::BigInt {
                display_width: Some(20),
                meta: meta(0, 0),
            },
            RenderConfig::default()
        ),
        "BIGINT(20)"
    );

    // The character-LOB size family preserves its source spelling, and collapses to
    // the portable `TEXT` for another target.
    let medium_text: DataType = DataType::Text {
        spelling: TextTypeName::MediumText,
        charset: None,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_config(&medium_text, RenderConfig::default()),
        "MEDIUMTEXT"
    );
    assert_eq!(render_target(&medium_text, FeatureSet::ANSI), "TEXT");
    assert_eq!(render_target(&medium_text, FeatureSet::POSTGRES), "TEXT");

    // The binary-LOB family preserves its source spelling; PostgreSQL targets `bytea`.
    let long_blob: DataType = DataType::Blob {
        spelling: BlobTypeName::LongBlob,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_config(&long_blob, RenderConfig::default()),
        "LONGBLOB"
    );
    assert_eq!(render_target(&long_blob, FeatureSet::POSTGRES), "BYTEA");

    // `DATETIME` reuses the timestamp shape; a non-PreserveSource target spells it as
    // the zone-less `TIMESTAMP`.
    let datetime: DataType = DataType::Timestamp {
        spelling: TimestampTypeName::Datetime,
        precision: Some(6),
        time_zone: TimeZone::Unspecified,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_config(&datetime, RenderConfig::default()),
        "DATETIME(6)"
    );
    assert_eq!(render_target(&datetime, FeatureSet::ANSI), "TIMESTAMP(6)");

    // Bare MySQL `DOUBLE` (distinct from the standard `DOUBLE PRECISION` spelling).
    assert_eq!(
        render_config(
            &DataType::<NoExt>::Double {
                spelling: DoubleTypeName::Double,
                meta: meta(0, 0),
            },
            RenderConfig::default()
        ),
        "DOUBLE"
    );

    // The numeric modifier wraps its inner type and space-joins the attributes.
    let modified: DataType = DataType::NumericModifier {
        element: Some(Box::new(DataType::TinyInt {
            display_width: None,
            meta: meta(0, 0),
        })),
        signedness: Signedness::Unsigned,
        zerofill: true,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_config(&modified, RenderConfig::default()),
        "TINYINT UNSIGNED ZEROFILL"
    );

    // A standalone `UNSIGNED` cast target names no base type.
    let standalone: DataType = DataType::NumericModifier {
        element: None,
        signedness: Signedness::Unsigned,
        zerofill: false,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_config(&standalone, RenderConfig::default()),
        "UNSIGNED"
    );
}

#[test]
fn parameter_placeholders_render_in_their_source_form() {
    // Positional placeholders render `$<index>`; the anonymous form renders `?`.
    // The placeholder variants carry no extension-typed field, so the `Expr`
    // (`= Expr<NoExt>`) annotation pins the otherwise-ambiguous extension parameter.
    let positional: Expr = Expr::Parameter {
        kind: ParameterKind::Positional(1),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&positional), "$1");

    let wide_index: Expr = Expr::Parameter {
        kind: ParameterKind::Positional(42),
        meta: meta(0, 0),
    };
    assert_eq!(canon(&wide_index), "$42");

    let anonymous: Expr = Expr::Parameter {
        kind: ParameterKind::Anonymous,
        meta: meta(0, 0),
    };
    assert_eq!(canon(&anonymous), "?");

    // Named placeholders render their sigil + resolved name (`r_abc`: 1 -> `a`,
    // 2 -> `b`); the sigil tag restores the colon vs at-sign spelling.
    let colon: Expr = Expr::Parameter {
        kind: ParameterKind::Named {
            name: Symbol::new(1).expect("non-zero symbol"),
            sigil: ParameterSigil::Colon,
        },
        meta: meta(0, 0),
    };
    assert_eq!(canon(&colon), ":a");

    let at: Expr = Expr::Parameter {
        kind: ParameterKind::Named {
            name: Symbol::new(2).expect("non-zero symbol"),
            sigil: ParameterSigil::At,
        },
        meta: meta(0, 0),
    };
    assert_eq!(canon(&at), "@b");
}

#[test]
fn positional_column_reference_renders_its_index_verbatim() {
    // `#n` renders as `#<index>`. The variant carries no extension-typed field, so the
    // `Expr` (`= Expr<NoExt>`) annotation pins the otherwise-ambiguous extension parameter.
    let one: Expr = Expr::PositionalColumn {
        index: 1,
        meta: meta(0, 0),
    };
    assert_eq!(canon(&one), "#1");

    let wide: Expr = Expr::PositionalColumn {
        index: 42,
        meta: meta(0, 0),
    };
    assert_eq!(canon(&wide), "#42");

    // The index is query structure, not a value, so redacted mode keeps it verbatim (a
    // positional reference is never masked the way a literal value is).
    assert_eq!(render_with(&one, &r_abc(), "", RenderMode::Redacted), "#1");
}

#[test]
fn predicate_expressions_render() {
    // IS [NOT] NULL, with a looser-binding operand parenthesized (minimal-paren
    // derivation is exercised by `predicate_operands_use_minimal_parens`).
    assert_eq!(
        canon(&Expr::IsNull {
            expr: Box::new(col(1)),
            negated: false,
            spelling: NullTestSpelling::Is,
            meta: meta(0, 0),
        }),
        "a IS NULL"
    );
    assert_eq!(
        canon(&Expr::IsNull {
            expr: Box::new(col(1)),
            negated: true,
            spelling: NullTestSpelling::Is,
            meta: meta(0, 0),
        }),
        "a IS NOT NULL"
    );
    assert_eq!(
        canon(&Expr::IsNull {
            expr: Box::new(bin(col(1), BinaryOperator::Or, col(2))),
            negated: false,
            spelling: NullTestSpelling::Is,
            meta: meta(0, 0),
        }),
        "(a OR b) IS NULL"
    );

    // IS [NOT] {TRUE|FALSE|UNKNOWN} — the truth-value tests (F571), rendered like the
    // sibling `IS NULL` postfix predicate with a looser-binding operand parenthesized.
    for (value, negated, expected) in [
        (TruthValue::True, false, "a IS TRUE"),
        (TruthValue::True, true, "a IS NOT TRUE"),
        (TruthValue::False, false, "a IS FALSE"),
        (TruthValue::False, true, "a IS NOT FALSE"),
        (TruthValue::Unknown, false, "a IS UNKNOWN"),
        (TruthValue::Unknown, true, "a IS NOT UNKNOWN"),
    ] {
        assert_eq!(
            canon(&Expr::IsTruth {
                expr: Box::new(col(1)),
                value,
                negated,
                meta: meta(0, 0),
            }),
            expected
        );
    }
    assert_eq!(
        canon(&Expr::IsTruth {
            expr: Box::new(bin(col(1), BinaryOperator::Or, col(2))),
            value: TruthValue::True,
            negated: false,
            meta: meta(0, 0),
        }),
        "(a OR b) IS TRUE"
    );

    assert_eq!(
        canon(&Expr::Between {
            expr: Box::new(col(1)),
            low: Box::new(col(2)),
            high: Box::new(col(3)),
            negated: true,
            symmetric: false,
            meta: meta(0, 0),
        }),
        "a NOT BETWEEN b AND c"
    );

    assert_eq!(
        canon(&Expr::InList {
            expr: Box::new(col(1)),
            list: thin_vec![col(2), col(3)],
            negated: false,
            meta: meta(0, 0),
        }),
        "a IN (b, c)"
    );

    assert_eq!(
        canon(&Expr::InSubquery {
            expr: Box::new(col(1)),
            subquery: Box::new(query_one(2)),
            negated: true,
            meta: meta(0, 0),
        }),
        "a NOT IN (SELECT b)"
    );

    assert_eq!(
        canon(&Expr::Exists {
            query: Box::new(query_one(1)),
            meta: meta(0, 0),
        }),
        "EXISTS (SELECT a)"
    );

    assert_eq!(
        canon(&Expr::QuantifiedComparison {
            left: Box::new(col(1)),
            op: BinaryOperator::Lt,
            quantifier: Quantifier::All,
            subquery: Box::new(query_one(2)),
            meta: meta(0, 0),
        }),
        "a < ALL (SELECT b)"
    );

    assert_eq!(
        canon(&Expr::QuantifiedComparison {
            left: Box::new(col(1)),
            op: BinaryOperator::Eq(EqualsSpelling::Single),
            quantifier: Quantifier::Some,
            subquery: Box::new(query_one(2)),
            meta: meta(0, 0),
        }),
        "a = SOME (SELECT b)"
    );

    assert_eq!(
        paren(&Expr::InSubquery {
            expr: Box::new(col(1)),
            subquery: Box::new(query_one(2)),
            negated: false,
            meta: meta(0, 0),
        }),
        "(a IN (SELECT b))"
    );

    assert_eq!(
        paren(&Expr::QuantifiedComparison {
            left: Box::new(col(1)),
            op: BinaryOperator::Eq(EqualsSpelling::Single),
            quantifier: Quantifier::Any,
            subquery: Box::new(query_one(2)),
            meta: meta(0, 0),
        }),
        "(a = ANY (SELECT b))"
    );
}

#[test]
fn predicate_operands_use_minimal_parens() {
    // Predicates sit at comparison precedence (ADR-0008): an operand is
    // parenthesized only when it binds looser than the predicate, never merely for
    // being compound. `+` binds tighter than `IS NULL`, so the operand stays bare.
    assert_eq!(
        canon(&Expr::IsNull {
            expr: Box::new(bin(col(1), BinaryOperator::Plus, col(2))),
            negated: false,
            spelling: NullTestSpelling::Is,
            meta: meta(0, 0),
        }),
        "a + b IS NULL"
    );
    // `OR` binds looser, so the necessary parens stay: without them
    // `a OR b IS NULL` would re-parse as `a OR (b IS NULL)`.
    assert_eq!(
        canon(&Expr::IsNull {
            expr: Box::new(bin(col(1), BinaryOperator::Or, col(2))),
            negated: false,
            spelling: NullTestSpelling::Is,
            meta: meta(0, 0),
        }),
        "(a OR b) IS NULL"
    );
    // A comparison operand is an equal-precedence, non-associative sibling, so it
    // is parenthesized just like a comparison child of a comparison.
    assert_eq!(
        canon(&Expr::IsNull {
            expr: Box::new(bin(
                col(1),
                BinaryOperator::Eq(EqualsSpelling::Single),
                col(2)
            )),
            negated: false,
            spelling: NullTestSpelling::Is,
            meta: meta(0, 0),
        }),
        "(a = b) IS NULL"
    );
    // A nested predicate is likewise non-associative at the same level.
    assert_eq!(
        canon(&Expr::IsNull {
            expr: Box::new(Expr::IsNull {
                expr: Box::new(col(1)),
                negated: false,
                spelling: NullTestSpelling::Is,
                meta: meta(0, 0),
            }),
            negated: false,
            spelling: NullTestSpelling::Is,
            meta: meta(0, 0),
        }),
        "(a IS NULL) IS NULL"
    );
    // The truth-value test sits at the same predicate level, so its operand parenthesizes
    // by the same rule: a tighter `+` operand stays bare, a looser `OR` operand and a
    // nested predicate (equal-precedence, non-associative) are parenthesized.
    assert_eq!(
        canon(&Expr::IsTruth {
            expr: Box::new(bin(col(1), BinaryOperator::Plus, col(2))),
            value: TruthValue::True,
            negated: false,
            meta: meta(0, 0),
        }),
        "a + b IS TRUE"
    );
    assert_eq!(
        canon(&Expr::IsTruth {
            expr: Box::new(Expr::IsTruth {
                expr: Box::new(col(1)),
                value: TruthValue::True,
                negated: false,
                meta: meta(0, 0),
            }),
            value: TruthValue::False,
            negated: true,
            meta: meta(0, 0),
        }),
        "(a IS TRUE) IS NOT FALSE"
    );
    // Prefix `-` (80) binds tighter than the predicate, so `-a` stays bare...
    assert_eq!(
        canon(&Expr::IsNull {
            expr: Box::new(un(UnaryOperator::Minus, col(1))),
            negated: false,
            spelling: NullTestSpelling::Is,
            meta: meta(0, 0),
        }),
        "-a IS NULL"
    );
    // ...but prefix `NOT` (30) binds looser, so the left operand needs parens
    // (otherwise `NOT a IS NULL` re-parses as `NOT (a IS NULL)`).
    assert_eq!(
        canon(&Expr::IsNull {
            expr: Box::new(un(UnaryOperator::Not, col(1))),
            negated: false,
            spelling: NullTestSpelling::Is,
            meta: meta(0, 0),
        }),
        "(NOT a) IS NULL"
    );
}

#[test]
fn between_and_in_operands_use_minimal_parens() {
    // The principal operand groups on the left: a tighter `+` stays bare.
    assert_eq!(
        canon(&Expr::Between {
            expr: Box::new(bin(col(1), BinaryOperator::Plus, col(2))),
            low: Box::new(col(3)),
            high: Box::new(col(1)),
            negated: false,
            symmetric: false,
            meta: meta(0, 0),
        }),
        "a + b BETWEEN c AND a"
    );
    // A `BETWEEN` bound parses on the right at comparison precedence: a looser `OR`
    // bound is parenthesized, a tighter `+` bound is not.
    assert_eq!(
        canon(&Expr::Between {
            expr: Box::new(col(1)),
            low: Box::new(bin(col(2), BinaryOperator::Plus, col(3))),
            high: Box::new(bin(col(2), BinaryOperator::Or, col(3))),
            negated: false,
            symmetric: false,
            meta: meta(0, 0),
        }),
        "a BETWEEN b + c AND (b OR c)"
    );
    // A prefix `NOT` closes its own left edge, so as a right-side bound it stays
    // bare — unlike the same operand as a left-side principal, which needs parens.
    assert_eq!(
        canon(&Expr::Between {
            expr: Box::new(col(1)),
            low: Box::new(col(2)),
            high: Box::new(un(UnaryOperator::Not, col(3))),
            negated: false,
            symmetric: false,
            meta: meta(0, 0),
        }),
        "a BETWEEN b AND NOT c"
    );
    // `IN` principal operand: a looser `OR` is parenthesized, a tighter `+` is not.
    assert_eq!(
        canon(&Expr::InList {
            expr: Box::new(bin(col(1), BinaryOperator::Plus, col(2))),
            list: thin_vec![col(3)],
            negated: false,
            meta: meta(0, 0),
        }),
        "a + b IN (c)"
    );
    assert_eq!(
        canon(&Expr::InList {
            expr: Box::new(bin(col(1), BinaryOperator::Or, col(2))),
            list: thin_vec![col(3)],
            negated: false,
            meta: meta(0, 0),
        }),
        "(a OR b) IN (c)"
    );
}

#[test]
fn predicate_as_operand_of_other_operator_parenthesizes() {
    // The inverse of the operand-side rule: a predicate *used as an operand* of
    // another operator parenthesizes by the same binding-power oracle, so the tree
    // round-trips instead of collapsing to a different parse (ADR-0008).

    // At equal (comparison) precedence a predicate is a non-associative sibling and
    // wraps on either side; bare `a IS NULL = b` would re-parse to a forbidden
    // comparison chain.
    assert_eq!(
        canon(&bin(
            is_null(col(1)),
            BinaryOperator::Eq(EqualsSpelling::Single),
            col(2)
        )),
        "(a IS NULL) = b"
    );
    assert_eq!(
        canon(&bin(
            col(1),
            BinaryOperator::Eq(EqualsSpelling::Single),
            is_null(col(2))
        )),
        "a = (b IS NULL)"
    );

    // Under a looser-binding operator the predicate already closes tighter, so no
    // parens are added: `a IS NULL AND b` round-trips as written.
    assert_eq!(
        canon(&bin(is_null(col(1)), BinaryOperator::And, col(2))),
        "a IS NULL AND b"
    );
    // A `BETWEEN` under `OR` likewise stays bare; its internal `AND` is part of the
    // predicate, not a looser conjunction, so `(a BETWEEN x AND y) OR c` needs no
    // parens to round-trip.
    assert_eq!(
        canon(&bin(
            Expr::Between {
                expr: Box::new(col(1)),
                low: Box::new(col(2)),
                high: Box::new(col(3)),
                negated: false,
                symmetric: false,
                meta: meta(0, 0),
            },
            BinaryOperator::Or,
            col(1),
        )),
        "a BETWEEN b AND c OR a"
    );

    // Prefix `-` (80) binds tighter than the predicate, so it must wrap its operand;
    // otherwise `-a IS NULL` re-parses as `(-a) IS NULL`.
    assert_eq!(
        canon(&un(UnaryOperator::Minus, is_null(col(1)))),
        "-(a IS NULL)"
    );
    // Prefix `NOT` (30) binds looser, so `NOT a IS NULL` already groups as
    // `NOT (a IS NULL)` and needs no parens.
    assert_eq!(
        canon(&un(UnaryOperator::Not, is_null(col(1)))),
        "NOT a IS NULL"
    );
}

// --- statements, queries, clauses -----------------------------------------

#[test]
fn select_renders_every_clause() {
    let resolver = VecResolver(vec!["a", "b", "t", "x"]);
    let select = Select {
        distinct: Some(SelectDistinct::Quantifier {
            quantifier: SetQuantifier::Distinct,
            meta: meta(0, 0),
        }),
        straight_join: false,
        projection: thin_vec![
            SelectItem::Expr {
                expr: col(1),
                alias: Some(ident(4, QuoteStyle::None)),
                alias_spelling: AliasSpelling::As,
                meta: meta(0, 0),
            },
            SelectItem::Wildcard {
                options: None,
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: meta(0, 0),
            },
        ],
        into: None,
        from: thin_vec![TableWithJoins {
            relation: plain_table(3),
            joins: ThinVec::new(),
            meta: meta(0, 0),
        }],
        lateral_views: ThinVec::new(),
        connect_by: None,
        selection: Some(bin(
            col(1),
            BinaryOperator::Eq(EqualsSpelling::Single),
            col(2),
        )),
        group_by: thin_vec![GroupByItem::Expr {
            expr: col(1),
            meta: meta(0, 0),
        }],
        group_by_quantifier: None,
        group_by_all: None,
        having: Some(col(1)),
        windows: ThinVec::new(),
        qualify: Some(Box::new(col(2))),
        sample: None,
        spelling: SelectSpelling::Select,
        meta: meta(0, 0),
    };
    let query = Query {
        with: None,
        body: SetExpr::Select {
            select: Box::new(select),
            meta: meta(0, 0),
        },
        order_by: thin_vec![OrderByExpr {
            expr: col(1),
            asc: Some(false),
            using: None,
            nulls_first: Some(true),
            meta: meta(0, 0),
        }],
        order_by_all: None,
        limit_by: None,
        limit: Some(Limit {
            limit: Some(int(0, 2)),
            offset: Some(int(3, 4)),
            syntax: LimitSyntax::LimitOffset,
            with_ties: None,
            percent: None,
            fetch_spelling: FetchSpelling::FirstRows,
            meta: meta(0, 0),
        }),
        settings: ThinVec::new(),
        format: None,
        locking: ThinVec::new(),
        pipe_operators: ThinVec::new(),
        for_clause: None,
        meta: meta(0, 0),
    };
    // A statement renders identically to the query it wraps.
    let statement = Statement::Query {
        query: Box::new(query),
        meta: meta(0, 0),
    };

    assert_eq!(
        render_with(&statement, &resolver, "10 5", RenderMode::Canonical),
        "SELECT DISTINCT a AS x, * FROM t WHERE a = b GROUP BY a HAVING a QUALIFY b \
         ORDER BY a DESC NULLS FIRST LIMIT 10 OFFSET 5"
    );
}

fn select_with_distinct(distinct: Option<SelectDistinct>, projection_col: u32) -> Select {
    Select {
        distinct,
        straight_join: false,
        projection: thin_vec![SelectItem::Expr {
            expr: col(projection_col),
            alias: None,
            alias_spelling: AliasSpelling::As,
            meta: meta(0, 0),
        }],
        into: None,
        from: ThinVec::new(),
        lateral_views: ThinVec::new(),
        connect_by: None,
        selection: None,
        group_by: ThinVec::new(),
        group_by_quantifier: None,
        group_by_all: None,
        having: None,
        windows: ThinVec::new(),
        qualify: None,
        sample: None,
        spelling: SelectSpelling::Select,
        meta: meta(0, 0),
    }
}

#[test]
fn select_set_quantifiers_render() {
    let resolver = VecResolver(vec!["a", "b", "c"]);

    // Explicit `ALL` is preserved (it round-trips, unlike folding to bare SELECT).
    let all = select_with_distinct(
        Some(SelectDistinct::Quantifier {
            quantifier: SetQuantifier::All,
            meta: meta(0, 0),
        }),
        1,
    );
    assert_eq!(
        render_with(&all, &resolver, "", RenderMode::Canonical),
        "SELECT ALL a"
    );

    let distinct = select_with_distinct(
        Some(SelectDistinct::Quantifier {
            quantifier: SetQuantifier::Distinct,
            meta: meta(0, 0),
        }),
        1,
    );
    assert_eq!(
        render_with(&distinct, &resolver, "", RenderMode::Canonical),
        "SELECT DISTINCT a"
    );

    // PostgreSQL `DISTINCT ON (a, b)` keeps and renders its key list.
    let on = select_with_distinct(
        Some(SelectDistinct::On {
            exprs: thin_vec![col(1), col(2)],
            meta: meta(0, 0),
        }),
        3,
    );
    assert_eq!(
        render_with(&on, &resolver, "", RenderMode::Canonical),
        "SELECT DISTINCT ON (a, b) c"
    );
}

#[test]
fn group_by_all_spelling_honours_source_and_normalizes_elsewhere() {
    let resolver = VecResolver(vec!["a"]);
    let render = |node: &Select, spelling: RenderSpelling, mode: RenderMode| {
        let config = RenderConfig {
            spelling,
            mode,
            ..RenderConfig::default()
        };
        let ctx = RenderCtx::new(&resolver, "", &config);
        node.displayed(&ctx).to_string()
    };

    let mut star = select_with_distinct(None, 1);
    star.group_by_all = Some(GroupByAllSpelling::Star);
    // Source fidelity (`PreserveSource`) replays the `*` shorthand exactly.
    assert_eq!(
        render(&star, RenderSpelling::PreserveSource, RenderMode::Canonical),
        "SELECT a GROUP BY *"
    );
    // A target-dialect re-spell canonicalizes `*` onto the keyword `ALL`.
    assert_eq!(
        render(&star, RenderSpelling::TargetDialect, RenderMode::Canonical),
        "SELECT a GROUP BY ALL"
    );
    // The redacted fingerprint canonicalizes too — the `*`/`ALL` spelling is cosmetic
    // (the identifier is separately masked to the `id` placeholder in this mode).
    assert_eq!(
        render(&star, RenderSpelling::PreserveSource, RenderMode::Redacted),
        "SELECT id GROUP BY ALL"
    );

    // The keyword form is already canonical: it renders `ALL` in every spelling mode.
    let mut keyword = select_with_distinct(None, 1);
    keyword.group_by_all = Some(GroupByAllSpelling::Keyword);
    assert_eq!(
        render(
            &keyword,
            RenderSpelling::PreserveSource,
            RenderMode::Canonical
        ),
        "SELECT a GROUP BY ALL"
    );
    assert_eq!(
        render(
            &keyword,
            RenderSpelling::TargetDialect,
            RenderMode::Canonical
        ),
        "SELECT a GROUP BY ALL"
    );
}

#[test]
fn aggregate_all_quantifier_renders() {
    let resolver = VecResolver(vec!["count", "x"]);
    let call = Expr::Function {
        call: Box::new(FunctionCall {
            name: name(1),
            quantifier: Some(SetQuantifier::All),
            args: thin_vec![pos_arg(col(2))],
            wildcard: false,
            order_by: ThinVec::new(),
            separator: None,
            within_group: None,
            filter: None,
            filter_where: FilterWhereSpelling::Where,
            over: None,
            null_treatment: None,
            window_tail: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&call, &resolver, "", RenderMode::Canonical),
        "count(ALL x)"
    );
}

#[test]
fn fetch_first_limit_renders() {
    let resolver = r_abc();

    // OFFSET … ROWS pairs with FETCH FIRST … ROWS ONLY in the canonical spelling.
    let both = Limit {
        limit: Some(int(2, 3)),
        offset: Some(int(0, 1)),
        syntax: LimitSyntax::FetchFirst,
        with_ties: Some(false),
        percent: None,
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&both, &resolver, "5 2", RenderMode::Canonical),
        "OFFSET 5 ROWS FETCH FIRST 2 ROWS ONLY"
    );

    // A fetch with no offset renders the FETCH clause alone.
    let fetch_only = Limit {
        limit: Some(int(0, 1)),
        offset: None,
        syntax: LimitSyntax::FetchFirst,
        with_ties: Some(false),
        percent: None,
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&fetch_only, &resolver, "2", RenderMode::Canonical),
        "FETCH FIRST 2 ROWS ONLY"
    );

    // No explicit count (PostgreSQL defaults it to 1) still renders the FETCH
    // clause; `with_ties: Some(_)` alone signals it was written.
    let fetch_no_count: Limit = Limit {
        limit: None,
        offset: None,
        syntax: LimitSyntax::FetchFirst,
        with_ties: Some(false),
        percent: None,
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&fetch_no_count, &resolver, "", RenderMode::Canonical),
        "FETCH FIRST ROWS ONLY"
    );

    // WITH TIES replaces the default ONLY.
    let with_ties = Limit {
        limit: Some(int(0, 1)),
        offset: None,
        syntax: LimitSyntax::FetchFirst,
        with_ties: Some(true),
        percent: None,
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&with_ties, &resolver, "2", RenderMode::Canonical),
        "FETCH FIRST 2 ROWS WITH TIES"
    );

    // An OFFSET with no FETCH tail at all renders only the OFFSET — distinct
    // from a countless FETCH ... ONLY tail, which bounds the result to 1 row.
    let offset_only = Limit {
        limit: None,
        offset: Some(int(0, 1)),
        syntax: LimitSyntax::FetchFirst,
        with_ties: None,
        percent: None,
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&offset_only, &resolver, "5", RenderMode::Canonical),
        "OFFSET 5 ROWS"
    );
}

#[test]
fn percent_limit_round_trips_each_spelling() {
    let resolver = r_abc();

    // `PERCENT` keyword: `LIMIT 40 PERCENT`.
    let keyword = Limit {
        limit: Some(int(0, 2)),
        offset: None,
        syntax: LimitSyntax::LimitOffset,
        with_ties: None,
        percent: Some(LimitPercent::Keyword),
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&keyword, &resolver, "40", RenderMode::Canonical),
        "LIMIT 40 PERCENT"
    );

    // `%` operator: `LIMIT 35%`, rendered with no separating space.
    let symbol = Limit {
        limit: Some(int(0, 2)),
        offset: None,
        syntax: LimitSyntax::LimitOffset,
        with_ties: None,
        percent: Some(LimitPercent::Symbol),
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&symbol, &resolver, "35", RenderMode::Canonical),
        "LIMIT 35%"
    );

    // The percentage count still composes with a trailing OFFSET.
    let with_offset = Limit {
        limit: Some(int(0, 2)),
        offset: Some(int(3, 4)),
        syntax: LimitSyntax::LimitOffset,
        with_ties: None,
        percent: Some(LimitPercent::Symbol),
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&with_offset, &resolver, "40 1", RenderMode::Canonical),
        "LIMIT 40% OFFSET 1"
    );
}

#[test]
fn percent_limit_parenthesizes_a_compound_count() {
    let resolver = r_abc();

    // The `%` marker reduces onto a multiplicative-or-tighter operand, so a count
    // carrying a looser binary operator is re-parenthesized to round-trip: bare
    // `LIMIT 30 - 10%` would reparse the `10%` as the count, not `(30 - 10)`.
    let compound = Limit {
        limit: Some(bin(int(0, 2), BinaryOperator::Minus, int(5, 7))),
        offset: None,
        syntax: LimitSyntax::LimitOffset,
        with_ties: None,
        percent: Some(LimitPercent::Symbol),
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&compound, &resolver, "30 - 10", RenderMode::Canonical),
        "LIMIT (30 - 10)%"
    );

    // A primary count (here a column reference) reparses under a trailing `%` on its
    // own, so it stays bare — the marker's operand is precisely the primary grammar.
    let primary = Limit {
        limit: Some(col(1)),
        offset: None,
        syntax: LimitSyntax::LimitOffset,
        with_ties: None,
        percent: Some(LimitPercent::Symbol),
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&primary, &resolver, "a", RenderMode::Canonical),
        "LIMIT a%"
    );
}

#[test]
fn create_table_renders_definition_body_and_options() {
    let resolver = VecResolver(vec!["t", "a", "pk"]);
    let statement: Statement = Statement::CreateTable {
        create: Box::new(CreateTable {
            or_replace: false,
            temporary: Some(TemporaryTableKind::Temp),
            unlogged: false,
            if_not_exists: true,
            name: name(1),
            body: CreateTableBody::Definition {
                elements: thin_vec![
                    TableElement::Column {
                        column: ColumnDef {
                            name: ident(2, QuoteStyle::None),
                            data_type: Some(DataType::Integer {
                                spelling: IntegerTypeName::Int,
                                display_width: None,
                                meta: meta(0, 0),
                            }),
                            storage: None,
                            compression: None,
                            constraints: thin_vec![ColumnConstraint {
                                name: None,
                                option: ColumnOption::NotNull { meta: meta(0, 0) },
                                conflict: None,
                                characteristics: None,
                                meta: meta(0, 0),
                            }],
                            meta: meta(0, 0),
                        },
                        meta: meta(0, 0),
                    },
                    TableElement::Constraint {
                        constraint: TableConstraintDef {
                            name: Some(ident(3, QuoteStyle::None)),
                            constraint: TableConstraint::PrimaryKey {
                                columns: thin_vec![key_col(2)],
                                include: ThinVec::new(),
                                meta: meta(0, 0),
                            },
                            no_inherit: false,
                            not_valid: false,
                            characteristics: None,
                            meta: meta(0, 0),
                        },
                        meta: meta(0, 0),
                    },
                ],
                meta: meta(0, 0),
            },
            inherits: ThinVec::new(),
            partition_by: None,
            access_method: None,
            options: thin_vec![CreateTableOption {
                kind: CreateTableOptionKind::OnCommit {
                    action: OnCommitAction::Drop,
                    meta: meta(0, 0),
                },
                meta: meta(0, 0),
            }],
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };

    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "CREATE TEMP TABLE IF NOT EXISTS t (a INT NOT NULL, CONSTRAINT pk PRIMARY KEY (a)) \
         ON COMMIT DROP"
    );
}

fn int_type() -> DataType {
    DataType::Integer {
        spelling: IntegerTypeName::Int,
        display_width: None,
        meta: meta(0, 0),
    }
}

fn int_column(id: u32) -> ColumnDef {
    ColumnDef {
        name: ident(id, QuoteStyle::None),
        data_type: Some(int_type()),
        storage: None,
        compression: None,
        constraints: ThinVec::new(),
        meta: meta(0, 0),
    }
}

#[test]
fn alter_table_renders_actions_and_guards() {
    // t, a, b, c, u, pk -> symbols 1..6.
    let resolver = VecResolver(vec!["t", "a", "b", "c", "u", "pk"]);
    let statement: Statement = Statement::AlterTable {
        alter: Box::new(AlterTable {
            if_exists: true,
            name: name(1),
            actions: thin_vec![
                AlterTableAction::AddColumn {
                    if_not_exists: true,
                    column_keyword: true,
                    target: None,
                    column: int_column(2),
                    meta: meta(0, 0),
                },
                AlterTableAction::DropColumn {
                    if_exists: false,
                    column_keyword: true,
                    name: AlterColumnTarget {
                        parts: thin_vec![ident(3, QuoteStyle::None)],
                        meta: meta(0, 0),
                    },
                    behavior: Some(DropBehavior::Cascade),
                    meta: meta(0, 0),
                },
                AlterTableAction::AlterColumn {
                    column_keyword: true,
                    name: ident(4, QuoteStyle::None),
                    change: AlterColumnAction::SetDataType {
                        set_data: true,
                        data_type: int_type(),
                        using: None,
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
                AlterTableAction::AddConstraint {
                    constraint: TableConstraintDef {
                        name: Some(ident(5, QuoteStyle::None)),
                        constraint: TableConstraint::Unique {
                            columns: thin_vec![key_col(2)],
                            nulls_not_distinct: None,
                            include: ThinVec::new(),
                            meta: meta(0, 0),
                        },
                        no_inherit: false,
                        not_valid: false,
                        characteristics: None,
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
                AlterTableAction::DropConstraint {
                    if_exists: true,
                    name: ident(6, QuoteStyle::None),
                    behavior: Some(DropBehavior::Restrict),
                    meta: meta(0, 0),
                },
            ],
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };

    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "ALTER TABLE IF EXISTS t ADD COLUMN IF NOT EXISTS a INT, DROP COLUMN b CASCADE, \
         ALTER COLUMN c SET DATA TYPE INT, ADD CONSTRAINT u UNIQUE (a), \
         DROP CONSTRAINT IF EXISTS pk RESTRICT"
    );
}

#[test]
fn alter_table_nested_column_targets_render() {
    let resolver = VecResolver(vec!["t", "s", "s2", "j", "k"]);
    let statement: Statement = Statement::AlterTable {
        alter: Box::new(AlterTable {
            if_exists: false,
            name: name(1),
            actions: thin_vec![
                AlterTableAction::AddColumn {
                    if_not_exists: false,
                    column_keyword: true,
                    target: Some(AlterColumnTarget {
                        parts: thin_vec![
                            ident(2, QuoteStyle::None),
                            ident(3, QuoteStyle::None),
                            ident(4, QuoteStyle::None),
                        ],
                        meta: meta(0, 0),
                    }),
                    column: int_column(4),
                    meta: meta(0, 0),
                },
                AlterTableAction::DropColumn {
                    if_exists: true,
                    column_keyword: true,
                    name: AlterColumnTarget {
                        parts: thin_vec![
                            ident(2, QuoteStyle::None),
                            ident(3, QuoteStyle::None),
                            ident(5, QuoteStyle::None),
                        ],
                        meta: meta(0, 0),
                    },
                    behavior: None,
                    meta: meta(0, 0),
                },
            ],
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };

    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "ALTER TABLE t ADD COLUMN s.s2.j INT, DROP COLUMN IF EXISTS s.s2.k"
    );
}

#[test]
fn alter_column_set_default_and_nullability_render() {
    let resolver = VecResolver(vec!["t", "a", "b"]);
    let statement: Statement = Statement::AlterTable {
        alter: Box::new(AlterTable {
            if_exists: false,
            name: name(1),
            actions: thin_vec![
                AlterTableAction::AlterColumn {
                    column_keyword: true,
                    name: ident(2, QuoteStyle::None),
                    // The literal `0` is sliced from the supplied source string.
                    change: AlterColumnAction::SetDefault {
                        expr: Box::new(int(0, 1)),
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
                AlterTableAction::AlterColumn {
                    column_keyword: true,
                    name: ident(3, QuoteStyle::None),
                    change: AlterColumnAction::DropDefault { meta: meta(0, 0) },
                    meta: meta(0, 0),
                },
            ],
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };

    assert_eq!(
        render_with(&statement, &resolver, "0", RenderMode::Canonical),
        "ALTER TABLE t ALTER COLUMN a SET DEFAULT 0, ALTER COLUMN b DROP DEFAULT"
    );
}

#[test]
fn drop_statement_renders_object_kinds_names_and_behaviour() {
    let resolver = VecResolver(vec!["a", "b"]);
    let statement: Statement = Statement::Drop {
        drop: Box::new(DropStatement {
            object_kind: DropObjectKind::Table,
            if_exists: true,
            names: thin_vec![name(1), name(2)],
            behavior: Some(DropBehavior::Cascade),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "DROP TABLE IF EXISTS a, b CASCADE"
    );

    let view = VecResolver(vec!["v"]);
    for (kind, expected) in [
        (DropObjectKind::View, "DROP VIEW v RESTRICT"),
        (DropObjectKind::Index, "DROP INDEX v RESTRICT"),
        (DropObjectKind::Schema, "DROP SCHEMA v RESTRICT"),
        (DropObjectKind::Macro, "DROP MACRO v RESTRICT"),
        (DropObjectKind::MacroTable, "DROP MACRO TABLE v RESTRICT"),
    ] {
        let statement: Statement = Statement::Drop {
            drop: Box::new(DropStatement {
                object_kind: kind,
                if_exists: false,
                names: thin_vec![name(1)],
                behavior: Some(DropBehavior::Restrict),
                meta: meta(0, 0),
            }),
            meta: meta(0, 0),
        };
        assert_eq!(
            render_with(&statement, &view, "", RenderMode::Canonical),
            expected
        );
    }
}

#[test]
fn create_schema_renders_guard_name_and_authorization() {
    let resolver = VecResolver(vec!["s", "joe"]);
    let statement: Statement = Statement::CreateSchema {
        schema: Box::new(CreateSchema {
            if_not_exists: true,
            name: Some(name(1)),
            authorization: Some(ident(2, QuoteStyle::None)),
            elements: ThinVec::new(),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "CREATE SCHEMA IF NOT EXISTS s AUTHORIZATION joe"
    );

    let statement: Statement = Statement::CreateSchema {
        schema: Box::new(CreateSchema {
            if_not_exists: false,
            name: None,
            authorization: Some(ident(2, QuoteStyle::None)),
            elements: ThinVec::new(),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "CREATE SCHEMA AUTHORIZATION joe"
    );
}

#[test]
fn create_view_renders_or_replace_columns_and_check_option() {
    let resolver = VecResolver(vec!["v", "a", "b"]);
    let statement: Statement = Statement::CreateView {
        view: Box::new(CreateView {
            or_replace: true,
            options: ViewOptions::default(),
            materialized: false,
            recursive: false,
            temporary: None,
            if_not_exists: false,
            name: name(1),
            columns: thin_vec![ident(2, QuoteStyle::None), ident(3, QuoteStyle::None)],
            query: Box::new(query_one(2)),
            check_option: Some(ViewCheckOption::Local),
            with_data: None,
            to: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "CREATE OR REPLACE VIEW v (a, b) AS SELECT a WITH LOCAL CHECK OPTION"
    );
}

#[test]
fn create_materialized_view_renders_guard_and_with_data() {
    let resolver = VecResolver(vec!["m", "a"]);
    let statement: Statement = Statement::CreateView {
        view: Box::new(CreateView {
            or_replace: false,
            options: ViewOptions::default(),
            materialized: true,
            recursive: false,
            temporary: None,
            if_not_exists: true,
            name: name(1),
            columns: ThinVec::new(),
            query: Box::new(query_one(2)),
            check_option: None,
            with_data: Some(false),
            to: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "CREATE MATERIALIZED VIEW IF NOT EXISTS m AS SELECT a WITH NO DATA"
    );
}

#[test]
fn create_recursive_view_renders_keyword_before_view() {
    let resolver = VecResolver(vec!["v", "a"]);
    let statement: Statement = Statement::CreateView {
        view: Box::new(CreateView {
            or_replace: true,
            options: ViewOptions::default(),
            materialized: false,
            recursive: true,
            temporary: Some(TemporaryTableKind::Temporary),
            if_not_exists: false,
            name: name(1),
            columns: thin_vec![ident(2, QuoteStyle::None)],
            query: Box::new(query_one(2)),
            check_option: None,
            with_data: None,
            to: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "CREATE OR REPLACE TEMPORARY RECURSIVE VIEW v (a) AS SELECT a"
    );
}

#[test]
fn create_view_renders_mysql_algorithm_definer_and_sql_security_prefix() {
    let resolver = VecResolver(vec!["v", "a", "root"]);
    let statement: Statement = Statement::CreateView {
        view: Box::new(CreateView {
            or_replace: false,
            options: ViewOptions {
                algorithm: Some(ViewAlgorithm::Merge),
                definer: Some(Box::new(Definer::Account {
                    user: ident(3, QuoteStyle::None),
                    host: None,
                    meta: meta(0, 0),
                })),
                sql_security: Some(SqlSecurityContext::Invoker),
            },
            materialized: false,
            recursive: false,
            temporary: None,
            if_not_exists: false,
            name: name(1),
            columns: ThinVec::new(),
            query: Box::new(query_one(2)),
            check_option: None,
            with_data: None,
            to: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "CREATE ALGORITHM = MERGE DEFINER = root SQL SECURITY INVOKER VIEW v AS SELECT a"
    );
}

#[test]
fn alter_view_renders_options_columns_and_check_option() {
    let resolver = VecResolver(vec!["v", "a"]);
    let statement: Statement = Statement::AlterView {
        alter: Box::new(AlterView {
            options: ViewOptions {
                algorithm: Some(ViewAlgorithm::Undefined),
                definer: Some(Box::new(Definer::CurrentUser {
                    parens: false,
                    meta: meta(0, 0),
                })),
                sql_security: Some(SqlSecurityContext::Definer),
            },
            name: name(1),
            columns: thin_vec![ident(2, QuoteStyle::None)],
            query: Box::new(query_one(2)),
            check_option: Some(ViewCheckOption::Cascaded),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "ALTER ALGORITHM = UNDEFINED DEFINER = CURRENT_USER SQL SECURITY DEFINER VIEW v (a) \
         AS SELECT a WITH CASCADED CHECK OPTION"
    );
}

#[test]
fn alter_view_renders_bare_redefinition() {
    let resolver = VecResolver(vec!["v", "a"]);
    let statement: Statement = Statement::AlterView {
        alter: Box::new(AlterView {
            options: ViewOptions::default(),
            name: name(1),
            columns: ThinVec::new(),
            query: Box::new(query_one(2)),
            check_option: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "ALTER VIEW v AS SELECT a"
    );
}

#[test]
fn create_index_renders_unique_concurrently_using_and_partial_where() {
    let resolver = VecResolver(vec!["i", "t", "a", "b", "btree"]);
    let statement: Statement = Statement::CreateIndex {
        index: Box::new(CreateIndex {
            unique: true,
            concurrently: true,
            if_not_exists: true,
            name: Some(ident(1, QuoteStyle::None)),
            table: name(2),
            using: Some(ident(5, QuoteStyle::None)),
            columns: thin_vec![
                IndexColumn {
                    expr: col(3),
                    asc: None,
                    nulls_first: None,
                    meta: meta(0, 0),
                },
                IndexColumn {
                    expr: col(4),
                    asc: Some(false),
                    nulls_first: Some(false),
                    meta: meta(0, 0),
                },
            ],
            predicate: Some(Box::new(is_null(col(3)))),
            with_params: ThinVec::new(),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&statement, &resolver, "", RenderMode::Canonical),
        "CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS i ON t USING btree \
         (a, b DESC NULLS LAST) WHERE a IS NULL"
    );
}

#[test]
fn update_and_delete_render_mutation_clauses() {
    let resolver = VecResolver(vec!["t", "target", "a", "b", "u", "src"]);
    let using = thin_vec![TableWithJoins {
        relation: plain_table(5),
        joins: ThinVec::new(),
        meta: meta(0, 0),
    }];
    let predicate = bin(col(3), BinaryOperator::Eq(EqualsSpelling::Single), col(4));

    let update: Statement = Statement::Update {
        update: Box::new(Update {
            with: None,
            or_action: None,
            target: DmlTarget {
                name: name(1),
                inheritance: RelationInheritance::Plain,
                alias: Some(ident(2, QuoteStyle::None)),
                alias_spelling: AliasSpelling::As,
                meta: meta(0, 0),
            },
            target_joins: ThinVec::new(),
            assignments: thin_vec![
                UpdateAssignment::Single {
                    target: name(3),
                    value: UpdateValue::Expr {
                        expr: int(0, 1),
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
                UpdateAssignment::Single {
                    target: name(4),
                    value: UpdateValue::Default {
                        default: DefaultValue { meta: meta(0, 0) },
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
            ],
            from: using.clone(),
            selection: Some(DmlSelection::Where {
                condition: predicate.clone(),
                meta: meta(0, 0),
            }),
            order_by: ThinVec::new(),
            limit: None,
            returning: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&update, &resolver, "1", RenderMode::Canonical),
        "UPDATE t AS target SET a = 1, b = DEFAULT FROM u WHERE a = b"
    );

    let cte_query = query_one(3);
    let delete: Statement = Statement::Delete {
        delete: Box::new(Delete {
            with: Some(With {
                recursive: false,
                ctes: thin_vec![Cte {
                    name: ident(6, QuoteStyle::None),
                    columns: ThinVec::new(),
                    using_key: None,
                    materialized: None,
                    body: CteBody::Query {
                        query: Box::new(cte_query),
                        meta: meta(0, 0),
                    },
                    search: None,
                    cycle: None,
                    meta: meta(0, 0),
                }],
                meta: meta(0, 0),
            }),
            target: DmlTarget {
                name: name(1),
                inheritance: RelationInheritance::Plain,
                alias: Some(ident(2, QuoteStyle::None)),
                alias_spelling: AliasSpelling::As,
                meta: meta(0, 0),
            },
            additional_targets: ThinVec::new(),
            target_joins: ThinVec::new(),
            using,
            selection: Some(DmlSelection::Where {
                condition: predicate,
                meta: meta(0, 0),
            }),
            order_by: ThinVec::new(),
            limit: None,
            returning: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&delete, &resolver, "", RenderMode::Canonical),
        "WITH src AS (SELECT a) DELETE FROM t AS target USING u WHERE a = b"
    );
}

#[test]
fn update_and_delete_advanced_forms_render() {
    let resolver = VecResolver(vec!["t", "a", "b", "c", "cur"]);

    // `ONLY ( target )`, a tuple assignment with a per-element DEFAULT value row,
    // an explicit `ROW( ... )` source, and a positioned `WHERE CURRENT OF`.
    let row_source = UpdateTupleSource::Row {
        explicit: false,
        values: thin_vec![
            UpdateValue::Expr {
                expr: int(0, 1),
                meta: meta(0, 0),
            },
            UpdateValue::Default {
                default: DefaultValue { meta: meta(0, 0) },
                meta: meta(0, 0),
            },
        ],
        meta: meta(0, 0),
    };
    let explicit_row_source = UpdateTupleSource::Row {
        explicit: true,
        values: thin_vec![UpdateValue::Expr {
            expr: int(0, 1),
            meta: meta(0, 0),
        }],
        meta: meta(0, 0),
    };
    let update: Statement = Statement::Update {
        update: Box::new(Update {
            with: None,
            or_action: None,
            target: DmlTarget {
                name: name(1),
                inheritance: RelationInheritance::Only(OnlySyntax::Parenthesized),
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: meta(0, 0),
            },
            target_joins: ThinVec::new(),
            assignments: thin_vec![
                UpdateAssignment::Tuple {
                    targets: thin_vec![name(2), name(3)],
                    source: row_source,
                    meta: meta(0, 0),
                },
                UpdateAssignment::Tuple {
                    targets: thin_vec![name(4)],
                    source: explicit_row_source,
                    meta: meta(0, 0),
                },
            ],
            from: ThinVec::new(),
            selection: Some(DmlSelection::CurrentOf {
                cursor: ident(5, QuoteStyle::None),
                meta: meta(0, 0),
            }),
            order_by: ThinVec::new(),
            limit: None,
            returning: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&update, &resolver, "1", RenderMode::Canonical),
        "UPDATE ONLY (t) SET (a, b) = (1, DEFAULT), (c) = ROW(1) WHERE CURRENT OF cur"
    );

    // `ONLY target` (bare) and a row-subquery source.
    let subquery_source = UpdateTupleSource::Subquery {
        query: Box::new(query_one(2)),
        meta: meta(0, 0),
    };
    let update: Statement = Statement::Update {
        update: Box::new(Update {
            with: None,
            or_action: None,
            target: DmlTarget {
                name: name(1),
                inheritance: RelationInheritance::Only(OnlySyntax::Bare),
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: meta(0, 0),
            },
            target_joins: ThinVec::new(),
            assignments: thin_vec![UpdateAssignment::Tuple {
                targets: thin_vec![name(2), name(3)],
                source: subquery_source,
                meta: meta(0, 0),
            }],
            from: ThinVec::new(),
            selection: None,
            order_by: ThinVec::new(),
            limit: None,
            returning: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&update, &resolver, "1", RenderMode::Canonical),
        "UPDATE ONLY t SET (a, b) = (SELECT a)"
    );

    // A bare `DEFAULT` row source defaults every target column.
    let update: Statement = Statement::Update {
        update: Box::new(Update {
            with: None,
            or_action: None,
            target: DmlTarget {
                name: name(1),
                inheritance: RelationInheritance::Plain,
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: meta(0, 0),
            },
            target_joins: ThinVec::new(),
            assignments: thin_vec![UpdateAssignment::Tuple {
                targets: thin_vec![name(2), name(3)],
                source: UpdateTupleSource::Default {
                    default: DefaultValue { meta: meta(0, 0) },
                    meta: meta(0, 0),
                },
                meta: meta(0, 0),
            }],
            from: ThinVec::new(),
            selection: None,
            order_by: ThinVec::new(),
            limit: None,
            returning: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&update, &resolver, "", RenderMode::Canonical),
        "UPDATE t SET (a, b) = DEFAULT"
    );
}

#[test]
fn insert_returning_and_on_conflict_render() {
    let resolver = VecResolver(vec!["t", "id", "n", "exc", "x", "pk"]);
    let default_source = || InsertSource::DefaultValues {
        default: DefaultValue { meta: meta(0, 0) },
        meta: meta(0, 0),
    };
    let bare_target = || InsertTarget {
        name: name(1),
        alias: None,
        alias_spelling: AliasSpelling::As,
        columns: ThinVec::new(),
        meta: meta(0, 0),
    };

    // Index arbiter with a partial-index predicate, a DO UPDATE action with its own
    // WHERE, and a RETURNING list mixing an aliased expression and a wildcard.
    let upsert: Statement = Statement::Insert {
        insert: Box::new(Insert {
            verb: InsertVerb::Insert,
            modifier: None,
            or_action: None,
            with: None,
            column_matching: None,
            target: bare_target(),
            overriding: None,
            source: default_source(),
            row_alias: None,
            upsert: Some(Box::new(Upsert::OnConflict {
                conflict: OnConflict {
                    target: Some(ConflictTarget::Index {
                        columns: thin_vec![col(2)],
                        predicate: Some(bin(col(2), BinaryOperator::Gt, int(0, 1))),
                        meta: meta(0, 0),
                    }),
                    action: ConflictAction::Update {
                        assignments: thin_vec![UpdateAssignment::Single {
                            target: name(3),
                            value: UpdateValue::Expr {
                                expr: col(4),
                                meta: meta(0, 0),
                            },
                            meta: meta(0, 0),
                        }],
                        selection: Some(bin(col(3), BinaryOperator::Lt, col(4))),
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
                meta: meta(0, 0),
            })),
            returning: Some(Returning {
                items: thin_vec![
                    SelectItem::Expr {
                        expr: col(2),
                        alias: Some(ident(5, QuoteStyle::None)),
                        alias_spelling: AliasSpelling::As,
                        meta: meta(0, 0),
                    },
                    SelectItem::Wildcard {
                        options: None,
                        alias: None,
                        alias_spelling: AliasSpelling::As,
                        meta: meta(0, 0),
                    },
                ],
                meta: meta(0, 0),
            }),
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&upsert, &resolver, "0", RenderMode::Canonical),
        "INSERT INTO t DEFAULT VALUES ON CONFLICT (id) WHERE id > 0 \
         DO UPDATE SET n = exc WHERE n < exc RETURNING id AS x, *"
    );

    // Named-constraint arbiter with a bare DO NOTHING and no RETURNING.
    let do_nothing: Statement = Statement::Insert {
        insert: Box::new(Insert {
            verb: InsertVerb::Insert,
            modifier: None,
            or_action: None,
            with: None,
            column_matching: None,
            target: bare_target(),
            overriding: None,
            source: default_source(),
            row_alias: None,
            upsert: Some(Box::new(Upsert::OnConflict {
                conflict: OnConflict {
                    target: Some(ConflictTarget::Constraint {
                        name: ident(6, QuoteStyle::None),
                        meta: meta(0, 0),
                    }),
                    action: ConflictAction::Nothing { meta: meta(0, 0) },
                    meta: meta(0, 0),
                },
                meta: meta(0, 0),
            })),
            returning: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&do_nothing, &resolver, "", RenderMode::Canonical),
        "INSERT INTO t DEFAULT VALUES ON CONFLICT ON CONSTRAINT pk DO NOTHING"
    );

    // MySQL's `ON DUPLICATE KEY UPDATE` arm reuses the same `UpdateAssignment` SET
    // body, so the only spelling difference from `DO UPDATE SET` is the clause lead-in.
    let on_duplicate: Statement = Statement::Insert {
        insert: Box::new(Insert {
            verb: InsertVerb::Insert,
            modifier: None,
            or_action: None,
            with: None,
            column_matching: None,
            target: bare_target(),
            overriding: None,
            source: default_source(),
            row_alias: None,
            upsert: Some(Box::new(Upsert::OnDuplicateKeyUpdate {
                assignments: thin_vec![UpdateAssignment::Single {
                    target: name(3),
                    value: UpdateValue::Expr {
                        expr: col(4),
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                }],
                meta: meta(0, 0),
            })),
            returning: None,
            meta: meta(0, 0),
        }),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&on_duplicate, &resolver, "", RenderMode::Canonical),
        "INSERT INTO t DEFAULT VALUES ON DUPLICATE KEY UPDATE n = exc"
    );
}

#[test]
fn with_and_values_render() {
    let resolver = VecResolver(vec!["a", "b", "seed", "n"]);
    let cte_query = Query {
        with: None,
        body: SetExpr::Values {
            values: Box::new(Values {
                explicit_row: false,
                rows: thin_vec![
                    thin_vec![values_item(col(1))],
                    thin_vec![values_item(col(2))]
                ],
                meta: meta(0, 0),
            }),
            meta: meta(0, 0),
        },
        order_by: ThinVec::new(),
        order_by_all: None,
        limit_by: None,
        limit: None,
        settings: ThinVec::new(),
        format: None,
        locking: ThinVec::new(),
        pipe_operators: ThinVec::new(),
        for_clause: None,
        meta: meta(0, 0),
    };
    let query = Query {
        with: Some(With {
            recursive: true,
            ctes: thin_vec![Cte {
                name: ident(3, QuoteStyle::None),
                columns: thin_vec![ident(4, QuoteStyle::None)],
                using_key: None,
                materialized: Some(false),
                body: CteBody::Query {
                    query: Box::new(cte_query),
                    meta: meta(0, 0),
                },
                search: None,
                cycle: None,
                meta: meta(0, 0),
            }],
            meta: meta(0, 0),
        }),
        body: SetExpr::Select {
            select: Box::new(select_one(4)),
            meta: meta(0, 0),
        },
        order_by: ThinVec::new(),
        order_by_all: None,
        limit_by: None,
        limit: None,
        settings: ThinVec::new(),
        format: None,
        locking: ThinVec::new(),
        pipe_operators: ThinVec::new(),
        for_clause: None,
        meta: meta(0, 0),
    };

    assert_eq!(
        render_with(&query, &resolver, "", RenderMode::Canonical),
        "WITH RECURSIVE seed(n) AS NOT MATERIALIZED (VALUES (a), (b)) SELECT n"
    );
}

#[test]
fn values_rows_render_default_items() {
    // A `DEFAULT` element of a VALUES row renders as the bare keyword, mixed freely
    // with expression items (prod-sql-values-default).
    let resolver = VecResolver(vec!["a", "b"]);
    let query = Query {
        with: None,
        body: SetExpr::Values {
            values: Box::new(Values {
                explicit_row: false,
                rows: thin_vec![
                    thin_vec![values_item(col(1)), values_default_item()],
                    thin_vec![values_default_item(), values_item(col(2))],
                ],
                meta: meta(0, 0),
            }),
            meta: meta(0, 0),
        },
        order_by: ThinVec::new(),
        order_by_all: None,
        limit_by: None,
        limit: None,
        settings: ThinVec::new(),
        format: None,
        locking: ThinVec::new(),
        pipe_operators: ThinVec::new(),
        for_clause: None,
        meta: meta(0, 0),
    };

    assert_eq!(
        render_with(&query, &resolver, "", RenderMode::Canonical),
        "VALUES (a, DEFAULT), (DEFAULT, b)"
    );
}

#[test]
fn joins_render_with_keyword_and_constraint() {
    let resolver = VecResolver(vec!["a", "b", "c", "t1", "t2", "t3", "t4", "t5"]);
    let join = |relation, operator| Join {
        relation,
        operator,
        meta: meta(0, 0),
    };
    let from = TableWithJoins {
        relation: plain_table(4),
        joins: thin_vec![
            join(
                plain_table(5),
                JoinOperator::Inner {
                    straight: false,
                    inner: false,
                    constraint: JoinConstraint::On {
                        expr: bin(col(1), BinaryOperator::Eq(EqualsSpelling::Single), col(2)),
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
            ),
            join(
                plain_table(6),
                JoinOperator::LeftOuter {
                    outer: false,
                    constraint: JoinConstraint::Using {
                        columns: thin_vec![ident(3, QuoteStyle::None)],
                        alias: None,
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
            ),
            join(
                plain_table(7),
                JoinOperator::Inner {
                    straight: false,
                    inner: false,
                    constraint: JoinConstraint::Natural { meta: meta(0, 0) },
                    meta: meta(0, 0),
                },
            ),
            join(plain_table(8), JoinOperator::Cross { meta: meta(0, 0) }),
        ],
        meta: meta(0, 0),
    };

    assert_eq!(
        render_with(&from, &resolver, "", RenderMode::Canonical),
        "t1 JOIN t2 ON a = b LEFT JOIN t3 USING (c) NATURAL JOIN t4 CROSS JOIN t5"
    );
}

#[test]
fn nonstandard_joins_render_with_the_prefixed_keyword() {
    // The `ASOF` prefix precedes the recorded side (`Inner` renders no side word,
    // like the standard joins), the constraint trails the relation as usual, and
    // `Positional` is bare like `Cross`.
    let resolver = VecResolver(vec!["a", "b", "l", "r", "s"]);
    let join = |relation, operator| Join {
        relation,
        operator,
        meta: meta(0, 0),
    };
    let from = TableWithJoins {
        relation: plain_table(3),
        joins: thin_vec![
            join(
                plain_table(4),
                JoinOperator::AsOf {
                    kind: AsOfJoinKind::Left,
                    constraint: JoinConstraint::On {
                        expr: bin(col(1), BinaryOperator::GtEq, col(2)),
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
            ),
            join(
                plain_table(5),
                JoinOperator::Positional { meta: meta(0, 0) }
            ),
        ],
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&from, &resolver, "", RenderMode::Canonical),
        "l ASOF LEFT JOIN r ON a >= b POSITIONAL JOIN s"
    );

    let inner: JoinOperator = JoinOperator::AsOf {
        kind: AsOfJoinKind::Inner,
        constraint: JoinConstraint::Using {
            columns: thin_vec![ident(1, QuoteStyle::None)],
            alias: None,
            meta: meta(0, 0),
        },
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(
            &join(plain_table(4), inner),
            &resolver,
            "",
            RenderMode::Canonical
        ),
        "ASOF JOIN r USING (a)"
    );

    // MSSQL CROSS/OUTER APPLY: the flavour is the whole operator keyword, the right
    // factor trails it, and there is no constraint (bare like `Cross`). Both `kind`
    // spellings render, so the sibling OUTER APPLY parser lands as a render-free change.
    assert_eq!(
        render_with(
            &join(
                plain_table(5),
                JoinOperator::Apply {
                    kind: ApplyKind::Cross,
                    meta: meta(0, 0),
                },
            ),
            &resolver,
            "",
            RenderMode::Canonical,
        ),
        "CROSS APPLY s"
    );
    assert_eq!(
        render_with(
            &join(
                plain_table(5),
                JoinOperator::Apply {
                    kind: ApplyKind::Outer,
                    meta: meta(0, 0),
                },
            ),
            &resolver,
            "",
            RenderMode::Canonical,
        ),
        "OUTER APPLY s"
    );
}

#[test]
fn semi_anti_joins_render_the_side_prefix() {
    // The `SemiAntiSide` axis selects the keyword phrase: `Sideless` is DuckDB's
    // side-less form (with its optional `ASOF` prefix), `Left`/`Right` the Spark sided
    // spellings. All four sided combinations render here, so the sibling RIGHT SEMI /
    // LEFT ANTI / RIGHT ANTI parsers land as render-free changes. The constraint trails
    // the relation as usual.
    let resolver = VecResolver(vec!["a", "b", "r"]);
    let on = || JoinConstraint::On {
        expr: bin(col(1), BinaryOperator::Eq(EqualsSpelling::Single), col(2)),
        meta: meta(0, 0),
    };
    let semi = |side| JoinOperator::Semi {
        asof: false,
        side,
        constraint: on(),
        meta: meta(0, 0),
    };
    let anti = |side| JoinOperator::Anti {
        asof: false,
        side,
        constraint: on(),
        meta: meta(0, 0),
    };
    let render_op = |operator| {
        render_with(
            &Join {
                relation: plain_table(3),
                operator,
                meta: meta(0, 0),
            },
            &resolver,
            "",
            RenderMode::Canonical,
        )
    };
    assert_eq!(
        render_op(semi(SemiAntiSide::Sideless)),
        "SEMI JOIN r ON a = b"
    );
    assert_eq!(
        render_op(semi(SemiAntiSide::Left)),
        "LEFT SEMI JOIN r ON a = b"
    );
    assert_eq!(
        render_op(semi(SemiAntiSide::Right)),
        "RIGHT SEMI JOIN r ON a = b"
    );
    assert_eq!(
        render_op(anti(SemiAntiSide::Sideless)),
        "ANTI JOIN r ON a = b"
    );
    assert_eq!(
        render_op(anti(SemiAntiSide::Left)),
        "LEFT ANTI JOIN r ON a = b"
    );
    assert_eq!(
        render_op(anti(SemiAntiSide::Right)),
        "RIGHT ANTI JOIN r ON a = b"
    );

    // The side-less `ASOF` composition still prefixes the keyword (side and `asof` are
    // mutually exclusive, so the sided forms never carry it).
    let asof_semi = JoinOperator::Semi {
        asof: true,
        side: SemiAntiSide::Sideless,
        constraint: on(),
        meta: meta(0, 0),
    };
    assert_eq!(render_op(asof_semi), "ASOF SEMI JOIN r ON a = b");
}

#[test]
fn advanced_table_factors_render() {
    let resolver = VecResolver(vec![
        "a",
        "b",
        "t",
        "x",
        "BERNOULLI",
        "generate_series",
        "g",
        "ord",
        "u",
        "v",
        "id",
        "j",
        "merged",
    ]);
    let sampled = TableFactor::Table {
        name: name(3),
        inheritance: RelationInheritance::Only(OnlySyntax::Parenthesized),
        json_path: ThinVec::new(),
        version: None,
        partition: ThinVec::new(),
        alias: Some(table_alias(4, &[1, 2])),
        indexed_by: None,
        index_hints: ThinVec::new(),
        sample: Some(TableSample {
            method: name(5),
            args: thin_vec![int(0, 2)],
            repeatable: Some(Box::new(int(3, 5))),
            meta: meta(0, 0),
        }),
        table_hints: ThinVec::new(),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&sampled, &resolver, "10 42", RenderMode::Canonical),
        "ONLY (t) AS x(a, b) TABLESAMPLE BERNOULLI (10) REPEATABLE (42)"
    );

    let function = TableFactor::Function {
        lateral: true,
        function: Box::new(FunctionCall {
            name: name(6),
            quantifier: None,
            args: thin_vec![pos_arg(int(0, 1)), pos_arg(int(2, 3))],
            wildcard: false,
            order_by: ThinVec::new(),
            separator: None,
            within_group: None,
            filter: None,
            filter_where: FilterWhereSpelling::Where,
            over: None,
            null_treatment: None,
            window_tail: None,
            meta: meta(0, 0),
        }),
        with_ordinality: true,
        alias: Some(table_alias(7, &[1, 8])),
        column_defs: ThinVec::new(),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&function, &resolver, "1 3", RenderMode::Canonical),
        "LATERAL generate_series(1, 3) WITH ORDINALITY AS g(a, ord)"
    );

    let nested: TableFactor = TableFactor::NestedJoin {
        table: Box::new(TableWithJoins {
            relation: plain_table(9),
            joins: thin_vec![Join {
                relation: plain_table(10),
                operator: JoinOperator::Inner {
                    straight: false,
                    inner: false,
                    constraint: JoinConstraint::Using {
                        columns: thin_vec![ident(11, QuoteStyle::None)],
                        alias: Some(ident(13, QuoteStyle::None)),
                        meta: meta(0, 0),
                    },
                    meta: meta(0, 0),
                },
                meta: meta(0, 0),
            }],
            meta: meta(0, 0),
        }),
        alias: Some(table_alias(12, &[])),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&nested, &resolver, "", RenderMode::Canonical),
        "(u JOIN v USING (id) AS merged) AS j"
    );
}

#[test]
fn set_operations_render() {
    let resolver = VecResolver(vec!["a", "b", "c"]);
    let union_all = SetExpr::SetOperation {
        op: SetOperator::Union,
        all: true,
        by_name: false,
        left: Box::new(set_select_one(1)),
        right: Box::new(set_select_one(2)),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&union_all, &resolver, "", RenderMode::Canonical),
        "SELECT a UNION ALL SELECT b"
    );

    let except = SetExpr::SetOperation {
        op: SetOperator::Except,
        all: false,
        by_name: false,
        left: Box::new(set_select_one(1)),
        right: Box::new(set_select_one(2)),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&except, &resolver, "", RenderMode::Canonical),
        "SELECT a EXCEPT SELECT b"
    );

    // DuckDB's name-matched modifier renders after the operator (and any `ALL`):
    // `UNION BY NAME` and `UNION ALL BY NAME` (probed on DuckDB 1.5.4).
    let union_by_name = SetExpr::SetOperation {
        op: SetOperator::Union,
        all: false,
        by_name: true,
        left: Box::new(set_select_one(1)),
        right: Box::new(set_select_one(2)),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&union_by_name, &resolver, "", RenderMode::Canonical),
        "SELECT a UNION BY NAME SELECT b"
    );

    let union_all_by_name = SetExpr::SetOperation {
        op: SetOperator::Union,
        all: true,
        by_name: true,
        left: Box::new(set_select_one(1)),
        right: Box::new(set_select_one(2)),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&union_all_by_name, &resolver, "", RenderMode::Canonical),
        "SELECT a UNION ALL BY NAME SELECT b"
    );

    let union_with_intersect_rhs = set_op(
        SetOperator::Union,
        set_select_one(1),
        set_op(SetOperator::Intersect, set_select_one(2), set_select_one(3)),
    );
    assert_eq!(
        render_with(
            &union_with_intersect_rhs,
            &resolver,
            "",
            RenderMode::Canonical
        ),
        "SELECT a UNION SELECT b INTERSECT SELECT c"
    );
    assert_eq!(
        render_with(
            &union_with_intersect_rhs,
            &resolver,
            "",
            RenderMode::Parenthesized
        ),
        "SELECT a UNION (SELECT b INTERSECT SELECT c)"
    );

    let intersect_with_union_lhs = set_op(
        SetOperator::Intersect,
        set_op(SetOperator::Union, set_select_one(1), set_select_one(2)),
        set_select_one(3),
    );
    assert_eq!(
        render_with(
            &intersect_with_union_lhs,
            &resolver,
            "",
            RenderMode::Canonical
        ),
        "(SELECT a UNION SELECT b) INTERSECT SELECT c"
    );

    let right_nested_union = set_op(
        SetOperator::Union,
        set_select_one(1),
        set_op(SetOperator::Union, set_select_one(2), set_select_one(3)),
    );
    assert_eq!(
        render_with(&right_nested_union, &resolver, "", RenderMode::Canonical),
        "SELECT a UNION (SELECT b UNION SELECT c)"
    );
    assert_eq!(
        render_config(
            &union_with_intersect_rhs,
            RenderConfig {
                target: HIGH_UNION_SET_OPERATION_FEATURES,
                ..RenderConfig::default()
            },
        ),
        "SELECT a UNION (SELECT b INTERSECT SELECT c)"
    );
}

#[test]
fn nested_queries_are_parenthesized() {
    let resolver = VecResolver(vec!["a", "x"]);

    // A subquery expression.
    let subquery = Expr::Subquery {
        query: Box::new(query_one(1)),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&subquery, &resolver, "", RenderMode::Canonical),
        "(SELECT a)"
    );

    // A derived table factor with an alias.
    let derived = TableFactor::Derived {
        lateral: false,
        subquery: Box::new(query_one(1)),
        alias: Some(table_alias(2, &[])),
        spelling: DerivedSpelling::Parenthesized,
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&derived, &resolver, "", RenderMode::Canonical),
        "(SELECT a) AS x"
    );

    // A parenthesized query body inside a set-expression position.
    let body = SetExpr::Query {
        query: Box::new(query_one(1)),
        meta: meta(0, 0),
    };
    assert_eq!(
        render_with(&body, &resolver, "", RenderMode::Canonical),
        "(SELECT a)"
    );
}

// --- opt-in debug rendering (ADR-0010 debug-SQL mitigation) -----------------

#[test]
fn debug_sql_resolves_known_symbols_and_spells_literals_by_kind() {
    // `a = 42`, where the literal spans bytes 4..6 of some source.
    let expr = bin(
        col(1),
        BinaryOperator::Eq(EqualsSpelling::Single),
        int(4, 6),
    );

    // The canonical path, given the matched source, round-trips exactly.
    assert_eq!(
        render_with(&expr, &r_abc(), "a = 42", RenderMode::Canonical),
        "a = 42",
    );
    // `debug_sql` takes only the resolver: the column still resolves to `a`, but the
    // literal is spelled by kind (debug never slices source), so `42` -> `0`.
    assert_eq!(expr.debug_sql(&r_abc()).to_string(), "a = 0");
}

#[test]
fn debug_sql_renders_placeholder_for_unknown_symbol_without_panicking() {
    // Symbol 9 is foreign to `r_abc` (which knows only 1..=3). The canonical path
    // would panic here (see `canonical_render_panics_on_unknown_symbol`); the debug
    // path shows a visible placeholder and still prints the rest of the tree.
    let expr = bin(col(1), BinaryOperator::Eq(EqualsSpelling::Single), col(9));

    assert_eq!(expr.debug_sql(&r_abc()).to_string(), "a = <unresolved>");
}

#[test]
#[should_panic(expected = "unknown symbol")]
fn canonical_render_panics_on_unknown_symbol() {
    // The strict canonical path is unchanged by the debug helper: a foreign symbol
    // is a caller bug and still panics, which is exactly why the tolerant debug path
    // exists for detached nodes.
    let _ = canon(&col(9));
}

#[test]
fn render_ctx_debug_composes_with_displayed_across_a_statement() {
    // `RenderCtx::debug` is the tolerant sibling of `RenderCtx::new`: it flows
    // through the same `displayed` wrapper, so a detached statement renders whole.
    let statement = Statement::Query {
        query: Box::new(query_one(1)),
        meta: meta(0, 0),
    };
    let resolver = r_abc();
    let config = RenderConfig::default();
    let ctx = RenderCtx::debug(&resolver, &config);
    assert_eq!(statement.displayed(&ctx).to_string(), "SELECT a");

    // A foreign symbol nested inside the statement placeholders, never panics.
    let foreign = Statement::Query {
        query: Box::new(query_one(9)),
        meta: meta(0, 0),
    };
    assert_eq!(foreign.displayed(&ctx).to_string(), "SELECT <unresolved>");
}
