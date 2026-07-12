// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

use super::*;
use crate::vocab::{Meta, NodeId, Span, Symbol};
use std::mem::size_of;
use thin_vec::thin_vec;

#[test]
fn other_no_ext_keeps_stock_enum_sizes() {
    assert_eq!(size_of::<Expr>(), EXPR_SIZE_BUDGET);
    assert_eq!(size_of::<Statement>(), STATEMENT_SIZE_BUDGET);
    assert_eq!(size_of::<SetExpr>(), SET_EXPR_SIZE_BUDGET);

    assert_eq!(size_of::<Expr>(), size_of::<ExprWithoutOther>());
    assert_eq!(size_of::<Statement>(), size_of::<StatementWithoutOther>());
    assert_eq!(
        size_of::<TableFactor>(),
        size_of::<TableFactorWithoutOther>()
    );
}

#[test]
fn ident_equality_is_structural_across_meta() {
    let first = ident(1, QuoteStyle::None, 0, 5, 1);
    let same_structure = ident(1, QuoteStyle::None, 100, 105, 2);
    let different_structure = ident(1, QuoteStyle::Double, 0, 5, 1);

    assert_eq!(first, same_structure);
    assert_ne!(first, different_structure);
}

#[test]
fn nested_query_equality_is_structural_across_meta() {
    let first = query(false, 0);
    let same_structure = query(false, 100);
    let different_structure = query(true, 0);

    assert_eq!(first, same_structure);
    assert_ne!(first, different_structure);
}

fn query(distinct: bool, offset: u32) -> Query {
    let column = ident(1, QuoteStyle::None, offset, offset + 1, offset + 1);

    Query {
        with: None,
        body: SetExpr::Select {
            select: Box::new(Select {
                distinct: distinct.then(|| SelectDistinct::Quantifier {
                    quantifier: SetQuantifier::Distinct,
                    meta: meta(offset, offset + 1, offset + 13),
                }),
                straight_join: false,
                projection: thin_vec![SelectItem::Expr {
                    expr: Expr::Column {
                        name: ObjectName(thin_vec![column]),
                        meta: meta(offset + 1, offset + 2, offset + 10),
                    },
                    alias: None,
                    alias_spelling: AliasSpelling::As,
                    meta: meta(offset + 1, offset + 2, offset + 11),
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
                meta: meta(offset + 10, offset + 20, offset + 2),
            }),
            meta: meta(offset + 10, offset + 20, offset + 12),
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
        meta: meta(offset + 10, offset + 20, offset + 3),
    }
}

fn ident(sym: u32, quote: QuoteStyle, start: u32, end: u32, id: u32) -> Ident {
    Ident {
        sym: Symbol::new(sym).expect("non-zero symbol"),
        quote,
        meta: meta(start, end, id),
    }
}

fn meta(start: u32, end: u32, id: u32) -> Meta {
    Meta::new(
        Span::new(start, end),
        NodeId::new(id).expect("non-zero node id"),
    )
}

#[expect(
    dead_code,
    reason = "mirror enum exists only for zero-cost extension size checks"
)]
enum ExprWithoutOther<X: Extension = NoExt> {
    Column {
        name: ObjectName,
        meta: Meta,
    },
    Literal {
        literal: Literal,
        meta: Meta,
    },
    BinaryOp {
        left: Box<ExprWithoutOther<X>>,
        op: BinaryOperator,
        right: Box<ExprWithoutOther<X>>,
        meta: Meta,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<ExprWithoutOther<X>>,
        meta: Meta,
    },
    Function {
        call: Box<FunctionCall<X>>,
        meta: Meta,
    },
    Case {
        case: Box<CaseExpr<X>>,
        meta: Meta,
    },
    Extract {
        extract: Box<ExtractExpr<X>>,
        meta: Meta,
    },
    Cast {
        expr: Box<ExprWithoutOther<X>>,
        data_type: Box<DataType<X>>,
        meta: Meta,
    },
    IsNull {
        expr: Box<ExprWithoutOther<X>>,
        negated: bool,
        meta: Meta,
    },
    IsTruth {
        expr: Box<ExprWithoutOther<X>>,
        value: TruthValue,
        negated: bool,
        meta: Meta,
    },
    Between {
        expr: Box<ExprWithoutOther<X>>,
        low: Box<ExprWithoutOther<X>>,
        high: Box<ExprWithoutOther<X>>,
        negated: bool,
        meta: Meta,
    },
    InList {
        expr: Box<ExprWithoutOther<X>>,
        list: ThinVec<ExprWithoutOther<X>>,
        negated: bool,
        meta: Meta,
    },
    InSubquery {
        expr: Box<ExprWithoutOther<X>>,
        subquery: Box<Query<X>>,
        negated: bool,
        meta: Meta,
    },
    Exists {
        query: Box<Query<X>>,
        meta: Meta,
    },
    QuantifiedComparison {
        left: Box<ExprWithoutOther<X>>,
        op: BinaryOperator,
        quantifier: Quantifier,
        subquery: Box<Query<X>>,
        meta: Meta,
    },
    Subquery {
        query: Box<Query<X>>,
        meta: Meta,
    },
    Parameter {
        kind: ParameterKind,
        meta: Meta,
    },
}

#[expect(
    dead_code,
    reason = "mirror enum exists only for zero-cost extension size checks"
)]
enum StatementWithoutOther<X: Extension = NoExt> {
    Query {
        query: Box<Query<X>>,
        meta: Meta,
    },
    CreateTable {
        create: Box<CreateTable<X>>,
        meta: Meta,
    },
    Insert {
        insert: Box<Insert<X>>,
        meta: Meta,
    },
    Update {
        update: Box<Update<X>>,
        meta: Meta,
    },
    Delete {
        delete: Box<Delete<X>>,
        meta: Meta,
    },
    Transaction {
        transaction: Box<TransactionStatement>,
        meta: Meta,
    },
    Session {
        session: Box<SessionStatement>,
        meta: Meta,
    },
    AccessControl {
        access: Box<AccessControlStatement>,
        meta: Meta,
    },
}

#[expect(
    dead_code,
    reason = "mirror enum exists only for zero-cost extension size checks"
)]
enum TableFactorWithoutOther<X: Extension = NoExt> {
    Table {
        name: ObjectName,
        inheritance: RelationInheritance,
        json_path: ThinVec<SemiStructuredPathSegment<X>>,
        version: Option<Box<TableVersion<X>>>,
        partition: ThinVec<Ident>,
        alias: Option<Box<TableAlias>>,
        indexed_by: Option<Box<IndexedBy>>,
        index_hints: ThinVec<IndexHint>,
        sample: Option<TableSample<X>>,
        table_hints: ThinVec<TableHint>,
        meta: Meta,
    },
    Derived {
        lateral: bool,
        subquery: Box<Query<X>>,
        alias: Option<Box<TableAlias>>,
        meta: Meta,
    },
    Function {
        lateral: bool,
        function: Box<FunctionCall<X>>,
        with_ordinality: bool,
        alias: Option<Box<TableAlias>>,
        meta: Meta,
    },
    RowsFrom {
        lateral: bool,
        functions: ThinVec<FunctionCall<X>>,
        with_ordinality: bool,
        alias: Option<Box<TableAlias>>,
        meta: Meta,
    },
    NestedJoin {
        table: Box<TableWithJoins<X>>,
        alias: Option<Box<TableAlias>>,
        meta: Meta,
    },
    SpecialFunction {
        keyword: SpecialFunctionKeyword,
        precision: Option<u32>,
        alias: Option<Box<TableAlias>>,
        meta: Meta,
    },
}

/// CamelCase leading segments that name a SQL dialect: an AST type/variant beginning
/// with one is a same-semantics spelling fork unless allowlisted as a semantic one.
const DIALECT_PREFIXES: &[&str] = &[
    "Postgres",
    "Pg",
    "Ansi",
    "MySql",
    "MySQL",
    "Tsql",
    "TSql",
    "Oracle",
    "Sqlite",
    "Snowflake",
    "BigQuery",
    "Redshift",
    "Duck",
    "Spark",
    "Maria",
];

/// Dialect-named identifiers that are an intentional *semantic* difference, not a
/// spelling fork. Adding an entry is the conscious-review escape hatch.
const DIALECT_NAME_ALLOWLIST: &[&str] = &[
    // `E'...'` escape-string body: backslash-escape *processing* differs from a
    // standard string constant — a semantic difference (acceptance gated by
    // `string_literals`), not a spelling of the same value.
    "PostgresEscape",
];

/// Guard (prod-dialect-canonical-tags-audit): an AST type or variant named
/// after a SQL dialect is almost always a same-semantics *spelling fork* — the exact
/// anti-pattern the canonical-shape policy bans (one shape + a surface tag instead).
/// This trips on any dialect-prefixed identifier in the node sources, so adding such
/// a shape fails the build until the author either collapses it to one canonical
/// shape or, for a genuine *semantic* difference, allowlists it with a rationale.
/// Mirrors the conformance `PG_DIVERGENCE_ALLOWLIST` discipline.
#[test]
fn no_dialect_named_ast_shape_forks() {
    let offenders = dialect_named_shape_offenders(DIALECT_NAME_ALLOWLIST);
    assert!(
        offenders.is_empty(),
        "dialect-named AST shapes found — collapse to one canonical shape plus a \
         surface tag (mirror `CastSyntax`, ADR-0011), or allowlist with a \
         semantic-difference rationale:\n  {}",
        offenders.join("\n  "),
    );
}

/// Teeth for [`no_dialect_named_ast_shape_forks`]: with an empty allowlist the scan
/// still reaches the one real dialect-named identifier (`PostgresEscape`), proving
/// the guard inspects live source and its allowlist is load-bearing rather than the
/// pass being vacuous.
#[test]
fn dialect_name_guard_reaches_real_identifiers() {
    let unallowlisted = dialect_named_shape_offenders(&[]);
    assert!(
        unallowlisted
            .iter()
            .any(|hit| hit.ends_with("PostgresEscape")),
        "the scan must reach the known dialect-named identifier; got {unallowlisted:?}",
    );
    // The segment matcher flags a fork lead but not letters buried in a lowercase word.
    assert!(leads_camel_segment("PostgresCast", "Postgres"));
    assert!(leads_camel_segment("PgLimit", "Pg"));
    assert!(!leads_camel_segment("Pgable", "Pg"));
    assert!(!leads_camel_segment("Parameter", "Pg"));
}

/// Scan the hand-written node sources for dialect-prefixed identifiers outside
/// `allowed`, returning `"<file>: <ident>"` for each.
fn dialect_named_shape_offenders(allowed: &[&str]) -> Vec<String> {
    use std::ffi::OsStr;
    use std::fs;
    use std::path::Path;

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ast");
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&dir).expect("read AST source directory") {
        let path = entry.expect("read AST source directory entry").path();
        if path.extension() != Some(OsStr::new("rs")) {
            continue;
        }
        // This guard file holds the prefix list itself (the codegen schema also skips
        // `tests.rs`), so scanning it would self-trip.
        if path.file_name() == Some(OsStr::new("tests.rs")) {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read AST source file");
        let file = path.file_name().unwrap_or_default().to_string_lossy();
        for raw_line in text.lines() {
            // Strip line/doc comments (doc prose carries `PostgresEscape`,
            // `PostgresCast`, … as examples) before scanning code identifiers.
            let code = match raw_line.find("//") {
                Some(index) => &raw_line[..index],
                None => raw_line,
            };
            for ident in code_identifiers(code) {
                if allowed.contains(&ident) {
                    continue;
                }
                if DIALECT_PREFIXES
                    .iter()
                    .any(|prefix| leads_camel_segment(ident, prefix))
                {
                    offenders.push(format!("{file}: {ident}"));
                }
            }
        }
    }
    offenders
}

/// Split a line of code into its Rust identifier tokens.
fn code_identifiers(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut idents = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            idents.push(&line[start..index]);
        } else {
            index += 1;
        }
    }
    idents
}

/// True when `ident` begins with `prefix` as a complete CamelCase segment: the
/// character after the prefix is uppercase, a digit, `_`, or the identifier ends — so
/// `PostgresEscape`/`PgLimit` match while a longer all-lowercase word does not.
fn leads_camel_segment(ident: &str, prefix: &str) -> bool {
    match ident.strip_prefix(prefix) {
        None => false,
        Some(rest) => match rest.chars().next() {
            None => true,
            Some(next) => next.is_ascii_uppercase() || next.is_ascii_digit() || next == '_',
        },
    }
}
