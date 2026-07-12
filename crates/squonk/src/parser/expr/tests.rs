// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

use crate::ast::dialect::{
    AggregateCallSyntax, CallSyntax, DoubleAmpersand, ExpressionSyntax, FeatureDelta, FeatureSet,
    KeywordOperators, LexicalConflict, NumericLiteralSyntax, OperatorSyntax, ParameterSyntax,
    PipeOperator, PredicateSyntax, SessionVariableSyntax, StringFuncForms, StringLiteralSyntax,
};
use crate::ast::precedence::{Assoc, BindingPower};
use crate::ast::{
    ArgSyntax, ArrayExpr, ArraySpelling, BinaryOperator, BitStringRadix, CastSyntax,
    CharacterTypeName, DataType, DecimalTypeName, EqualsSpelling, Expr, FieldSelector,
    FilterWhereSpelling, IntegerDivideSpelling, IntegerTypeName, IntervalFields,
    IsDistinctFromSpelling, IsNotDistinctFromSpelling, Literal, LiteralKind, ModuloSpelling, NoExt,
    NotEqSpelling, ParameterKind, ParameterSigil, Quantifier, RegexpSpelling, Resolver as _,
    SelectItem, SemiStructuredPathSegment, SessionVariableKind, SetExpr, SetQuantifier, Span,
    Spanned, SpecialFunctionKeyword, Statement, StructKeySpelling, SubscriptKind, TableFactor,
    TimeZone, TimestampTypeName, TruthValue, UnaryOperator, WindowFrameBound, WindowFrameExclusion,
    WindowFrameUnits, WindowSpec,
};
use crate::dialect::{Ansi, DuckDb, MySql, Postgres, Sqlite};
use crate::parser::{
    FeatureDialect, ParseOptions, Parsed, TestDialect, parse_with, parse_with_options,
};
use crate::render::Renderer;

/// ANSI plus all PostgreSQL expression-syntax extensions, so the tests below
/// isolate the new forms from the rest of the PostgreSQL preset (casing, etc.).
const PG_EXPR_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet = FeatureSet::ANSI.with(
        FeatureDelta::EMPTY
            .expression_syntax(ExpressionSyntax::POSTGRES)
            .operator_syntax(OperatorSyntax::POSTGRES)
            .call_syntax(CallSyntax::POSTGRES)
            .string_func_forms(StringFuncForms::POSTGRES)
            .aggregate_call_syntax(AggregateCallSyntax::POSTGRES),
    );
    FeatureDialect {
        features: &FEATURES,
    }
};

/// ANSI plus the PostgreSQL row-constructor (so `(a, b)` is a row operand) and the
/// `OVERLAPS` period predicate, isolating the predicate from the rest of the preset.
const OVERLAPS_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet = FeatureSet::ANSI.with(
        FeatureDelta::EMPTY
            .expression_syntax(ExpressionSyntax::POSTGRES)
            .predicate_syntax(PredicateSyntax::POSTGRES),
    );
    FeatureDialect {
        features: &FEATURES,
    }
};

/// ANSI plus the MySQL expression-syntax extensions, so the `GROUP_CONCAT`
/// separator test isolates the new form from the rest of the MySQL preset (casing,
/// keyword operators, etc.).
const MYSQL_EXPR_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet = FeatureSet::ANSI.with(
        FeatureDelta::EMPTY
            .expression_syntax(ExpressionSyntax::MYSQL)
            .operator_syntax(OperatorSyntax::MYSQL)
            .call_syntax(CallSyntax::MYSQL)
            .string_func_forms(StringFuncForms::MYSQL)
            .aggregate_call_syntax(AggregateCallSyntax::MYSQL),
    );
    FeatureDialect {
        features: &FEATURES,
    }
};

/// The full DuckDb preset with a `RenderDialect` impl, for the anonymous composite
/// type (`STRUCT`/`ROW`/`UNION`/`MAP`), array-suffix, and `TRY_CAST` parse + exact-text
/// round-trip checks — the real `DuckDb` dialect handle parses but does not render.
const DUCKDB_TYPE_DIALECT: FeatureDialect = FeatureDialect {
    features: &FeatureSet::DUCKDB,
};

/// A focused feature dialect for Snowflake/Databricks-style semi-structured paths.
const SEMI_STRUCTURED_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
            semi_structured_access: true,
            ..ExpressionSyntax::ANSI
        }));
    FeatureDialect {
        features: &FEATURES,
    }
};

/// Semi-structured paths plus the neighboring postfix/collection surfaces they must not
/// steal from.
const SEMI_STRUCTURED_POSTFIX_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
            semi_structured_access: true,
            subscript: true,
            typecast_operator: true,
            collection_literals: true,
            ..ExpressionSyntax::ANSI
        }));
    FeatureDialect {
        features: &FEATURES,
    }
};

/// Borrow the sole projection expression of a single-statement `SELECT`.
fn project_expr(parsed: &Parsed) -> &Expr<NoExt> {
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    match &select.projection[0] {
        SelectItem::Expr {
            expr, alias: None, ..
        } => expr,
        other => panic!("expected a bare expression item, got {other:?}"),
    }
}

/// Resolve an unqualified column expression to its source name.
fn column_name<'a>(parsed: &'a Parsed, expr: &Expr<NoExt>) -> &'a str {
    match expr {
        Expr::Column { name, .. } if name.0.len() == 1 => parsed.resolver().resolve(name.0[0].sym),
        other => panic!("expected an unqualified column, got {other:?}"),
    }
}

fn cast_type(parsed: &Parsed) -> &DataType {
    match project_expr(parsed) {
        Expr::Cast { data_type, .. } => data_type,
        other => panic!("expected a CAST expression, got {other:?}"),
    }
}

fn selection_expr(parsed: &Parsed) -> &Expr<NoExt> {
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    select
        .selection
        .as_ref()
        .expect("expected a WHERE expression")
}

#[test]
fn literal_keywords_parse_as_literals_in_expression_position() {
    let parsed =
        parse_with("SELECT TRUE, FALSE, NULL", TestDialect).expect("literal keywords parse");
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    let kinds: Vec<_> = select
        .projection
        .iter()
        .map(|item| {
            let SelectItem::Expr {
                expr: Expr::Literal { literal, .. },
                ..
            } = item
            else {
                panic!("expected literal projection item, got {item:?}");
            };
            literal.kind.clone()
        })
        .collect();

    assert_eq!(
        kinds,
        vec![
            LiteralKind::Boolean(true),
            LiteralKind::Boolean(false),
            LiteralKind::Null,
        ],
    );
}

#[test]
fn literal_keywords_still_follow_identifier_rules_outside_expression_position() {
    // NULL is a literal in expression position, but a name position treats it as
    // an identifier — and NULL is reserved (PostgreSQL), so a bare `FROM null`
    // is rejected while quoting bypasses reservation and makes it a table name.
    assert!(
        parse_with("SELECT a FROM null", TestDialect).is_err(),
        "a reserved keyword is not a bare table name",
    );
    let parsed = parse_with("SELECT a FROM \"null\"", TestDialect)
        .expect("a quoted reserved word is a valid table name");
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    let TableFactor::Table { name, .. } = &select.from[0].relation else {
        panic!("expected a table factor");
    };

    assert_eq!(parsed.resolver().resolve(name.0[0].sym), "null");
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    // `1 + 2 * 3` parses as `1 + (2 * 3)`.
    let parsed = parse_with("SELECT 1 + 2 * 3", TestDialect).expect("valid expression");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Plus,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("the root operator should be `+`");
    };
    assert!(matches!(**left, Expr::Literal { .. }), "left of `+` is `1`");
    assert!(
        matches!(
            **right,
            Expr::BinaryOp {
                op: BinaryOperator::Multiply,
                ..
            }
        ),
        "right of `+` is the `2 * 3` product",
    );
}

#[test]
fn parentheses_override_precedence() {
    // `(1 + 2) * 3` parses as `(1 + 2) * 3`; the parens are not stored as a node.
    let parsed = parse_with("SELECT (1 + 2) * 3", TestDialect).expect("valid expression");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Multiply,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("the root operator should be `*`");
    };
    assert!(
        matches!(
            **left,
            Expr::BinaryOp {
                op: BinaryOperator::Plus,
                ..
            }
        ),
        "left of `*` is the grouped `1 + 2`",
    );
    assert!(
        matches!(**right, Expr::Literal { .. }),
        "right of `*` is `3`"
    );
}

#[test]
fn and_binds_tighter_than_or() {
    // `a AND b OR c` parses as `(a AND b) OR c`.
    let parsed = parse_with("SELECT a AND b OR c", TestDialect).expect("valid expression");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Or,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("the root operator should be `OR`");
    };
    assert!(
        matches!(
            **left,
            Expr::BinaryOp {
                op: BinaryOperator::And,
                ..
            }
        ),
        "left of `OR` is the `a AND b` conjunction",
    );
    assert_eq!(column_name(&parsed, right), "c");
}

#[test]
fn string_concat_binds_between_additive_and_comparison() {
    // PostgreSQL ranking (the M1 oracle): `a = b || c` parses as `a = (b || c)`.
    let parsed = parse_with("SELECT a = b || c", TestDialect).expect("valid expression");
    let Expr::BinaryOp {
        op: BinaryOperator::Eq(_),
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("the root operator should be `=`");
    };
    assert!(
        matches!(
            **right,
            Expr::BinaryOp {
                op: BinaryOperator::StringConcat,
                ..
            }
        ),
        "right of `=` is the `b || c` concatenation",
    );
}

const HIGH_CONCAT_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet = FeatureSet::ANSI.with(FeatureDelta::EMPTY.binding_powers(
        FeatureSet::ANSI.binding_powers.with_binary(
            &BinaryOperator::StringConcat,
            BindingPower {
                left: 70,
                right: 71,
                assoc: Assoc::Left,
            },
        ),
    ));
    FeatureDialect {
        features: &FEATURES,
    }
};

const LEFT_ASSOC_COMPARISON_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet = FeatureSet::ANSI.with(FeatureDelta::EMPTY.binding_powers(
        FeatureSet::ANSI.binding_powers.with_binary(
            &BinaryOperator::Lt,
            BindingPower {
                left: 40,
                right: 41,
                assoc: Assoc::Left,
            },
        ),
    ));
    FeatureDialect {
        features: &FEATURES,
    }
};

const LOGICAL_OR_PIPE_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.pipe_operator(PipeOperator::LogicalOr));
    FeatureDialect {
        features: &FEATURES,
    }
};

#[test]
fn pipe_operator_meaning_is_dialect_data() {
    // ANSI/PostgreSQL: `||` concatenates and binds tighter than `=`, so `=` is the
    // root of `a = b || c` with `b || c` on its right.
    let concat = parse_with("SELECT a = b || c", TestDialect).expect("concat dialect parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Eq(_),
        right,
        ..
    } = project_expr(&concat)
    else {
        panic!("under string-concat `||`, `=` is the root of `a = b || c`");
    };
    assert!(
        matches!(
            **right,
            Expr::BinaryOp {
                op: BinaryOperator::StringConcat,
                ..
            }
        ),
        "`||` concatenates `b || c` on the right of `=`",
    );

    // MySQL-like: `||` is logical OR — the loosest operator — so it is the root of
    // `a = b || c` and `a = b` is its left operand. Same canonical `Or` shape as
    // the `OR` keyword; only the acceptance/meaning of the token changed.
    let logical_or =
        parse_with("SELECT a = b || c", LOGICAL_OR_PIPE_DIALECT).expect("OR-pipe dialect parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Or,
        left,
        ..
    } = project_expr(&logical_or)
    else {
        panic!("under logical-OR `||`, `||` is the root of `a = b || c`");
    };
    assert!(
        matches!(
            **left,
            Expr::BinaryOp {
                op: BinaryOperator::Eq(_),
                ..
            }
        ),
        "`a = b` is the left operand of the `||`-as-OR",
    );
}

const LOGICAL_AND_AMP_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.double_ampersand(DoubleAmpersand::LogicalAnd));
    FeatureDialect {
        features: &FEATURES,
    }
};

#[test]
fn double_ampersand_meaning_is_dialect_data() {
    // ANSI/PostgreSQL: `&&` is not a scalar operator, so it lexes but cannot
    // appear as an infix operator — the expression ends and the trailing `&& c`
    // is a parse error.
    assert!(
        parse_with("SELECT a && b", TestDialect).is_err(),
        "ANSI does not accept `&&` as a scalar operator",
    );

    // MySQL-like: `&&` is logical AND, the same canonical shape as the `AND`
    // keyword. `a && b` parses to a single `And`.
    let and = parse_with("SELECT a && b", LOGICAL_AND_AMP_DIALECT).expect("AND-amp dialect parses");
    assert!(
        matches!(
            project_expr(&and),
            Expr::BinaryOp {
                op: BinaryOperator::And,
                ..
            }
        ),
        "`&&` parses to logical AND under a MySQL-like dialect",
    );

    // The `AND` binding power follows automatically: AND is looser than `=`, so
    // `&&` is the root of `a = b && c` with `a = b` as its left operand.
    let rooted =
        parse_with("SELECT a = b && c", LOGICAL_AND_AMP_DIALECT).expect("AND-amp dialect parses");
    let Expr::BinaryOp {
        op: BinaryOperator::And,
        left,
        ..
    } = project_expr(&rooted)
    else {
        panic!("under `&&`-as-AND, `&&` is the root of `a = b && c`");
    };
    assert!(
        matches!(
            **left,
            Expr::BinaryOp {
                op: BinaryOperator::Eq(_),
                ..
            }
        ),
        "`a = b` is the left operand of the `&&`-as-AND",
    );
}

/// ANSI plus MySQL's keyword infix operators, isolating `DIV`/`MOD`/`XOR`/
/// `RLIKE`/`REGEXP` from the rest of the MySQL preset. Implements `RenderDialect`
/// for the round-trip check (the parser-only `MySql` dialect is not one).
const KEYWORD_OPERATOR_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.keyword_operators(KeywordOperators::MySql));
    FeatureDialect {
        features: &FEATURES,
    }
};

#[test]
fn keyword_operators_bind_at_their_dialect_precedence() {
    // `DIV` and `MOD` are multiplicative, binding tighter than `+`: `a + b DIV c`
    // roots at `+` with `b DIV c` on its right, and `MOD` reuses the canonical
    // modulo operator tagged with its keyword spelling.
    for (sql, expected) in [
        (
            "SELECT a + b DIV c",
            BinaryOperator::IntegerDivide(IntegerDivideSpelling::Div),
        ),
        (
            "SELECT a + b MOD c",
            BinaryOperator::Modulo(ModuloSpelling::Mod),
        ),
    ] {
        let parsed = parse_with(sql, KEYWORD_OPERATOR_DIALECT).expect("multiplicative parses");
        let Expr::BinaryOp {
            op: BinaryOperator::Plus,
            right,
            ..
        } = project_expr(&parsed)
        else {
            panic!("{sql}: a multiplicative keyword operator binds tighter than `+`");
        };
        let Expr::BinaryOp { op, .. } = &**right else {
            panic!("{sql}: `b OP c` is the right operand of `+`");
        };
        assert_eq!(*op, expected, "{sql}");
    }

    // `XOR` ranks strictly between `OR` and `AND`. `AND` binds tighter, so
    // `a XOR b AND c` roots at `XOR` with `b AND c` on its right...
    let xor_and =
        parse_with("SELECT a XOR b AND c", KEYWORD_OPERATOR_DIALECT).expect("`XOR`/`AND` parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Xor,
        right,
        ..
    } = project_expr(&xor_and)
    else {
        panic!("`AND` binds tighter than `XOR`, so `XOR` roots `a XOR b AND c`");
    };
    assert!(
        matches!(
            **right,
            Expr::BinaryOp {
                op: BinaryOperator::And,
                ..
            }
        ),
        "`b AND c` is the right operand of `XOR`",
    );

    // ...while `OR` binds looser, so `a XOR b OR c` roots at `OR` with `a XOR b`
    // on its left.
    let xor_or =
        parse_with("SELECT a XOR b OR c", KEYWORD_OPERATOR_DIALECT).expect("`XOR`/`OR` parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Or,
        left,
        ..
    } = project_expr(&xor_or)
    else {
        panic!("`OR` binds looser than `XOR`, so `OR` roots `a XOR b OR c`");
    };
    assert!(
        matches!(
            **left,
            Expr::BinaryOp {
                op: BinaryOperator::Xor,
                ..
            }
        ),
        "`a XOR b` is the left operand of `OR`",
    );

    // `RLIKE`/`REGEXP` match at comparison precedence, looser than `+`, and the
    // two keywords fold onto one operator distinguished by the spelling tag:
    // `a + b RLIKE c` roots at `RLIKE` with `a + b` on its left.
    for (sql, expected) in [
        ("SELECT a + b RLIKE c", RegexpSpelling::Rlike),
        ("SELECT a + b REGEXP c", RegexpSpelling::Regexp),
    ] {
        let parsed = parse_with(sql, KEYWORD_OPERATOR_DIALECT).expect("regex match parses");
        let Expr::BinaryOp {
            op: BinaryOperator::Regexp(spelling),
            left,
            ..
        } = project_expr(&parsed)
        else {
            panic!("{sql}: regex match is looser than `+`, so it roots the expression");
        };
        assert_eq!(*spelling, expected, "{sql}");
        assert!(
            matches!(
                **left,
                Expr::BinaryOp {
                    op: BinaryOperator::Plus,
                    ..
                }
            ),
            "{sql}: `a + b` is the left operand of the regex match",
        );
    }
}

#[test]
fn keyword_operators_round_trip_exact_spelling() {
    // Each keyword spelling renders back verbatim — `MOD` does not canonicalize to
    // `%`, and `RLIKE`/`REGEXP` keep the source keyword (ADR-0011 surface tags).
    for sql in [
        "SELECT a DIV b",
        "SELECT a MOD b",
        "SELECT a XOR b",
        "SELECT a RLIKE b",
        "SELECT a REGEXP b",
    ] {
        let parsed = parse_with(sql, KEYWORD_OPERATOR_DIALECT)
            .unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(KEYWORD_OPERATOR_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn duckdb_equals_and_integer_divide_spellings_round_trip() {
    // DuckDB's `==` (equality) and `//` (integer division) fold onto the canonical
    // `Eq`/`IntegerDivide` operators with a spelling tag (ADR-0011), so each renders back
    // verbatim rather than canonicalizing to `=` / `DIV`.
    for (sql, op) in [
        ("SELECT a == b", BinaryOperator::Eq(EqualsSpelling::Double)),
        (
            "SELECT a // b",
            BinaryOperator::IntegerDivide(IntegerDivideSpelling::SlashSlash),
        ),
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::BinaryOp { op: parsed_op, .. } = project_expr(&parsed) else {
            panic!("expected a binary operator for {sql}");
        };
        assert_eq!(*parsed_op, op, "operator for {sql}");
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn duckdb_integer_divide_slash_binds_multiplicative() {
    // `//` binds at multiplicative precedence, tighter than `+`: `a + b // c` roots at
    // `+` with `b // c` on its right.
    let parsed = parse_with("SELECT a + b // c", DUCKDB_TYPE_DIALECT).expect("`//` parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Plus,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected `+` at the root");
    };
    assert!(
        matches!(
            **right,
            Expr::BinaryOp {
                op: BinaryOperator::IntegerDivide(IntegerDivideSpelling::SlashSlash),
                ..
            }
        ),
        "`//` is the right operand of `+`",
    );
}

#[test]
fn duckdb_string_projection_alias_round_trips() {
    // DuckDB admits a single-quoted string as a projection alias (`alias_string_literals`),
    // reusing the MySQL round-trip machinery — the value becomes the alias and the single
    // quote is recorded so it renders back verbatim.
    let sql = "SELECT 1 AS 'x'";
    let parsed = parse_with(sql, DUCKDB_TYPE_DIALECT).expect("string alias parses");
    let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
        .render_parsed(&parsed)
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(rendered, sql);
}

#[test]
fn duckdb_symbol_spellings_and_string_alias_reject_elsewhere() {
    // ANSI has neither the DuckDB `==`/`//` spellings nor the general operator surface, so it
    // leaves the doubled byte unmunched (the second `=`/`/` is leftover input) and admits only
    // an identifier alias — each is a clean reject.
    for sql in ["SELECT a == b", "SELECT a // b", "SELECT 1 AS 'x'"] {
        assert!(parse_with(sql, TestDialect).is_err(), "ANSI rejects {sql}");
    }
    // Under PostgreSQL the DuckDB *spellings* still do not transfer — `==`/`//` do NOT fold
    // onto `Eq`/`IntegerDivide` — but they are not rejected either: the general operator
    // surface (`custom_operators`) reads each as a generic symbolic operator (`Op`-class run),
    // exactly as the real PostgreSQL parser does (a user-operator name, resolved at analysis).
    for sql in ["SELECT a == b", "SELECT a // b"] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("PG accepts {sql}: {e}"));
        assert!(matches!(project_expr(&parsed), Expr::NamedOperator { .. }));
    }
    // The single-quoted projection alias, by contrast, has no operator reading, so PostgreSQL
    // (no `alias_string_literals`) still rejects it.
    assert!(parse_with("SELECT 1 AS 'x'", Postgres).is_err());
}

#[test]
fn duckdb_shares_the_general_symbolic_operator_surface() {
    // DuckDB inherits PostgreSQL's generalized maximal-munch operator lexer and parse-accepts
    // the same `Op`-class runs (bind-rejecting the ones with no backing function) — measured
    // against libduckdb 1.5.4 in `duckdb-pg-operator-spelling-under-acceptance`. Our parser is
    // parse-only, so these now parse as generic symbolic operators instead of stray-byte
    // rejects. Infix runs fold onto `Expr::NamedOperator`; a lead operator folds onto
    // `Expr::PrefixOperator`.
    for sql in [
        "SELECT 1 <<| 2",     // geometric, no DuckDB function — parse-accept/bind-reject
        "SELECT 1 <-> 2",     // distance
        "SELECT p &&&&&@ Le", // long &/@ run (soak artifact `SELECT T,p&&&&&@Le`)
        "SELECT 1 ~ 2",       // regex match
        "SELECT 1 !~ 2",      // regex non-match
        "SELECT 1 ~* 2",      // case-insensitive regex
        "SELECT p `= q",      // backtick-led operator (soak artifact)
        "SELECT 1 ` 2",       // lone backtick infix operator
    ] {
        assert!(
            matches!(
                project_expr(
                    &parse_with(sql, DuckDb)
                        .unwrap_or_else(|e| panic!("DuckDb accepts {sql}: {e}"))
                ),
                Expr::NamedOperator { .. }
            ),
            "DuckDb reads {sql} as a bare named operator",
        );
    }
    for sql in ["SELECT @ 1", "SELECT |/ 4", "SELECT ||/ 8", "SELECT !! 3"] {
        assert!(
            matches!(
                project_expr(
                    &parse_with(sql, DuckDb)
                        .unwrap_or_else(|e| panic!("DuckDb accepts {sql}: {e}"))
                ),
                Expr::PrefixOperator { .. }
            ),
            "DuckDb reads {sql} as a prefix operator",
        );
    }
    // DuckDB drops `#` and `?` from the `Op` charset (its positional-column `#1` and
    // anonymous-parameter `?` sigils), so a run stops at either — `1 @#@ 2` is `@` then a
    // stray `#`, and a lone `1 # 2` / `1 ? 2` are stray-byte / two-expression rejects, exactly
    // as DuckDB 1.5.4 rejects them. PostgreSQL (neither sigil) keeps `#`/`?` in its runs.
    for sql in [
        "SELECT 1 @#@ 2",
        "SELECT 1 # 2",
        "SELECT 1 ? 2",
        "SELECT 1 &#& 2",
    ] {
        assert!(parse_with(sql, DuckDb).is_err(), "DuckDb rejects {sql}");
        assert!(
            parse_with(sql, Postgres).is_ok(),
            "Postgres keeps # / ? in its operator runs: {sql}"
        );
    }
}

#[test]
fn duckdb_general_operator_trees_round_trip() {
    // The generic operators the surface now admits render back verbatim (the round-trip
    // oracle over the DuckDb preset with a render impl).
    for sql in [
        "SELECT 1 <-> 2",
        "SELECT 1 ~ 2",
        "SELECT @ 1",
        "SELECT 1 ` 2",
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn duckdb_postfix_symbolic_operators_parse_in_operand_absent_position() {
    // DuckDB keeps the generalized postfix reading PostgreSQL removed in 14: a trailing
    // `Op`-class operator with no operand folds onto `Expr::PostfixOperator` — measured
    // against libduckdb 1.5.4 in `duckdb-postfix-operator-dimension` (each parse-accepts;
    // `10!` binds via `!__postfix`, the rest bind-reject the missing `__postfix` function).
    // Covers the `Custom` residue, the lone `!`/`~`, and the dedicated `&`/`|`/`<<`/`<@`/`@>`.
    for sql in [
        "SELECT 10!",
        "SELECT 1 !",
        "SELECT 1 ~",
        "SELECT 1 <->",
        "SELECT 1 !!",
        "SELECT 1 &",
        "SELECT 1 |",
        "SELECT 1 <<",
        "SELECT 1 <@",
        "SELECT 1 @>",
        "SELECT 1 ! FROM t", // a clause keyword terminates the operand — postfix, then FROM
        "SELECT (1!)",       // a closing paren terminates the operand
    ] {
        assert!(
            matches!(
                project_expr(
                    &parse_with(sql, DuckDb)
                        .unwrap_or_else(|e| panic!("DuckDb accepts {sql}: {e}"))
                ),
                Expr::PostfixOperator { .. }
            ),
            "DuckDb reads {sql} as a postfix operator",
        );
    }
    // The infix reading still wins whenever an operand follows: `1 ! + 2` is the infix
    // `1 ! (+2)` (a bare named operator), never a postfix `1!`.
    assert!(
        matches!(
            project_expr(&parse_with("SELECT 1 ! + 2", DuckDb).expect("infix `!` parses")),
            Expr::NamedOperator { .. }
        ),
        "an operand after the operator keeps the infix reading",
    );
    // The JSON arrows are NOT postfix-eligible — DuckDB syntax-errors a trailing `->`/`->>`.
    for sql in ["SELECT 1 ->", "SELECT 1 ->>"] {
        assert!(
            parse_with(sql, DuckDb).is_err(),
            "DuckDb rejects trailing {sql}"
        );
    }
}

#[test]
fn duckdb_postfix_operator_precedence_and_round_trip() {
    // Postfix binds at the "any other operator" left rank (looser than the arithmetic
    // operators), so a tighter operand groups first while the postfix stays a complete unary
    // token. Measured on DuckDB 1.5.4 via json_serialize_sql: `2 * 3!` is `(2 * 3)!`,
    // `1! < 2` is `(1!) < 2`, `-3!` is `(-3)!`, `1! :: INT` casts the postfix. The normal
    // render places the operator with a leading space (a bare token); it re-parses to the same
    // tree and re-renders identically (the idempotent round-trip oracle, ADR-0008/0014).
    for (sql, expected) in [
        ("SELECT 10!", "SELECT 10 !"),
        ("SELECT 2 * 3!", "SELECT 2 * 3 !"),
        ("SELECT 1 + 2!", "SELECT 1 + 2 !"),
        ("SELECT 1! < 2", "SELECT 1 ! < 2"),
        ("SELECT 1! :: INT", "SELECT 1 !::INTEGER"),
        ("SELECT 1! IS NULL", "SELECT 1 ! IS NULL"),
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, expected, "render for {sql}");
        let reparsed = parse_with(&rendered, DUCKDB_TYPE_DIALECT)
            .unwrap_or_else(|err| panic!("re-parse {rendered}: {err:?}"));
        let again = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&reparsed)
            .unwrap_or_else(|err| panic!("{rendered}: {err}"));
        assert_eq!(again, rendered, "round-trip changed the tree for {sql}");
    }
    // `2 * 3!` groups `(2 * 3)!` — the postfix wraps the whole product (looser than `*`), not
    // `2 * (3!)`. Verify the tree directly so the grouping is not merely a render coincidence.
    let parsed = parse_with("SELECT 2 * 3!", DuckDb).expect("`2 * 3!` parses");
    let Expr::PostfixOperator {
        postfix_operator, ..
    } = project_expr(&parsed)
    else {
        panic!("`2 * 3!` is a postfix operator at the top");
    };
    assert!(
        matches!(&postfix_operator.operand, Expr::BinaryOp { .. }),
        "the postfix operand is the whole `2 * 3` product",
    );
    // `1! < 2` groups `(1!) < 2` — the comparison's left operand is the postfix.
    let parsed = parse_with("SELECT 1! < 2", DuckDb).expect("`1! < 2` parses");
    let Expr::BinaryOp { left, .. } = project_expr(&parsed) else {
        panic!("`1! < 2` is a comparison at the top");
    };
    assert!(
        matches!(left.as_ref(), Expr::PostfixOperator { .. }),
        "the comparison's left operand is the `1!` postfix",
    );
}

#[test]
fn postfix_operators_reject_without_the_dialect() {
    // The postfix reduction is gated on `postfix_operators` (DuckDB/Lenient only). PostgreSQL
    // removed postfix operators in 14, so `SELECT 10!` rejects there (the `!` is left
    // unconsumed); ANSI/MySQL/SQLite, with no general operator surface at all, reject too.
    for dialect_rejects in [
        parse_with("SELECT 10!", Postgres).is_err(),
        parse_with("SELECT 1 ~", Postgres).is_err(),
        parse_with("SELECT 10!", Ansi).is_err(),
        parse_with("SELECT 10!", MySql).is_err(),
        parse_with("SELECT 10!", Sqlite).is_err(),
    ] {
        assert!(
            dialect_rejects,
            "postfix operators reject without the dialect gate"
        );
    }
}

#[test]
fn duckdb_unparenthesized_in_parses_as_in_expr_and_round_trips() {
    // DuckDB's unparenthesized `<expr> [NOT] IN <c_expr>` list-membership (`z IN y`,
    // desugaring to `contains(y, z)`): a restricted `c_expr` RHS — a column, qualified
    // reference, function call, subscript, array/struct/map literal, or parameter —
    // parses to the `Expr::InExpr` node and renders back verbatim (engine-verified accept
    // set, DuckDB 1.5.4).
    for (sql, negated) in [
        ("SELECT z IN y", false),
        ("SELECT z IN t.c", false),
        ("SELECT z IN f(x)", false),
        ("SELECT z IN y[1]", false),
        ("SELECT z IN [1, 2, 3]", false),
        ("SELECT z IN {'a': 1}", false),
        ("SELECT z IN ?", false),
        ("SELECT z IN $1", false),
        ("SELECT z NOT IN y", true),
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::InExpr {
            negated: parsed_negated,
            ..
        } = project_expr(&parsed)
        else {
            panic!("expected `Expr::InExpr` for {sql}");
        };
        assert_eq!(*parsed_negated, negated, "negation for {sql}");
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn duckdb_unparenthesized_in_rejects_constant_and_unary_leading_rhs() {
    // DuckDB's gram.y forbids a constant or unary-sign leading token in the
    // unparenthesized-IN RHS (an LALR grammar-generator restriction): each of these is a
    // DuckDB parser error, so the leading-token gate must reject them here too — accepting
    // any one would be a new syntax over-acceptance. Also excludes the `*` star, `EXISTS`,
    // and the `COLUMNS(…)` star expression (all engine-verified rejects on 1.5.4).
    for sql in [
        "SELECT z IN 4",
        "SELECT z IN 3.5",
        "SELECT z IN 'abc'",
        "SELECT z IN TRUE",
        "SELECT z IN FALSE",
        "SELECT z IN NULL",
        "SELECT z IN -5",
        "SELECT z IN +y",
        "SELECT z IN ~y",
        "SELECT z IN b'101'",
        "SELECT z IN DATE '2020-01-01'",
        "SELECT z IN *",
        "SELECT z IN EXISTS (SELECT 1)",
        "SELECT z IN COLUMNS('a')",
        "SELECT z NOT IN 4",
        "SELECT z NOT IN -5",
    ] {
        assert!(
            parse_with(sql, DUCKDB_TYPE_DIALECT).is_err(),
            "DuckDB rejects {sql} (constant/unary/excluded leading token)",
        );
    }
}

#[test]
fn duckdb_parenthesized_in_stays_the_standard_predicate() {
    // A `(` after `IN` is always the standard parenthesized predicate, never this
    // operator: `IN (4)` is a one-element `InList` (the leading-constant gate does not
    // reach it), `IN (SELECT …)` an `InSubquery`, `IN (y)` a one-element `InList`.
    for (sql, is_subquery) in [
        ("SELECT z IN (4)", false),
        ("SELECT z IN (y)", false),
        ("SELECT z IN (a, b)", false),
        ("SELECT z IN (SELECT 1)", true),
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let expr = project_expr(&parsed);
        if is_subquery {
            assert!(
                matches!(expr, Expr::InSubquery { .. }),
                "expected `InSubquery` for {sql}, got {expr:?}",
            );
        } else {
            assert!(
                matches!(expr, Expr::InList { .. }),
                "expected `InList` for {sql}, got {expr:?}",
            );
        }
    }
}

#[test]
fn duckdb_unparenthesized_in_binds_tighter_than_comparison_and_arithmetic() {
    // DuckDB ranks the unparenthesized `IN` between the comparison operators and
    // string-concat (measured on 1.5.4 via `json_serialize_sql`): `z = w IN y` groups
    // `z = (w IN y)` (tighter than `=`), while `a * b IN y` groups `(a * b) IN y` and
    // `a || b IN y` groups `(a || b) IN y` (looser than `*`/`||` on the left operand).
    // `z IN y IN w` is left-associative: `(z IN y) IN w`.

    // `z = w IN y` -> `=` at the root with the `InExpr` on its right.
    let parsed = parse_with("SELECT z = w IN y", DUCKDB_TYPE_DIALECT).expect("parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Eq(EqualsSpelling::Single),
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected `=` at the root");
    };
    assert!(
        matches!(**right, Expr::InExpr { .. }),
        "the `IN` is the right operand of `=`",
    );

    // `z IN y = w` -> `=` at the root with the `InExpr` on its left.
    let parsed = parse_with("SELECT z IN y = w", DUCKDB_TYPE_DIALECT).expect("parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Eq(EqualsSpelling::Single),
        left,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected `=` at the root");
    };
    assert!(
        matches!(**left, Expr::InExpr { .. }),
        "the `IN` is the left operand of `=`",
    );

    // `a * b IN y` -> `InExpr` at the root with `a * b` as its left operand.
    let parsed = parse_with("SELECT a * b IN y", DUCKDB_TYPE_DIALECT).expect("parses");
    let Expr::InExpr { expr, .. } = project_expr(&parsed) else {
        panic!("expected `InExpr` at the root");
    };
    assert!(
        matches!(
            **expr,
            Expr::BinaryOp {
                op: BinaryOperator::Multiply,
                ..
            }
        ),
        "`a * b` is the left operand of the `IN`",
    );

    // `z IN y IN w` -> left-associative: the outer `InExpr`'s left operand is another one.
    let parsed = parse_with("SELECT z IN y IN w", DUCKDB_TYPE_DIALECT).expect("parses");
    let Expr::InExpr { expr, .. } = project_expr(&parsed) else {
        panic!("expected `InExpr` at the root");
    };
    assert!(
        matches!(**expr, Expr::InExpr { .. }),
        "`z IN y IN w` associates left as `(z IN y) IN w`",
    );
}

#[test]
fn duckdb_unparenthesized_in_rhs_is_c_expr() {
    // The RHS is DuckDB's `c_expr`: subscript indirection binds *into* it (`z IN y[1]` is
    // `contains(y[1], z)`), while the `::` typecast binds *outside* (`z IN y::INT` is
    // `(z IN y)::INT`) — both measured on 1.5.4.

    // `z IN y[1]` -> the subscript is inside the RHS (root is `InExpr`, RHS a `Subscript`).
    let parsed = parse_with("SELECT z IN y[1]", DUCKDB_TYPE_DIALECT).expect("parses");
    let Expr::InExpr { rhs, .. } = project_expr(&parsed) else {
        panic!("expected `InExpr` at the root");
    };
    assert!(
        matches!(**rhs, Expr::Subscript { .. }),
        "the subscript binds into the RHS",
    );

    // `z IN y::INT` -> the cast is outside the RHS (root is `Cast` over the `InExpr`).
    let parsed = parse_with("SELECT z IN y::INT", DUCKDB_TYPE_DIALECT).expect("parses");
    let Expr::Cast { expr, .. } = project_expr(&parsed) else {
        panic!("expected `Cast` at the root");
    };
    assert!(
        matches!(**expr, Expr::InExpr { .. }),
        "the `::` typecast wraps the whole `InExpr`",
    );
}

#[test]
fn duckdb_unparenthesized_in_gated_off_in_other_dialects() {
    // The unparenthesized `IN <value>` is DuckDB(+Lenient)-gated: ANSI and PostgreSQL
    // require the parentheses, so `z IN y` is the missing-`(` parse error there. The
    // parenthesized `IN (y)` still parses everywhere.
    for sql in [
        "SELECT z IN y",
        "SELECT z NOT IN y",
        "SELECT z IN [1, 2, 3]",
    ] {
        assert!(parse_with(sql, TestDialect).is_err(), "ANSI rejects {sql}");
        assert!(
            parse_with(sql, Postgres).is_err(),
            "PostgreSQL rejects {sql}"
        );
    }
    assert!(
        parse_with("SELECT z IN (y)", Postgres).is_ok(),
        "the parenthesized form still parses",
    );
}

#[test]
fn keyword_operators_are_inert_without_the_dialect() {
    // ANSI/PostgreSQL do not treat these words as operators, so the trailing
    // operand is leftover input — the same reject path as `&&` under ANSI. The
    // `%` symbol still parses as modulo (it is not a keyword operator).
    for sql in [
        "SELECT a DIV b",
        "SELECT a MOD b",
        "SELECT a XOR b",
        "SELECT a RLIKE b",
        "SELECT a REGEXP b",
    ] {
        assert!(
            parse_with(sql, TestDialect).is_err(),
            "ANSI does not treat the keyword as an operator: {sql}",
        );
        assert!(
            parse_with(sql, Postgres).is_err(),
            "PostgreSQL does not treat the keyword as an operator: {sql}",
        );
    }
}

#[test]
fn mysql_preset_gives_keyword_operators_their_meaning() {
    // The shipped MySQL preset enables the keyword operators (not just the
    // isolated knob above), so each parses to its canonical operator there.
    let cases: [(&str, BinaryOperator); 5] = [
        (
            "SELECT a DIV b",
            BinaryOperator::IntegerDivide(IntegerDivideSpelling::Div),
        ),
        (
            "SELECT a MOD b",
            BinaryOperator::Modulo(ModuloSpelling::Mod),
        ),
        ("SELECT a XOR b", BinaryOperator::Xor),
        (
            "SELECT a RLIKE b",
            BinaryOperator::Regexp(RegexpSpelling::Rlike),
        ),
        (
            "SELECT a REGEXP b",
            BinaryOperator::Regexp(RegexpSpelling::Regexp),
        ),
    ];
    for (sql, expected) in cases {
        let parsed = parse_with(sql, MySql).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::BinaryOp { op, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a binary operator expression");
        };
        assert_eq!(*op, expected, "{sql}");
    }
}

#[test]
fn is_distinct_from_parses_to_binary_op_in_both_polarities() {
    // The predicate routes through the comparison `BinaryOperator` pair rather than a
    // dedicated expression node (close-p0-datafusion-parity-coverage-gaps).
    for (sql, expected) in [
        (
            "SELECT a IS DISTINCT FROM b",
            BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Keyword),
        ),
        (
            "SELECT a IS NOT DISTINCT FROM b",
            BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::Keyword),
        ),
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::BinaryOp { op, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a binary operator expression");
        };
        assert_eq!(*op, expected, "{sql}");
    }
}

#[test]
fn is_distinct_from_is_non_associative() {
    // Comparison precedence is non-associative, so a chained predicate is a clean
    // parse error (mirrors PostgreSQL's `%prec IS`), exactly like `a < b < c`. `IS
    // NULL` still chains onto the left operand, so this does not regress it.
    for sql in [
        "SELECT a IS DISTINCT FROM b IS DISTINCT FROM c",
        "SELECT a IS NOT DISTINCT FROM b IS NOT DISTINCT FROM c",
    ] {
        assert!(parse_with(sql, Postgres).is_err(), "{sql} should reject");
    }
}

#[test]
fn is_truth_predicate_parses_all_six_forms() {
    // The truth-value tests (SQL:2016 F571) parse to the dedicated postfix `Expr::IsTruth`,
    // the sibling of `IS NULL`, carrying the tested value and the `IS NOT` negation. A
    // one-keyword lookahead settles the shared `IS` lead against `IS NULL`/`IS DISTINCT`.
    for (sql, expected_value, expected_negated) in [
        ("SELECT a IS TRUE", TruthValue::True, false),
        ("SELECT a IS NOT TRUE", TruthValue::True, true),
        ("SELECT a IS FALSE", TruthValue::False, false),
        ("SELECT a IS NOT FALSE", TruthValue::False, true),
        ("SELECT a IS UNKNOWN", TruthValue::Unknown, false),
        ("SELECT a IS NOT UNKNOWN", TruthValue::Unknown, true),
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::IsTruth { value, negated, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected Expr::IsTruth");
        };
        assert_eq!(*value, expected_value, "{sql}");
        assert_eq!(*negated, expected_negated, "{sql}");
    }
}

#[test]
fn is_truth_binds_tighter_than_boolean_and() {
    // `IS TRUE` is a comparison-level predicate, so it binds tighter than `AND`:
    // `a IS TRUE AND b IS FALSE` is `(a IS TRUE) AND (b IS FALSE)`, a top-level boolean AND
    // over two truth tests (engine-confirmed on pg_query).
    let parsed = parse_with("SELECT a IS TRUE AND b IS FALSE", Postgres).expect("parses");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::And,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected a top-level boolean AND");
    };
    assert!(matches!(
        **left,
        Expr::IsTruth {
            value: TruthValue::True,
            negated: false,
            ..
        }
    ));
    assert!(matches!(
        **right,
        Expr::IsTruth {
            value: TruthValue::False,
            negated: false,
            ..
        }
    ));
}

#[test]
fn is_truth_is_non_associative() {
    // Like every comparison-level predicate (`IS NULL`, `IS DISTINCT FROM`, `BETWEEN`), an
    // unparenthesized truth-test chain is a non-associative parse error, mirroring `a = b =
    // c`. (PostgreSQL is laxer and accepts the chain; our whole predicate family is
    // uniformly non-associative — see the sibling `is_distinct_from_is_non_associative`.)
    // The parenthesized nesting parses.
    assert!(
        parse_with("SELECT a IS TRUE IS FALSE", Postgres).is_err(),
        "unparenthesized truth-test chain should reject"
    );
    let parsed = parse_with("SELECT (a IS TRUE) IS FALSE", Postgres).expect("parenthesized nests");
    let Expr::IsTruth {
        value: TruthValue::False,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected an outer IS FALSE over a parenthesized IS TRUE");
    };
}

#[test]
fn is_truth_gated_off_under_sqlite_general_equality() {
    // SQLite has no truth-value predicate: its `IS` is a general null-safe equality, so
    // `a IS TRUE` folds onto the null-safe-equality operator against the boolean literal
    // (never an `Expr::IsTruth`), tagged with the bare-`IS` spelling, engine-measured via
    // rusqlite.
    let parsed = parse_with("SELECT a IS TRUE", Sqlite).expect("SQLite parses `a IS TRUE`");
    assert!(
        matches!(
            project_expr(&parsed),
            Expr::BinaryOp {
                op: BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::Is),
                ..
            }
        ),
        "SQLite `a IS TRUE` is general null-safe equality, not a truth test",
    );
    // And `a IS UNKNOWN` there is equality against an identifier `unknown` — it must not be
    // rejected (SQLite accepts it against a bound column), and it is never `Expr::IsTruth`.
    let parsed = parse_with("SELECT a IS UNKNOWN", Sqlite).expect("SQLite parses `a IS UNKNOWN`");
    assert!(
        !matches!(project_expr(&parsed), Expr::IsTruth { .. }),
        "SQLite `a IS UNKNOWN` is general equality against `unknown`, not a truth test",
    );
}

const PARAMETER_DIALECT: FeatureDialect = {
    // Every placeholder form on, so the positional, anonymous, and all three named
    // sigils are exercised under one dialect. The two `$` forms are follow-set
    // disjoint (`$1` positional vs `$name` named), so both coexist. ANSI-based (no
    // subscript), so `:name` has no array-slice form to contend with here.
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
            positional_dollar: true,
            anonymous_question: true,
            named_colon: true,
            named_at: true,
            named_dollar: true,
            numbered_question: true,
        }));
    FeatureDialect {
        features: &FEATURES,
    }
};

/// The SQLite feature set as a render-capable `FeatureDialect` for round-trip tests
/// (the `Sqlite` marker parses but has no Tier-1 render dialect).
const SQLITE_DIALECT: FeatureDialect = FeatureDialect {
    features: &FeatureSet::SQLITE,
};

#[test]
fn parameter_placeholders_parse_to_parameter_expressions() {
    // Positional `$1` carries its 1-based index; `?` is anonymous.
    let positional =
        parse_with("SELECT $1", PARAMETER_DIALECT).expect("positional parameter parses");
    assert!(
        matches!(
            project_expr(&positional),
            Expr::Parameter {
                kind: ParameterKind::Positional(1),
                ..
            }
        ),
        "`$1` parses to a positional parameter with index 1: {:?}",
        project_expr(&positional),
    );

    let anonymous = parse_with("SELECT ?", PARAMETER_DIALECT).expect("anonymous parameter parses");
    assert!(
        matches!(
            project_expr(&anonymous),
            Expr::Parameter {
                kind: ParameterKind::Anonymous,
                ..
            }
        ),
        "`?` parses to an anonymous parameter",
    );

    // The placeholder forms are dialect-gated: ANSI accepts neither.
    assert!(
        parse_with("SELECT $1", TestDialect).is_err(),
        "ANSI rejects `$1`",
    );
    assert!(
        parse_with("SELECT ?", TestDialect).is_err(),
        "ANSI rejects `?`",
    );
}

#[test]
fn named_parameters_parse_and_round_trip() {
    // `:name` and `@name` parse to a named placeholder carrying the interned name
    // (sigil stripped) and the sigil tag, and render back to their exact source.
    for (sql, name, want_sigil) in [
        ("SELECT :user_id", "user_id", ParameterSigil::Colon),
        ("SELECT @count", "count", ParameterSigil::At),
    ] {
        let parsed = parse_with(sql, PARAMETER_DIALECT).expect("named parameter parses");
        let Expr::Parameter {
            kind: ParameterKind::Named { name: sym, sigil },
            ..
        } = project_expr(&parsed)
        else {
            panic!(
                "expected a named parameter, got {:?}",
                project_expr(&parsed)
            );
        };
        assert_eq!(*sigil, want_sigil, "sigil tag for {sql:?}");
        assert_eq!(
            parsed.resolver().resolve(*sym),
            name,
            "interned name (sigil stripped) for {sql:?}",
        );
        assert_eq!(
            Renderer::new(PARAMETER_DIALECT)
                .render_parsed(&parsed)
                .expect("named parameter renders"),
            sql,
            "named parameter round-trips to its source",
        );
    }

    // Dialect-gated: ANSI accepts neither named sigil. `:name` is a parse error
    // (lone `:` cannot begin an expression); `@name` is a lexical stray byte.
    assert!(
        parse_with("SELECT :user_id", TestDialect).is_err(),
        "ANSI rejects `:name`",
    );
    assert!(
        parse_with("SELECT @count", TestDialect).is_err(),
        "ANSI rejects `@name`",
    );
}

#[test]
fn sqlite_numbered_parameter_parses_range_checks_and_round_trips() {
    // SQLite numbered `?NNN` (sqlite-lexer-under-acceptance-bundle): parses to the
    // `?`-spelled positional kind carrying its 1-based index and renders back as `?N`.
    for (sql, index) in [
        ("SELECT ?1", 1),
        ("SELECT ?123", 123),
        ("SELECT ?32766", 32766),
    ] {
        let parsed = parse_with(sql, SQLITE_DIALECT).unwrap_or_else(|e| panic!("{sql:?}: {e}"));
        assert!(
            matches!(
                project_expr(&parsed),
                Expr::Parameter {
                    kind: ParameterKind::Numbered(n),
                    ..
                } if *n == index,
            ),
            "{sql:?} parses to Numbered({index}): {:?}",
            project_expr(&parsed),
        );
        assert_eq!(
            Renderer::new(SQLITE_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            sql,
            "{sql:?} round-trips",
        );
    }
    // The number is a maximal digit munch: `?1abc` is `?1` aliased `abc`.
    parse_with("SELECT ?1abc", SQLITE_DIALECT).expect("`?1abc` is `?1` aliased `abc`");
    // SQLite restricts the index to 1..=32766 (SQLITE_MAX_VARIABLE_NUMBER); out-of-range and
    // overflowing forms reject at parse time (engine-measured), not silently accept.
    for sql in [
        "SELECT ?0",
        "SELECT ?32767",
        "SELECT ?70000",
        "SELECT ?999999999999999999999",
    ] {
        let err = parse_with(sql, SQLITE_DIALECT).expect_err(&format!("{sql:?} is out of range"));
        assert_eq!(
            err.expected.as_str(),
            "a numbered parameter index between ?1 and ?32766",
            "{sql:?}",
        );
    }
    // Dialect-gated: ANSI has no numbered `?` (and no anonymous `?` either), so `?1` rejects.
    assert!(parse_with("SELECT ?1", Ansi).is_err(), "ANSI rejects `?1`");
}

#[test]
fn sqlite_bare_string_projection_alias_round_trips() {
    // SQLite reads a bare (`AS`-less) string literal in projection-alias position as the
    // column name (`bare_alias_string_literals`). The canonical (target-dialect) render
    // spells the alias with `AS` — SQLite accepts both forms — and that render re-parses,
    // proving the round-trip is structurally sound.
    let sql = "SELECT 1 'x'";
    let parsed = parse_with(sql, SQLITE_DIALECT).expect("bare string alias parses");
    let rendered = Renderer::new(SQLITE_DIALECT)
        .render_parsed(&parsed)
        .expect("renders");
    assert_eq!(
        rendered, "SELECT 1 AS 'x'",
        "bare alias canonicalises to the `AS` spelling"
    );
    parse_with(&rendered, SQLITE_DIALECT).expect("the canonical render re-parses");
    // DuckDB accepts only the `AS 'x'` form (probed), so the bare form rejects there — proving
    // the axis is separate from `alias_string_literals`.
    assert!(
        parse_with("SELECT 1 'x'", DuckDb).is_err(),
        "DuckDB rejects the bare string alias (AS-only)",
    );
    assert!(
        parse_with("SELECT 1 'x'", Ansi).is_err(),
        "ANSI rejects the bare string alias",
    );
}

#[test]
fn mysql_bare_string_alias_and_adjacent_concat_split_by_parse_order() {
    // MySQL reads a string after a NON-string expression as a bare (`AS`-less) projection
    // alias, but folds a string after a STRING into the preceding value as an
    // adjacent-string concatenation — the two readings split by parse order (the
    // continuation folds during expression parsing, before the alias parser runs). All
    // engine-measured on mysql:8.4.10 (`mysql-bare-string-alias-vs-adjacent-concat`).
    // Bare alias: the string names the column; the expression is the untouched operand.
    for (sql, alias) in [("SELECT 1 'x'", "x"), ("SELECT 1 \"x\"", "x")] {
        let parsed = parse_with(sql, MySql).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("{sql}: expected a query");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("{sql}: expected a SELECT body");
        };
        assert_eq!(select.projection.len(), 1, "{sql}: one projected column");
        let SelectItem::Expr {
            expr,
            alias: Some(ident),
            ..
        } = &select.projection[0]
        else {
            panic!(
                "{sql}: expected an aliased expression, got {:?}",
                select.projection[0]
            );
        };
        assert_eq!(
            parsed.resolver().resolve(ident.sym),
            alias,
            "{sql}: alias name"
        );
        assert!(
            matches!(expr, Expr::Literal { .. }),
            "{sql}: the operand is the bare `1`, not the alias",
        );
    }

    // Concatenation: adjacent strings (single or double, MySQL lexing `"…"` as a string)
    // fold into one value with no alias — the string never reaches the alias branch.
    for (sql, value) in [
        ("SELECT 'a' 'b'", "ab"),
        ("SELECT 'a' 'b' 'c'", "abc"),
        ("SELECT 'a' \"b\"", "ab"),
        ("SELECT \"a\" \"b\"", "ab"),
        ("SELECT N'a' 'b'", "ab"),
    ] {
        let parsed = parse_with(sql, MySql).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("{sql}: expected a query");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("{sql}: expected a SELECT body");
        };
        assert_eq!(select.projection.len(), 1, "{sql}: one projected column");
        let SelectItem::Expr {
            expr: Expr::Literal { literal, .. },
            alias: None,
            ..
        } = &select.projection[0]
        else {
            panic!(
                "{sql}: expected an unaliased literal, got {:?}",
                select.projection[0]
            );
        };
        assert_eq!(
            parsed.literal_str(literal).expect("materialises"),
            value,
            "{sql}: the adjacent segments concatenate",
        );
    }

    // The reject boundary: a bare string alias takes no second string, and only an
    // *unprefixed* string continues a concatenation (a prefixed `_charset'…'`/`N'…'`
    // second segment is neither a continuation nor a bare alias).
    for sql in [
        "SELECT 1 'x' 'y'",
        "SELECT 'a' _utf8'b'",
        "SELECT _utf8'a' _utf8'b'",
        "SELECT 'a' N'b'",
    ] {
        assert!(parse_with(sql, MySql).is_err(), "{sql}: rejects");
    }
}

#[test]
fn positional_parameter_index_overflow_is_a_clean_error() {
    // `$<huge>` lexes as one placeholder, but the index does not fit in u32, so
    // parsing reports a precise error rather than panicking on the overflow.
    let err = parse_with("SELECT $99999999999", PARAMETER_DIALECT)
        .expect_err("an out-of-range positional index is rejected");
    assert_eq!(
        err.expected.as_str(),
        "a positional parameter index within u32 range",
    );
}

const SESSION_VARIABLE_DIALECT: FeatureDialect = {
    // Both MySQL session-variable forms on (`@x` user, `@@x` system); ANSI-based
    // otherwise so `@` has no other meaning to contend with.
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.session_variables(SessionVariableSyntax::MYSQL));
    FeatureDialect {
        features: &FEATURES,
    }
};

#[test]
fn session_variables_parse_and_round_trip() {
    // All four surface forms parse to one canonical shape carrying the sigil/scope
    // kind tag and the interned name (sigil and scope stripped), and render back to
    // their exact source.
    for (sql, want_kind, name) in [
        (
            "SELECT @user_count",
            SessionVariableKind::User,
            "user_count",
        ),
        (
            "SELECT @@max_connections",
            SessionVariableKind::System,
            "max_connections",
        ),
        (
            "SELECT @@global.time_zone",
            SessionVariableKind::SystemGlobal,
            "time_zone",
        ),
        (
            "SELECT @@session.sql_mode",
            SessionVariableKind::SystemSession,
            "sql_mode",
        ),
    ] {
        let parsed = parse_with(sql, SESSION_VARIABLE_DIALECT).expect("session variable parses");
        let Expr::SessionVariable {
            kind, name: sym, ..
        } = project_expr(&parsed)
        else {
            panic!(
                "expected a session variable, got {:?}",
                project_expr(&parsed)
            );
        };
        assert_eq!(*kind, want_kind, "kind tag for {sql:?}");
        assert_eq!(
            parsed.resolver().resolve(*sym),
            name,
            "interned name (sigil/scope stripped) for {sql:?}",
        );
        assert_eq!(
            Renderer::new(SESSION_VARIABLE_DIALECT)
                .render_parsed(&parsed)
                .expect("session variable renders"),
            sql,
            "session variable round-trips to its source",
        );
    }

    // `@@global` with no `.name` is a system variable literally named `global`
    // (implicit scope), not a scoped reference.
    let bare =
        parse_with("SELECT @@global", SESSION_VARIABLE_DIALECT).expect("bare `@@global` parses");
    assert!(
        matches!(
            project_expr(&bare),
            Expr::SessionVariable {
                kind: SessionVariableKind::System,
                ..
            }
        ),
        "`@@global` is an implicit-scope system variable named `global`",
    );

    // An unrecognised `@@scope.name` scope is a clean parse error, not a misparse.
    assert!(
        parse_with("SELECT @@bogus.x", SESSION_VARIABLE_DIALECT).is_err(),
        "an unknown system-variable scope is rejected",
    );

    // The shipped MySQL preset accepts all four forms (its `named_at` is off, so
    // `@name` is a user variable, not a placeholder).
    for sql in [
        "SELECT @user_count",
        "SELECT @@max_connections",
        "SELECT @@global.time_zone",
        "SELECT @@session.sql_mode",
    ] {
        assert!(parse_with(sql, MySql).is_ok(), "MySQL parses {sql:?}",);
    }

    // Dialect-gated: ANSI lexes `@`/`@@` as stray bytes, so neither form parses.
    assert!(
        parse_with("SELECT @x", TestDialect).is_err(),
        "ANSI rejects `@x`",
    );
    assert!(
        parse_with("SELECT @@x", TestDialect).is_err(),
        "ANSI rejects `@@x`",
    );
}

#[test]
fn parser_reads_binding_powers_from_the_dialect() {
    // Standard M1: `||` is looser than `*`, so this is `a || (b * c)`.
    let standard =
        parse_with("SELECT a || b * c", TestDialect).expect("standard precedence parses");
    let Expr::BinaryOp {
        op: BinaryOperator::StringConcat,
        right,
        ..
    } = project_expr(&standard)
    else {
        panic!("standard dialect root should be concatenation");
    };
    assert!(
        matches!(
            **right,
            Expr::BinaryOp {
                op: BinaryOperator::Multiply,
                ..
            }
        ),
        "multiply binds under concat in the standard table",
    );

    // Custom dialect: `||` is tighter than `*`, so this is `(a || b) * c`.
    let custom =
        parse_with("SELECT a || b * c", HIGH_CONCAT_DIALECT).expect("custom precedence parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Multiply,
        left,
        ..
    } = project_expr(&custom)
    else {
        panic!("custom dialect root should be multiplication");
    };
    assert!(
        matches!(
            **left,
            Expr::BinaryOp {
                op: BinaryOperator::StringConcat,
                ..
            }
        ),
        "concat binds under multiply in the custom table",
    );
}

#[test]
fn not_parses_as_a_prefix_unary() {
    let parsed = parse_with("SELECT NOT a", TestDialect).expect("valid expression");
    let Expr::UnaryOp {
        op: UnaryOperator::Not,
        expr,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected a unary `NOT`");
    };
    assert_eq!(column_name(&parsed, expr), "a");
}

#[test]
fn unary_minus_wraps_its_operand() {
    // `- 1` is a unary operator over the literal, not a folded negative literal.
    let parsed = parse_with("SELECT - 1", TestDialect).expect("valid expression");
    let Expr::UnaryOp {
        op: UnaryOperator::Minus,
        expr,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected a unary minus");
    };
    assert!(
        matches!(**expr, Expr::Literal { .. }),
        "operand is the literal `1`"
    );
}

#[test]
fn cast_parses_numeric_and_character_type_names() {
    let parsed = parse_with("SELECT CAST(a AS INT)", TestDialect).expect("CAST parses");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Integer {
            spelling: IntegerTypeName::Int,
            ..
        }
    ));

    let parsed = parse_with("SELECT CAST(1 AS NUMERIC(10, 2))", TestDialect)
        .expect("NUMERIC precision and scale parse");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Decimal {
            spelling: DecimalTypeName::Numeric,
            precision: Some(10),
            scale: Some(2),
            ..
        }
    ));

    let parsed = parse_with("SELECT CAST(a AS CHARACTER VARYING(5))", TestDialect)
        .expect("CHARACTER VARYING size parses");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Character {
            spelling: CharacterTypeName::CharacterVarying,
            size: Some(5),
            ..
        }
    ));
}

#[test]
fn cast_parses_temporal_interval_and_array_type_names() {
    let parsed = parse_with("SELECT CAST(a AS TIMESTAMP(3) WITH TIME ZONE)", TestDialect)
        .expect("TIMESTAMP WITH TIME ZONE parses");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Timestamp {
            spelling: TimestampTypeName::Timestamp,
            precision: Some(3),
            time_zone: TimeZone::WithTimeZone,
            ..
        }
    ));

    let parsed = parse_with("SELECT CAST(a AS INTERVAL DAY TO SECOND(3))", TestDialect)
        .expect("INTERVAL DAY TO SECOND precision parses");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Interval {
            fields: Some(IntervalFields::DayToSecond),
            precision: Some(3),
            ..
        }
    ));

    let parsed =
        parse_with("SELECT CAST(a AS VARCHAR(5)[])", TestDialect).expect("array suffix parses");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Array { element, .. }
            if matches!(
                &**element,
                DataType::Character {
                    spelling: CharacterTypeName::Varchar,
                    size: Some(5),
                    ..
                }
            )
    ));
}

#[test]
fn cast_parses_user_defined_qualified_type_names() {
    let parsed = parse_with("SELECT CAST(a AS public.geometry(4326))", TestDialect)
        .expect("qualified user-defined type parses");
    let DataType::UserDefined {
        name, modifiers, ..
    } = cast_type(&parsed)
    else {
        panic!("expected a user-defined type");
    };

    assert_eq!(name.0.len(), 2);
    assert_eq!(parsed.resolver().resolve(name.0[0].sym), "public");
    assert_eq!(parsed.resolver().resolve(name.0[1].sym), "geometry");
    assert_eq!(modifiers.len(), 1);
    assert_eq!(modifiers[0].kind, LiteralKind::Integer);
    assert_eq!(
        modifiers[0]
            .as_i64(parsed.source())
            .expect("integer modifier"),
        4326
    );
}

#[test]
fn scalar_subquery_parses_as_expression() {
    let parsed = parse_with("SELECT (SELECT 1)", TestDialect).expect("subquery parses");
    let Expr::Subquery { query, .. } = project_expr(&parsed) else {
        panic!("expected a scalar subquery expression");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected the scalar subquery body to be SELECT");
    };
    assert_eq!(select.projection.len(), 1);
}

#[test]
fn exists_predicate_parses_as_distinct_expression() {
    let parsed = parse_with("SELECT * FROM t WHERE EXISTS (SELECT 1)", TestDialect)
        .expect("EXISTS predicate parses");
    let Expr::Exists { query, .. } = selection_expr(&parsed) else {
        panic!("expected an EXISTS predicate");
    };
    assert!(matches!(query.body, SetExpr::Select { .. }));
}

#[test]
fn special_value_functions_parse_nullary_and_precision_forms() {
    // The nullary keyword forms parse to `SpecialFunction` with no precision.
    for (sql, expected) in [
        ("SELECT CURRENT_DATE", SpecialFunctionKeyword::CurrentDate),
        ("SELECT CURRENT_USER", SpecialFunctionKeyword::CurrentUser),
        ("SELECT USER", SpecialFunctionKeyword::User),
        ("SELECT SESSION_USER", SpecialFunctionKeyword::SessionUser),
        (
            "SELECT CURRENT_CATALOG",
            SpecialFunctionKeyword::CurrentCatalog,
        ),
    ] {
        let parsed = parse_with(sql, Postgres).expect("special value function parses");
        let Expr::SpecialFunction {
            keyword,
            precision: None,
            ..
        } = project_expr(&parsed)
        else {
            panic!("expected a nullary special function for {sql}");
        };
        assert_eq!(*keyword, expected);
    }

    // The four temporal forms accept an optional `(precision)`.
    let parsed = parse_with("SELECT CURRENT_TIME(3)", Postgres).expect("precision form parses");
    let Expr::SpecialFunction {
        keyword: SpecialFunctionKeyword::CurrentTime,
        precision: Some(3),
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected CURRENT_TIME(3)");
    };

    // A nullary form rejects a trailing argument (`CURRENT_DATE(1)` is not a
    // production), matching PostgreSQL.
    assert!(parse_with("SELECT CURRENT_DATE(1)", Postgres).is_err());

    // `CURRENT_SCHEMA` is also an ordinary function name, so the call form stays
    // a generic function while the bare keyword is the special value function.
    assert!(matches!(
        project_expr(&parse_with("SELECT current_schema", Postgres).expect("bare parses")),
        Expr::SpecialFunction {
            keyword: SpecialFunctionKeyword::CurrentSchema,
            ..
        }
    ));
    assert!(matches!(
        project_expr(&parse_with("SELECT current_schema()", Postgres).expect("call parses")),
        Expr::Function { .. }
    ));
}

#[test]
fn special_value_functions_round_trip_through_rendering() {
    for sql in [
        "SELECT CURRENT_DATE",
        "SELECT CURRENT_TIMESTAMP",
        "SELECT CURRENT_TIME(3)",
        "SELECT LOCALTIMESTAMP(6)",
        "SELECT USER",
        "SELECT SESSION_USER",
    ] {
        let parsed = parse_with(sql, Postgres).expect("special value function parses");
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .expect("special value function renders");
        assert_eq!(rendered, sql);
    }
}

#[test]
fn nullif_requires_exactly_two_arguments() {
    // The valid two-argument form keeps the canonical `Function` shape.
    let parsed = parse_with("SELECT nullif(a, b)", Postgres).expect("NULLIF(a, b) parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a NULLIF function call");
    };
    assert_eq!(parsed.resolver().resolve(call.name.0[0].sym), "nullif");
    assert_eq!(call.args.len(), 2);

    // Any other arity (or shape) is rejected, matching PostgreSQL's dedicated
    // two-argument production.
    for sql in [
        "SELECT nullif(1)",
        "SELECT nullif(1, 2, 3)",
        "SELECT nullif(*)",
        "SELECT nullif(DISTINCT a, b)",
    ] {
        assert!(parse_with(sql, Postgres).is_err(), "{sql} must be rejected");
    }

    // A bare `nullif` is an ordinary column reference (it is `col_name`).
    let bare = parse_with("SELECT nullif", Postgres).expect("bare nullif parses");
    assert_eq!(column_name(&bare, project_expr(&bare)), "nullif");
}

#[test]
fn bare_exists_is_a_column_reference() {
    // `exists` is `col_name`, so without a following `(` it is a column.
    let parsed = parse_with("SELECT exists", Postgres).expect("bare exists parses");
    assert_eq!(column_name(&parsed, project_expr(&parsed)), "exists");

    // `EXISTS (<query>)` is still the subquery operator.
    assert!(matches!(
        selection_expr(
            &parse_with("SELECT * FROM t WHERE EXISTS (SELECT 1)", Postgres)
                .expect("EXISTS predicate parses"),
        ),
        Expr::Exists { .. }
    ));
}

#[test]
fn cast_parses_special_postgres_type_productions() {
    // BIT [VARYING] [(n)].
    assert!(matches!(
        cast_type(&parse_with("SELECT CAST(x AS bit)", Postgres).expect("bit parses")),
        DataType::Bit {
            varying: false,
            size: None,
            ..
        }
    ));
    assert!(matches!(
        cast_type(
            &parse_with("SELECT CAST(x AS bit varying(3))", Postgres).expect("bit varying parses")
        ),
        DataType::Bit {
            varying: true,
            size: Some(3),
            ..
        }
    ));

    // JSON is a built-in (distinct from the `jsonb` user-defined name).
    assert!(matches!(
        cast_type(&parse_with("SELECT CAST(x AS json)", Postgres).expect("json parses")),
        DataType::Json { .. }
    ));

    // UUID is a first-class built-in too — the canonical identity a type planner reads,
    // ungated like JSON and case-insensitive, rather than a `UserDefined` name.
    assert!(matches!(
        cast_type(&parse_with("SELECT CAST(x AS uuid)", Postgres).expect("uuid parses")),
        DataType::Uuid { .. }
    ));
    // A `UUID '…'` typed literal resolves the same variant as its cast target.
    assert!(matches!(
        cast_type(
            &parse_with(
                "SELECT UUID '00000000-0000-0000-0000-000000000000'",
                Postgres,
            )
            .expect("uuid typed literal parses")
        ),
        DataType::Uuid { .. }
    ));

    // NCHAR and NATIONAL CHAR[ACTER] are character types.
    for (sql, expected) in [
        ("SELECT CAST(x AS nchar)", CharacterTypeName::Nchar),
        (
            "SELECT CAST(x AS national character)",
            CharacterTypeName::NationalCharacter,
        ),
        (
            "SELECT CAST(x AS national char)",
            CharacterTypeName::NationalChar,
        ),
    ] {
        let parsed = parse_with(sql, Postgres).expect("national character type parses");
        let DataType::Character { spelling, .. } = cast_type(&parsed) else {
            panic!("expected a character type for {sql}");
        };
        assert_eq!(*spelling, expected);
    }

    // A bare `DOUBLE` (without `PRECISION`) is an ordinary user-defined type name.
    assert!(matches!(
        cast_type(&parse_with("SELECT CAST(x AS double)", Postgres).expect("bare double parses")),
        DataType::UserDefined { .. }
    ));
    // `DOUBLE PRECISION` stays the built-in.
    assert!(matches!(
        cast_type(
            &parse_with("SELECT CAST(x AS double precision)", Postgres)
                .expect("double precision parses")
        ),
        DataType::Double { .. }
    ));
}

#[test]
fn uuid_type_name_renders_canonical_uppercase() {
    // First-classing UUID makes it render as the canonical keyword `UUID` rather than the
    // source spelling a `UserDefined` name preserves: a lowercase `uuid` canonicalizes,
    // exactly like the other single-keyword built-ins (`JSON`, `DATE`).
    for (sql, expected) in [
        ("SELECT CAST(x AS UUID)", "SELECT CAST(x AS UUID)"),
        ("SELECT CAST(x AS uuid)", "SELECT CAST(x AS UUID)"),
    ] {
        let parsed =
            parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        assert!(
            matches!(cast_type(&parsed), DataType::Uuid { .. }),
            "UUID identity for {sql}",
        );
        let rendered = Renderer::new(PG_EXPR_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, expected, "canonical UUID render for {sql}");
    }
}

#[test]
fn in_subquery_predicate_preserves_negation_and_lhs() {
    let parsed = parse_with(
        "SELECT * FROM t WHERE a NOT IN (SELECT b FROM u)",
        TestDialect,
    )
    .expect("NOT IN subquery predicate parses");
    let Expr::InSubquery {
        expr,
        subquery,
        negated,
        ..
    } = selection_expr(&parsed)
    else {
        panic!("expected a NOT IN subquery predicate");
    };

    assert!(*negated);
    assert_eq!(column_name(&parsed, expr), "a");
    assert!(matches!(subquery.body, SetExpr::Select { .. }));
}

#[test]
fn quantified_comparison_parses_any_all_and_some() {
    for (sql, expected) in [
        (
            "SELECT * FROM t WHERE a = ANY (SELECT b FROM u)",
            Quantifier::Any,
        ),
        (
            "SELECT * FROM t WHERE a < ALL (SELECT b FROM u)",
            Quantifier::All,
        ),
        (
            "SELECT * FROM t WHERE a <> SOME (SELECT b FROM u)",
            Quantifier::Some,
        ),
    ] {
        let parsed = parse_with(sql, TestDialect).expect("quantified comparison parses");
        let Expr::QuantifiedComparison {
            left,
            op: _,
            quantifier,
            subquery,
            ..
        } = selection_expr(&parsed)
        else {
            panic!("expected a quantified comparison for {sql}");
        };
        assert_eq!(column_name(&parsed, left), "a");
        assert_eq!(*quantifier, expected);
        assert!(matches!(subquery.body, SetExpr::Select { .. }));
    }
}

#[test]
fn subquery_predicates_bind_at_comparison_precedence() {
    let parsed = parse_with(
        "SELECT * FROM t WHERE a = ANY (SELECT b) AND c = d",
        TestDialect,
    )
    .expect("quantified comparison and AND parse");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::And,
        right,
        ..
    } = selection_expr(&parsed)
    else {
        panic!("expected AND at the root");
    };
    assert!(
        matches!(**left, Expr::QuantifiedComparison { .. }),
        "left side is the comparison-level quantified predicate",
    );
    assert!(
        matches!(
            **right,
            Expr::BinaryOp {
                op: BinaryOperator::Eq(_),
                ..
            }
        ),
        "right side is a regular comparison",
    );
}

#[test]
fn quantified_list_parses_scalar_array_operand() {
    // DuckDB/PostgreSQL `= ANY (<list>)`: a quantified comparison over a value
    // operand (a column, a `[…]` list literal), not a subquery — the distinct
    // `QuantifiedList` node. Each round-trips verbatim under the DuckDb renderer.
    for (sql, quantifier) in [
        ("SELECT * FROM t WHERE a = ANY (b)", Quantifier::Any),
        ("SELECT * FROM t WHERE a = ANY ([1, 2, 3])", Quantifier::Any),
        ("SELECT * FROM t WHERE a < ALL (b)", Quantifier::All),
        ("SELECT * FROM t WHERE a <> SOME (b)", Quantifier::Some),
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::QuantifiedList {
            left,
            op: _,
            quantifier: parsed_quantifier,
            ..
        } = selection_expr(&parsed)
        else {
            panic!("expected a quantified-list comparison for {sql}");
        };
        assert_eq!(column_name(&parsed, left), "a");
        assert_eq!(*parsed_quantifier, quantifier);
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn quantified_like_and_arbitrary_operator_parse_and_round_trip() {
    // PostgreSQL `<expr> [NOT] LIKE|ILIKE {ANY|ALL} (<array>)` builds the distinct
    // `QuantifiedLike` node, and any operator (not only the comparisons) may take the
    // quantifier. Each round-trips verbatim under the Postgres renderer.
    for sql in [
        "SELECT * FROM t WHERE a LIKE ANY (ARRAY['%a', '%o'])",
        "SELECT * FROM t WHERE a NOT LIKE ALL (b)",
        "SELECT * FROM t WHERE a ILIKE SOME (b)",
        "SELECT * FROM t WHERE a * ANY (b) > 0",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
    // The quantified-pattern node is the one the first two build.
    let like = parse_with("SELECT * FROM t WHERE a LIKE ANY (ARRAY['%a'])", Postgres)
        .expect("LIKE ANY parses");
    assert!(
        matches!(selection_expr(&like), Expr::QuantifiedLike { .. }),
        "LIKE ANY builds the QuantifiedLike node",
    );
    // A cast of a scalar subquery is an expression operand (the list node), not a bare
    // subquery — the `(subquery)::type` disambiguation.
    let cast = parse_with(
        "SELECT * FROM t WHERE 'foo'::text = ANY ((SELECT ARRAY['a']::text[])::text[])",
        Postgres,
    )
    .expect("cast-of-subquery operand parses");
    assert!(
        matches!(selection_expr(&cast), Expr::QuantifiedList { .. }),
        "a (subquery)::type operand builds the list node, not the subquery node",
    );
}

#[test]
fn quantified_comparison_dispatch_splits_subquery_from_list_operand() {
    // The same `ANY (…)` position resolves to the subquery node when a query keyword
    // leads the parentheses and to the list node otherwise — the `IN (…)` split.
    let subquery = parse_with(
        "SELECT * FROM t WHERE a = ANY (SELECT b FROM u)",
        DUCKDB_TYPE_DIALECT,
    )
    .expect("subquery operand parses");
    assert!(
        matches!(selection_expr(&subquery), Expr::QuantifiedComparison { .. }),
        "a leading SELECT keeps the subquery node even where the list form is enabled",
    );
    let list = parse_with("SELECT * FROM t WHERE a = ANY (b)", DUCKDB_TYPE_DIALECT)
        .expect("list operand parses");
    assert!(
        matches!(selection_expr(&list), Expr::QuantifiedList { .. }),
        "a value operand builds the list node",
    );
}

#[test]
fn quantified_list_operand_rejected_without_the_gate() {
    // ANSI admits the subquery quantifier but not the list operand: the non-query
    // content surfaces as the standard "a subquery" parse error.
    assert!(
        parse_with("SELECT * FROM t WHERE a = ANY (b)", TestDialect).is_err(),
        "ANSI rejects the list-operand quantified comparison",
    );
    parse_with(
        "SELECT * FROM t WHERE a = ANY (SELECT b FROM u)",
        TestDialect,
    )
    .expect("ANSI still accepts the subquery quantifier");
}

#[test]
fn subquery_predicates_do_not_chain_with_comparisons() {
    let err = parse_with("SELECT * FROM t WHERE a IN (SELECT b) = c", TestDialect)
        .expect_err("IN predicate is non-associative with comparisons");
    assert_eq!(err.expected.as_str(), "the end of the comparison");
}

#[test]
fn comparison_parses_as_a_binary_op() {
    let parsed = parse_with("SELECT a < b", TestDialect).expect("valid expression");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Lt,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected the binary `a < b`");
    };
    assert_eq!(column_name(&parsed, left), "a");
    assert_eq!(column_name(&parsed, right), "b");
}

#[test]
fn chained_comparison_is_rejected_as_non_associative() {
    // PostgreSQL rejects the unparenthesized chain at the second operator.
    // Comparisons are non-associative (ADR-0008), so this must never silently
    // left-associate. Pinned against both the internal ANSI test harness and
    // the real Postgres preset, since MySQL now carries a per-dialect `Left`
    // override (mysql-comparison-operators-are-left-associative) that must
    // not leak into either.
    let err =
        parse_with("SELECT a < b < c", TestDialect).expect_err("comparison operators do not chain");
    // `SELECT a < b < c`: the second `<` sits at bytes 13..14.
    assert_eq!(err.span, Span::new(13, 14));

    let err = parse_with("SELECT a < b < c", Postgres)
        .expect_err("PostgreSQL comparison operators do not chain");
    assert_eq!(err.span, Span::new(13, 14));
}

#[test]
fn parenthesized_comparisons_reset_non_associative_chain_detection() {
    // Verified against libpg_query: PostgreSQL accepts both explicit
    // groupings even though it rejects `a < b < c`.
    let parsed = parse_with("SELECT (a < b) < c", TestDialect)
        .expect("parenthesized left comparison parses");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Lt,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected the outer comparison");
    };
    assert!(
        matches!(
            **left,
            Expr::BinaryOp {
                op: BinaryOperator::Lt,
                ..
            }
        ),
        "the parenthesized comparison is the left operand",
    );
    assert_eq!(column_name(&parsed, right), "c");

    let parsed = parse_with("SELECT a < (b < c)", TestDialect)
        .expect("parenthesized right comparison parses");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Lt,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected the outer comparison");
    };
    assert_eq!(column_name(&parsed, left), "a");
    assert!(
        matches!(
            **right,
            Expr::BinaryOp {
                op: BinaryOperator::Lt,
                ..
            }
        ),
        "the parenthesized comparison is the right operand",
    );
}

#[test]
fn parser_reads_comparison_associativity_from_the_dialect() {
    let parsed = parse_with("SELECT a < b < c", LEFT_ASSOC_COMPARISON_DIALECT)
        .expect("left-associative comparison dialect permits chains");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Lt,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected the outer comparison");
    };
    assert!(
        matches!(
            **left,
            Expr::BinaryOp {
                op: BinaryOperator::Lt,
                ..
            }
        ),
        "the custom dialect left-associates the comparison chain",
    );
    assert_eq!(column_name(&parsed, right), "c");
}

#[test]
fn mysql_left_associates_comparison_chains() {
    // Real MySQL parses a comparison chain left-associatively: `a < b < c`
    // means `(a < b) < c` (the boolean 0/1 result of the inner comparison
    // feeds the outer one) — where ANSI/PostgreSQL reject the same source as
    // a clean ParseError (`chained_comparison_is_rejected_as_non_associative`
    // above). `=` and `<>` share the same `comparison` binding-power row as
    // `<`, so they chain the same way
    // (mysql-comparison-operators-are-left-associative).
    for (sql, op) in [
        ("SELECT a < b < c", BinaryOperator::Lt),
        (
            "SELECT a = b = c",
            BinaryOperator::Eq(EqualsSpelling::Single),
        ),
        (
            "SELECT a <> b <> c",
            BinaryOperator::NotEq(NotEqSpelling::AngleBracket),
        ),
    ] {
        let parsed = parse_with(sql, MySql).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::BinaryOp {
            left,
            op: outer_op,
            right,
            ..
        } = project_expr(&parsed)
        else {
            panic!("{sql}: expected the outer comparison");
        };
        assert_eq!(*outer_op, op, "{sql}: outer operator");

        let Expr::BinaryOp { op: inner_op, .. } = &**left else {
            panic!("{sql}: left operand should itself be a binary comparison");
        };
        assert_eq!(
            *inner_op, op,
            "{sql}: left operand is the inner `a {op:?} b`"
        );

        assert_eq!(
            column_name(&parsed, right),
            "c",
            "{sql}: right operand is bare `c`"
        );
    }
}

#[test]
fn expression_spans_are_recoverable() {
    // The generated `Spanned` recovers a binary op's span as the union of its
    // operands' spans (the operator node carries no `Meta` of its own).
    let parsed = parse_with("SELECT 1 + 2", TestDialect).expect("valid expression");
    // `1` at byte 7, `2` at byte 11; the union covers 7..12.
    assert_eq!(project_expr(&parsed).span(), Span::new(7, 12));
}

#[test]
fn function_call_parses_name_and_arguments() {
    let parsed = parse_with("SELECT coalesce(a, b, c)", TestDialect).expect("function call parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert_eq!(parsed.resolver().resolve(call.name.0[0].sym), "coalesce");
    assert_eq!(call.args.len(), 3);
    assert!(call.quantifier.is_none());
    assert!(!call.wildcard);
    assert!(call.order_by.is_empty());
    assert!(call.filter.is_none());
}

#[test]
fn function_call_parses_empty_distinct_and_wildcard_forms() {
    let parsed = parse_with("SELECT now()", TestDialect).expect("no-arg call parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(call.args.is_empty() && !call.wildcard && call.quantifier.is_none());

    let parsed = parse_with("SELECT count(*)", TestDialect).expect("count star parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(call.wildcard && call.args.is_empty());

    let parsed =
        parse_with("SELECT count(DISTINCT a)", TestDialect).expect("distinct aggregate parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(
        matches!(call.quantifier, Some(SetQuantifier::Distinct))
            && call.args.len() == 1
            && !call.wildcard
    );
}

#[test]
fn function_call_parses_explicit_all_quantifier() {
    // `count(ALL x)` is the explicit spelling of the default aggregate
    // quantifier; it is captured so the surface round-trips.
    let parsed = parse_with("SELECT count(ALL a)", TestDialect).expect("ALL aggregate parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(
        matches!(call.quantifier, Some(SetQuantifier::All))
            && call.args.len() == 1
            && !call.wildcard
    );
}

#[test]
fn function_call_rejects_quantifier_with_wildcard() {
    // `ALL`/`DISTINCT` cannot combine with the `*` argument.
    assert!(parse_with("SELECT count(ALL *)", TestDialect).is_err());
}

#[test]
fn function_call_nests_argument_expressions() {
    let parsed = parse_with("SELECT f(a + 1, g(b))", TestDialect).expect("nested args parse");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(matches!(call.args[0].value, Expr::BinaryOp { .. }));
    assert!(matches!(call.args[1].value, Expr::Function { .. }));
}

#[test]
fn function_call_span_covers_the_whole_call() {
    // `count(a)` spans bytes 7..15 within `SELECT count(a)`.
    let parsed = parse_with("SELECT count(a)", TestDialect).expect("call parses");
    assert_eq!(project_expr(&parsed).span(), Span::new(7, 15));
}

#[test]
fn function_call_parses_order_by_modifier() {
    let parsed = parse_with("SELECT array_agg(a ORDER BY b DESC)", TestDialect)
        .expect("ordered-set aggregate parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert_eq!(call.args.len(), 1);
    assert_eq!(call.order_by.len(), 1);
    assert_eq!(call.order_by[0].asc, Some(false));
    assert!(call.filter.is_none());
}

#[test]
fn duckdb_standalone_argument_order_by_parses_and_round_trips() {
    // DuckDB lets a window/rank function carry its ordering as a bare in-parenthesis
    // `ORDER BY` with no positional argument (`rank(ORDER BY x) OVER w`); the ordering
    // lands on the same `FunctionCall.order_by` the `array_agg(x ORDER BY y)` form fills,
    // with an empty `args`. Engine-probed accept on DuckDB 1.5.4.
    for (sql, keys) in [
        ("SELECT rank(ORDER BY b DESC) OVER w", 1usize),
        ("SELECT cume_dist(ORDER BY b DESC) OVER w", 1),
        ("SELECT row_number(ORDER BY b) OVER w", 1),
        ("SELECT rank(ORDER BY b DESC, c ASC) OVER w", 2),
    ] {
        let parsed = parse_with(sql, DUCKDB_TYPE_DIALECT).expect("standalone ORDER BY parses");
        let Expr::Function { call, .. } = project_expr(&parsed) else {
            panic!("expected a function call for {sql}");
        };
        assert!(call.args.is_empty(), "no positional argument in {sql}");
        assert_eq!(call.order_by.len(), keys, "sort-key count for {sql}");
        assert!(call.over.is_some(), "OVER clause retained for {sql}");
        // Renders back with the `ORDER BY` opening the parens — no stray leading space.
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .expect("standalone ORDER BY renders");
        assert_eq!(rendered, sql, "round-trips exactly");
    }
}

#[test]
fn duckdb_standalone_argument_order_by_rejects_trailing_comma() {
    // The standalone `ORDER BY` routes through the non-trailing sort-clause parser, so a
    // trailing comma inside it rejects — matching DuckDB's parser (probed 1.5.4: every
    // `ORDER BY … ,)` position is a `Parser Error`). The over-acceptance guard: opening
    // the standalone form must not open trailing-comma tolerance in any ORDER BY list.
    for sql in [
        "SELECT rank(ORDER BY b,) OVER w",
        "SELECT rank(ORDER BY b DESC, c,) OVER w",
        "SELECT rank(ORDER BY b,,) OVER w",
        // sibling ORDER BY positions the flag must leave rejecting
        "SELECT sum(a ORDER BY b,) OVER w",
        "SELECT sum(a) OVER (ORDER BY b,)",
        "SELECT sum(a) OVER (PARTITION BY b,)",
        "SELECT a FROM t ORDER BY a,",
    ] {
        assert!(
            parse_with(sql, DUCKDB_TYPE_DIALECT).is_err(),
            "a trailing comma in an ORDER BY list must reject: {sql}",
        );
    }
}

#[test]
fn standalone_argument_order_by_is_duckdb_gated() {
    // The bare in-parenthesis `ORDER BY` (empty positional list) is a DuckDB extension:
    // standard SQL / PostgreSQL require an argument before an aggregate `ORDER BY`, so the
    // `ORDER` keyword falls into the argument grammar and the reserved word rejects. Prove
    // both the default table and a dialect override honour the gate.
    let sql = "SELECT rank(ORDER BY b DESC) OVER w";
    assert!(
        parse_with(sql, TestDialect).is_err(),
        "ANSI rejects the standalone in-argument ORDER BY",
    );
    assert!(
        parse_with(sql, Postgres).is_err(),
        "PostgreSQL rejects the standalone in-argument ORDER BY",
    );
    // The argument-then-`ORDER BY` form is unaffected — it stays accepted everywhere.
    parse_with("SELECT array_agg(a ORDER BY b)", TestDialect)
        .expect("argument-then-ORDER BY is not gated");
}

#[test]
fn group_concat_separator_parses_and_round_trips() {
    // The MySQL `SEPARATOR '<sep>'` delimiter rides inside the call parentheses,
    // after any in-parenthesis `ORDER BY`, on the shared `FunctionCall.separator`.
    let sql = "SELECT group_concat(a ORDER BY b SEPARATOR ',')";
    let parsed = parse_with(sql, MYSQL_EXPR_DIALECT).expect("GROUP_CONCAT SEPARATOR parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert_eq!(call.order_by.len(), 1);
    let separator = call.separator.as_ref().expect("a SEPARATOR delimiter");
    assert_eq!(separator.kind, LiteralKind::String);
    assert_eq!(
        Renderer::new(MYSQL_EXPR_DIALECT)
            .render_parsed(&parsed)
            .expect("SEPARATOR renders"),
        sql,
    );
}

#[test]
fn ansi_and_postgres_reject_group_concat_separator() {
    // `SEPARATOR` is gated by `group_concat_separator` (MySQL/Lenient only); elsewhere
    // it is left unconsumed and the expected closing `)` sees it as leftover -> reject.
    let sql = "SELECT group_concat(a SEPARATOR ',')";
    parse_with(sql, TestDialect).expect_err("ANSI has no GROUP_CONCAT SEPARATOR");
    parse_with(sql, Postgres).expect_err("PostgreSQL has no GROUP_CONCAT SEPARATOR");
}

#[test]
fn function_call_parses_filter_clause() {
    let parsed = parse_with("SELECT count(*) FILTER (WHERE a)", TestDialect)
        .expect("aggregate FILTER parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(call.wildcard);
    assert!(call.filter.is_some());
}

#[test]
fn duckdb_filter_omits_where_keyword() {
    // DuckDB accepts an aggregate `FILTER (…)` body without the standard `WHERE`
    // (`sum(x) FILTER (x > 1)`, probed on 1.5.4); the omission round-trips through the
    // `FilterWhereSpelling` tag. The keyword-full form still parses and is tagged as such.
    let parsed = parse_with("SELECT sum(x) FILTER (x > 1)", DuckDb)
        .expect("DuckDB accepts a keyword-less FILTER body");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(call.filter.is_some());
    assert_eq!(call.filter_where, FilterWhereSpelling::Omitted);

    let parsed = parse_with("SELECT sum(x) FILTER (WHERE x > 1)", DuckDb)
        .expect("DuckDB still accepts the standard FILTER (WHERE …)");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert_eq!(call.filter_where, FilterWhereSpelling::Where);

    // Every other dialect requires the keyword: a keyword-less body is a clean reject.
    for dialect_rejects in [
        parse_with("SELECT sum(x) FILTER (x > 1)", Postgres).is_err(),
        parse_with("SELECT sum(x) FILTER (x > 1)", Sqlite).is_err(),
    ] {
        assert!(
            dialect_rejects,
            "non-DuckDB dialects require FILTER (WHERE …)"
        );
    }

    // Both spellings round-trip exactly — no injected or dropped `WHERE`.
    for sql in [
        "SELECT sum(x) FILTER (x > 1)",
        "SELECT sum(x) FILTER (WHERE x > 1)",
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn bare_filter_after_a_call_is_not_a_bare_alias() {
    // D2 (prod-keyword-position-reserved-sets): FILTER is `AS_LABEL` in
    // PostgreSQL, so a bare `count(*) filter` is *not* an alias — it is rejected,
    // matching libpg_query. The per-position model flips the prior divergence,
    // where we accepted it.
    assert!(
        parse_with("SELECT count(*) filter", TestDialect).is_err(),
        "FILTER is AS_LABEL, so it cannot be a bare alias",
    );

    // FILTER still introduces the aggregate filter clause when `(` follows, and
    // an explicit `AS filter` alias is accepted (a ColLabel admits every keyword).
    parse_with("SELECT count(*) filter (WHERE a)", TestDialect)
        .expect("FILTER (WHERE ...) is the aggregate filter clause");
    let parsed = parse_with("SELECT count(*) AS filter", TestDialect)
        .expect("AS filter is a valid explicit alias");
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    let SelectItem::Expr {
        expr: Expr::Function { call, .. },
        alias: Some(alias),
        ..
    } = &select.projection[0]
    else {
        panic!("expected an aliased function call");
    };
    assert!(call.filter.is_none());
    assert_eq!(parsed.resolver().resolve(alias.sym), "filter");
}

#[test]
fn function_call_parses_within_group_ordered_set() {
    let parsed = parse_with(
        "SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY x DESC)",
        TestDialect,
    )
    .expect("WITHIN GROUP ordered-set aggregate parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    // The direct argument stays in `args`; the WITHIN GROUP sort key is a separate
    // clause, distinct from the in-parenthesis `order_by`.
    assert_eq!(call.args.len(), 1);
    assert!(call.order_by.is_empty());
    let within_group = call.within_group.as_ref().expect("WITHIN GROUP present");
    assert_eq!(within_group.len(), 1);
    assert_eq!(within_group[0].asc, Some(false));
}

#[test]
fn within_group_precedes_filter_and_over() {
    // PostgreSQL's grammar order is WITHIN GROUP, then FILTER, then OVER; all three
    // compose on one call in that sequence.
    let parsed = parse_with(
        "SELECT rank(a) WITHIN GROUP (ORDER BY b) FILTER (WHERE c) OVER w",
        TestDialect,
    )
    .expect("WITHIN GROUP composes before FILTER and OVER");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(call.within_group.is_some());
    assert!(call.filter.is_some());
    assert!(call.over.is_some());
}

#[test]
fn filter_before_within_group_is_rejected() {
    // The reverse order is a syntax error: WITHIN GROUP may not trail FILTER, so a
    // bare `WITHIN` after the FILTER clause is left unconsumed and rejected.
    assert!(
        parse_with(
            "SELECT rank(a) FILTER (WHERE c) WITHIN GROUP (ORDER BY b)",
            TestDialect,
        )
        .is_err(),
        "WITHIN GROUP must precede FILTER, matching PostgreSQL",
    );
}

#[test]
fn within_group_requires_the_group_keyword() {
    // Only `WITHIN GROUP` introduces the clause; a `WITHIN` not followed by `GROUP`
    // is not consumed, so the trailing token is rejected rather than swallowed.
    assert!(
        parse_with("SELECT count(x) WITHIN (ORDER BY y)", TestDialect).is_err(),
        "a bare WITHIN without GROUP does not open the ordered-set clause",
    );
}

#[test]
fn within_group_rejects_distinct_and_in_paren_order_by() {
    // PostgreSQL rejects both combinations at parse time: WITHIN GROUP shares the
    // aggregate ORDER BY slot, and an ordered-set aggregate is never DISTINCT.
    assert!(
        parse_with(
            "SELECT array_agg(x ORDER BY y) WITHIN GROUP (ORDER BY z)",
            TestDialect,
        )
        .is_err(),
        "an in-parenthesis ORDER BY cannot combine with WITHIN GROUP",
    );
    assert!(
        parse_with(
            "SELECT count(DISTINCT x) WITHIN GROUP (ORDER BY y)",
            TestDialect,
        )
        .is_err(),
        "a WITHIN GROUP ordered-set aggregate cannot be DISTINCT",
    );
    // ALL is not DISTINCT, so it composes with WITHIN GROUP just as PostgreSQL admits.
    parse_with("SELECT count(ALL x) WITHIN GROUP (ORDER BY y)", TestDialect)
        .expect("ALL composes with WITHIN GROUP");
}

#[test]
fn function_call_parses_over_inline_window() {
    let parsed = parse_with(
        "SELECT sum(a) OVER (PARTITION BY b, c ORDER BY d DESC)",
        TestDialect,
    )
    .expect("window function parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    let Some(WindowSpec::Inline { definition, .. }) = &call.over else {
        panic!("expected an inline OVER window");
    };
    assert!(definition.existing.is_none());
    assert_eq!(definition.partition_by.len(), 2);
    assert_eq!(definition.order_by.len(), 1);
    assert_eq!(definition.order_by[0].asc, Some(false));
    assert!(definition.frame.is_none());
}

#[test]
fn function_call_parses_over_named_window() {
    let parsed = parse_with("SELECT count(*) OVER w", TestDialect).expect("named window parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    let Some(WindowSpec::Named { name, .. }) = &call.over else {
        panic!("expected a named OVER window");
    };
    assert_eq!(parsed.resolver().resolve(name.sym), "w");
}

#[test]
fn window_frame_parses_between_bounds_and_exclusion() {
    let parsed = parse_with(
            "SELECT avg(a) OVER (ORDER BY b ROWS BETWEEN 1 PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE TIES)",
            TestDialect,
        )
        .expect("framed window parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    let Some(WindowSpec::Inline { definition, .. }) = &call.over else {
        panic!("expected an inline OVER window");
    };
    let frame = definition.frame.as_ref().expect("a frame clause");
    assert!(matches!(frame.units, WindowFrameUnits::Rows));
    assert!(matches!(frame.start, WindowFrameBound::Preceding { .. }));
    assert!(matches!(
        frame.end,
        Some(WindowFrameBound::UnboundedFollowing { .. })
    ));
    assert!(matches!(frame.exclusion, Some(WindowFrameExclusion::Ties)));
}

#[test]
fn window_frame_parses_bare_current_row_bound() {
    let sql = "SELECT avg(a) OVER (ORDER BY b RANGE CURRENT ROW)";
    let parsed = parse_with(sql, TestDialect).expect("bare frame bound parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    let Some(WindowSpec::Inline { definition, .. }) = &call.over else {
        panic!("expected an inline OVER window");
    };
    let frame = definition.frame.as_ref().expect("a frame clause");
    assert!(matches!(frame.units, WindowFrameUnits::Range));
    assert!(matches!(frame.start, WindowFrameBound::CurrentRow { .. }));
    let current_row_start = sql.find("CURRENT").expect("test SQL contains CURRENT") as u32;
    assert_eq!(
        frame.start.span(),
        Span::new(
            current_row_start,
            current_row_start + "CURRENT ROW".len() as u32
        ),
    );
    assert!(frame.end.is_none());
    assert!(frame.exclusion.is_none());
}

#[test]
fn window_frame_word_led_offset_is_a_value_expression() {
    // `UNBOUNDED`/`CURRENT` are non-reserved: an offset that merely starts with one of
    // those words (a function call, qualified column, or arithmetic) is a value-offset
    // bound, not the sentinel. These two forms come verbatim from PostgreSQL's own
    // `window` regression suite (pg-regress corpus), where `unbounded` is a function/column.
    for sql in [
        "SELECT sum(u) OVER (ROWS BETWEEN unbounded(1) PRECEDING AND unbounded(1) FOLLOWING) FROM t",
        "SELECT sum(u) OVER (ROWS BETWEEN unbounded.x PRECEDING AND unbounded.x FOLLOWING) FROM t",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Function { call, .. } = project_expr(&parsed) else {
            panic!("expected a function call: {sql}");
        };
        let Some(WindowSpec::Inline { definition, .. }) = &call.over else {
            panic!("expected an inline OVER window: {sql}");
        };
        let frame = definition.frame.as_ref().expect("a frame clause");
        // The word-led offset must fold into the value-offset productions, never the
        // UNBOUNDED sentinel, so the offset expression is preserved.
        assert!(
            matches!(frame.start, WindowFrameBound::Preceding { .. }),
            "start must be a value-offset PRECEDING, not the UNBOUNDED sentinel: {sql}",
        );
        assert!(
            matches!(frame.end, Some(WindowFrameBound::Following { .. })),
            "end must be a value-offset FOLLOWING, not the UNBOUNDED sentinel: {sql}",
        );
    }

    // The two-token sentinels still win by longest match: `UNBOUNDED PRECEDING` and
    // `CURRENT ROW` route to the sentinel bounds even though the words are non-reserved.
    let parsed = parse_with(
        "SELECT sum(u) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM t",
        Postgres,
    )
    .expect("sentinel frame still parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    let Some(WindowSpec::Inline { definition, .. }) = &call.over else {
        panic!("expected an inline OVER window");
    };
    let frame = definition.frame.as_ref().expect("a frame clause");
    assert!(matches!(
        frame.start,
        WindowFrameBound::UnboundedPreceding { .. }
    ));
    assert!(matches!(
        frame.end,
        Some(WindowFrameBound::CurrentRow { .. })
    ));
}

#[test]
fn window_frame_rejects_impossible_bound_ordering() {
    // The SQL-standard frame-bound order (UNBOUNDED PRECEDING < <expr> PRECEDING <
    // CURRENT ROW < <expr> FOLLOWING < UNBOUNDED FOLLOWING) makes three shape-level
    // constraints parse-checkable, all of which DuckDB, SQLite, and PostgreSQL reject at
    // parse (probed): the start may not be UNBOUNDED FOLLOWING, the end may not be
    // UNBOUNDED PRECEDING, and the start category may not follow the end category. The
    // rule is unconditional, so `TestDialect` (ANSI) exercises it.
    for sql in [
        // start = UNBOUNDED FOLLOWING (bare bound and BETWEEN forms)
        "SELECT count(*) OVER (ORDER BY b RANGE UNBOUNDED FOLLOWING)",
        "SELECT count(*) OVER (ORDER BY b RANGE BETWEEN UNBOUNDED FOLLOWING AND UNBOUNDED FOLLOWING)",
        "SELECT count(*) OVER (ORDER BY b RANGE BETWEEN UNBOUNDED FOLLOWING AND UNBOUNDED PRECEDING)",
        // end = UNBOUNDED PRECEDING
        "SELECT count(*) OVER (ORDER BY b RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED PRECEDING)",
        "SELECT count(*) OVER (ORDER BY b RANGE BETWEEN CURRENT ROW AND UNBOUNDED PRECEDING)",
        // start category after end category
        "SELECT count(*) OVER (ORDER BY b ROWS BETWEEN CURRENT ROW AND 1 PRECEDING)",
        "SELECT count(*) OVER (ORDER BY b ROWS BETWEEN 1 FOLLOWING AND CURRENT ROW)",
        "SELECT count(*) OVER (ORDER BY b ROWS BETWEEN 1 FOLLOWING AND 1 PRECEDING)",
    ] {
        assert!(
            parse_with(sql, TestDialect).is_err(),
            "an impossibly-ordered frame must reject at parse: {sql}",
        );
    }
    // Valid frames — including the two same-category offset forms whose real order is only
    // known at execution — must still parse.
    for sql in [
        "SELECT count(*) OVER (ORDER BY b RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING)",
        "SELECT count(*) OVER (ORDER BY b ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING)",
        "SELECT count(*) OVER (ORDER BY b ROWS BETWEEN 1 PRECEDING AND 2 PRECEDING)",
        "SELECT count(*) OVER (ORDER BY b ROWS BETWEEN 2 FOLLOWING AND 1 FOLLOWING)",
        "SELECT count(*) OVER (ORDER BY b RANGE UNBOUNDED PRECEDING)",
        "SELECT count(*) OVER (ORDER BY b RANGE CURRENT ROW)",
    ] {
        assert!(
            parse_with(sql, TestDialect).is_ok(),
            "a validly-ordered frame must still parse: {sql}",
        );
    }
}

#[test]
fn grouping_without_arguments_rejects_under_grouping_set_dialects() {
    // `GROUPING()` with no arguments is a parse error on the dialects that model the
    // SQL:1999 grouping-set constructs (their `GROUPING '(' expr_list ')'` grammar
    // requires a non-empty list) — DuckDB and PostgreSQL both parse-reject it — but a
    // dialect without those constructs (MySQL) treats `grouping` as an ordinary function
    // name whose empty call parses. A non-empty `GROUPING(a)` parses everywhere.
    assert!(
        parse_with("SELECT GROUPING()", Postgres).is_err(),
        "GROUPING() with no arguments must reject under a grouping-set dialect",
    );
    assert!(
        parse_with("SELECT GROUPING()", TestDialect).is_err(),
        "GROUPING() with no arguments must reject under a grouping-set dialect",
    );
    assert!(
        parse_with("SELECT GROUPING()", MySql).is_ok(),
        "GROUPING() is an ordinary empty call where grouping-set constructs are off",
    );
    assert!(
        parse_with("SELECT GROUPING(a)", Postgres).is_ok(),
        "GROUPING with an argument must parse",
    );
    // A quoted / qualified spelling defeats the reserved special form (an ordinary call).
    assert!(
        parse_with("SELECT \"grouping\"()", Postgres).is_ok(),
        "a quoted grouping() is an ordinary call, not the GROUPING special form",
    );
}

#[test]
fn select_window_clause_defines_named_windows() {
    let parsed = parse_with(
        "SELECT count(*) OVER w FROM t WINDOW w AS (PARTITION BY a ORDER BY b)",
        TestDialect,
    )
    .expect("WINDOW clause parses");
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    assert_eq!(select.windows.len(), 1);
    assert_eq!(parsed.resolver().resolve(select.windows[0].name.sym), "w");
    assert_eq!(select.windows[0].definition.partition_by.len(), 1);
    assert_eq!(select.windows[0].definition.order_by.len(), 1);
}

#[test]
fn over_definition_extends_a_base_window() {
    let parsed = parse_with(
        "SELECT count(*) OVER (w ORDER BY b) FROM t WINDOW w AS (PARTITION BY a)",
        TestDialect,
    )
    .expect("base-window reference parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    let Some(WindowSpec::Inline { definition, .. }) = &call.over else {
        panic!("expected an inline OVER window");
    };
    let existing = definition.existing.as_ref().expect("a base window name");
    assert_eq!(parsed.resolver().resolve(existing.sym), "w");
    assert_eq!(definition.order_by.len(), 1);
}

#[test]
fn frame_keywords_stay_usable_as_identifiers() {
    // The frame vocabulary is non-reserved, so these words remain valid column
    // and table names outside a window clause.
    let parsed = parse_with("SELECT partition, range, preceding FROM rows", TestDialect)
        .expect("non-reserved window keywords parse as identifiers");
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    assert_eq!(select.projection.len(), 3);
    assert_eq!(select.from.len(), 1);
}

#[test]
fn non_reserved_keyword_after_paren_parses_as_column_not_values_row() {
    // `values` is non-reserved as a column name (ADR-0011), so a `(` opening an
    // expression must read `(values + 1)` as a grouped column reference, not as
    // the head of a `VALUES` row constructor. Regression for the fully
    // parenthesized render `... WHERE ((values + 1) > 3)` of
    // `WHERE values + 1 > 3` failing to reparse (sqlglot round-trip oracle).
    let parsed = parse_with(
        "SELECT values AS values FROM t WHERE ((values + 1) > 3)",
        TestDialect,
    )
    .expect("a parenthesized non-reserved keyword parses as a column reference");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Gt,
        ..
    } = selection_expr(&parsed)
    else {
        panic!("expected the `>` comparison at the WHERE root");
    };
    let Expr::BinaryOp {
        left: inner_left,
        op: BinaryOperator::Plus,
        ..
    } = &**left
    else {
        panic!("expected the grouped `values + 1` on the left of `>`");
    };
    assert_eq!(column_name(&parsed, inner_left), "values");
}

#[test]
fn non_reserved_keyword_in_an_in_list_parses_as_column_not_values_subquery() {
    // `x IN (values, y)` is a list over the non-reserved column `values`
    // (ADR-0011), the sibling of the grouped-expression case: both gate the
    // `VALUES`-subquery arm on a following `(` via
    // `peek_starts_subquery_in_parens`, so a bare `values` after `(` stays a
    // column reference.
    let parsed = parse_with("SELECT a FROM t WHERE x IN (values, y)", TestDialect)
        .expect("a non-reserved keyword in an IN list parses as a column reference");
    let Expr::InList {
        list,
        negated: false,
        ..
    } = selection_expr(&parsed)
    else {
        panic!("expected an IN-list predicate");
    };
    assert_eq!(list.len(), 2);
    assert_eq!(column_name(&parsed, &list[0]), "values");
    assert_eq!(column_name(&parsed, &list[1]), "y");

    // The genuine `VALUES` constructor (`VALUES (`) still opens an IN subquery.
    let parsed = parse_with("SELECT a FROM t WHERE x IN (VALUES (1), (2))", TestDialect)
        .expect("a VALUES constructor still parses as an IN subquery");
    assert!(matches!(selection_expr(&parsed), Expr::InSubquery { .. }));
}

#[test]
fn window_is_reserved_but_over_is_not() {
    // Matches PostgreSQL (verified against libpg_query): WINDOW is reserved —
    // it introduces the SELECT-level window clause, so it must not be read as a
    // table alias in `FROM t WINDOW w …` — but OVER is non-reserved and stays a
    // usable identifier (it is recognized positionally as a function-call tail).
    assert!(parse_with("SELECT a FROM window", TestDialect).is_err());
    parse_with("SELECT over FROM t", TestDialect).expect("OVER is a usable column name");
    parse_with("SELECT a FROM over", TestDialect).expect("OVER is a usable table name");
}

#[test]
fn searched_case_parses_with_when_then_else() {
    let parsed = parse_with(
        "SELECT CASE WHEN a THEN b WHEN c THEN d ELSE e END",
        TestDialect,
    )
    .expect("searched CASE parses");
    let Expr::Case { case, .. } = project_expr(&parsed) else {
        panic!("expected a CASE expression");
    };
    assert!(case.operand.is_none());
    assert_eq!(case.when_clauses.len(), 2);
    assert!(case.else_result.is_some());
    assert_eq!(column_name(&parsed, &case.when_clauses[0].condition), "a");
    assert_eq!(column_name(&parsed, &case.when_clauses[0].result), "b");
}

#[test]
fn simple_case_parses_with_operand() {
    let parsed =
        parse_with("SELECT CASE a WHEN 1 THEN b END", TestDialect).expect("simple CASE parses");
    let Expr::Case { case, .. } = project_expr(&parsed) else {
        panic!("expected a CASE expression");
    };
    let operand = case.operand.as_ref().expect("simple CASE has an operand");
    assert_eq!(column_name(&parsed, operand), "a");
    assert_eq!(case.when_clauses.len(), 1);
    assert!(case.else_result.is_none());
}

#[test]
fn case_requires_at_least_one_when() {
    let err =
        parse_with("SELECT CASE a END", TestDialect).expect_err("CASE with no WHEN is rejected");
    assert_eq!(err.expected.as_str(), "`WHEN` after `CASE`");
}

#[test]
fn subscript_on_bare_case_is_rejected_but_parenthesized_is_allowed() {
    // A subscript indirection needs a parenthesized `c_expr`; a bare `CASE … END`
    // is not one, matching PostgreSQL (tighten-pg-overacceptance-trio). `Postgres`
    // enables the `[...]` subscript syntax.
    let err = parse_with("SELECT CASE 1 WHEN 1 THEN 2 ELSE 3 END['a']", Postgres)
        .expect_err("a subscript on a bare CASE is rejected");
    assert_eq!(
        err.expected.as_str(),
        "`(` around the `CASE` expression before subscripting it"
    );
    // The parenthesized form (a grouped `c_expr`) stays accepted.
    let parsed = parse_with("SELECT (CASE 1 WHEN 1 THEN 2 ELSE 3 END)['a']", Postgres)
        .expect("a subscript on a parenthesized CASE parses");
    assert!(matches!(project_expr(&parsed), Expr::Subscript { .. }));
}

#[test]
fn extract_parses_field_and_source() {
    let parsed = parse_with("SELECT EXTRACT(year FROM a)", TestDialect).expect("EXTRACT parses");
    let Expr::Extract { extract, .. } = project_expr(&parsed) else {
        panic!("expected an EXTRACT expression");
    };
    assert_eq!(parsed.resolver().resolve(extract.field.sym), "year");
    assert_eq!(column_name(&parsed, &extract.source), "a");
}

#[test]
fn bare_extract_stays_a_column() {
    // `extract` is non-reserved, so without a following `(` it is a column.
    let parsed = parse_with("SELECT extract", TestDialect).expect("extract as column parses");
    assert_eq!(column_name(&parsed, project_expr(&parsed)), "extract");
}

#[test]
fn is_null_predicate_parses_with_negation() {
    let parsed =
        parse_with("SELECT a FROM t WHERE a IS NULL", TestDialect).expect("IS NULL parses");
    let Expr::IsNull { expr, negated, .. } = selection_expr(&parsed) else {
        panic!("expected IS NULL");
    };
    assert_eq!(column_name(&parsed, expr), "a");
    assert!(!negated);

    let parsed =
        parse_with("SELECT a FROM t WHERE a IS NOT NULL", TestDialect).expect("IS NOT NULL parses");
    let Expr::IsNull { negated, .. } = selection_expr(&parsed) else {
        panic!("expected IS NOT NULL");
    };
    assert!(negated);
}

#[test]
fn between_bounds_bind_above_the_separator_and() {
    // The inner AND is the BETWEEN separator; a trailing boolean AND binds looser,
    // so this is `(a BETWEEN 1 AND 2) AND b`.
    let parsed = parse_with("SELECT a FROM t WHERE a BETWEEN 1 AND 2 AND b", TestDialect)
        .expect("BETWEEN parses");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::And,
        right,
        ..
    } = selection_expr(&parsed)
    else {
        panic!("expected a top-level boolean AND");
    };
    assert!(matches!(**left, Expr::Between { negated: false, .. }));
    assert_eq!(column_name(&parsed, right), "b");
}

#[test]
fn not_between_parses_negated() {
    let parsed = parse_with("SELECT a FROM t WHERE a NOT BETWEEN 1 AND 2", TestDialect)
        .expect("NOT BETWEEN parses");
    assert!(matches!(
        selection_expr(&parsed),
        Expr::Between { negated: true, .. }
    ));
}

#[test]
fn in_value_list_is_distinct_from_in_subquery() {
    let parsed = parse_with("SELECT a FROM t WHERE a IN (1, 2, 3)", TestDialect)
        .expect("IN value list parses");
    let Expr::InList {
        expr,
        list,
        negated,
        ..
    } = selection_expr(&parsed)
    else {
        panic!("expected an IN value list");
    };
    assert_eq!(column_name(&parsed, expr), "a");
    assert_eq!(list.len(), 3);
    assert!(!negated);

    // A query operand still parses to the subquery shape.
    let parsed = parse_with("SELECT a FROM t WHERE a IN (SELECT b FROM u)", TestDialect)
        .expect("IN subquery parses");
    assert!(matches!(selection_expr(&parsed), Expr::InSubquery { .. }));
}

#[test]
fn not_in_value_list_parses_negated() {
    let parsed = parse_with("SELECT a FROM t WHERE a NOT IN (1, 2)", TestDialect)
        .expect("NOT IN list parses");
    assert!(matches!(
        selection_expr(&parsed),
        Expr::InList { negated: true, .. }
    ));
}

#[test]
fn double_colon_cast_parses_with_syntax_tag() {
    let parsed = parse_with("SELECT a::int", PG_EXPR_DIALECT).expect("`::` cast parses");
    let Expr::Cast {
        expr,
        data_type,
        syntax,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected a `::` cast");
    };
    assert_eq!(*syntax, CastSyntax::DoubleColon);
    assert_eq!(column_name(&parsed, expr), "a");
    assert!(matches!(
        **data_type,
        DataType::Integer {
            spelling: IntegerTypeName::Int,
            ..
        }
    ));

    // ANSI does not enable the typecast operator: `::` lexes but is rejected.
    assert!(
        parse_with("SELECT a::int", TestDialect).is_err(),
        "ANSI rejects the `::` cast operator",
    );
}

#[test]
fn double_colon_cast_binds_tighter_than_arithmetic_and_unary() {
    // `a::int + b` is `(a::int) + b`: the cast binds tighter than `+`.
    let parsed = parse_with("SELECT a::int + b", PG_EXPR_DIALECT).expect("cast in addition parses");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Plus,
        ..
    } = project_expr(&parsed)
    else {
        panic!("the root operator should be `+`");
    };
    assert!(
        matches!(
            **left,
            Expr::Cast {
                syntax: CastSyntax::DoubleColon,
                ..
            }
        ),
        "the cast is the left operand of `+`",
    );

    // `- a::int` is `-(a::int)`: the cast binds tighter than the unary sign.
    let parsed = parse_with("SELECT - a::int", PG_EXPR_DIALECT).expect("unary over cast parses");
    let Expr::UnaryOp {
        op: UnaryOperator::Minus,
        expr,
        ..
    } = project_expr(&parsed)
    else {
        panic!("the root operator should be unary minus");
    };
    assert!(
        matches!(
            **expr,
            Expr::Cast {
                syntax: CastSyntax::DoubleColon,
                ..
            }
        ),
        "the unary minus wraps the cast",
    );

    // `a::int::text` is `(a::int)::text`: left-associative chained casts.
    let parsed = parse_with("SELECT a::int::text", PG_EXPR_DIALECT).expect("chained casts parse");
    let Expr::Cast { expr, .. } = project_expr(&parsed) else {
        panic!("expected the outer cast");
    };
    assert!(
        matches!(
            **expr,
            Expr::Cast {
                syntax: CastSyntax::DoubleColon,
                ..
            }
        ),
        "the inner cast is the operand of the outer cast",
    );
}

/// Root binary operator of the sole projection expression.
fn project_binary_op(parsed: &Parsed) -> BinaryOperator {
    match project_expr(parsed) {
        Expr::BinaryOp { op, .. } => op.clone(),
        other => panic!("expected a binary expression, got {other:?}"),
    }
}

#[test]
fn pg_at_family_operators_parse_to_their_binary_operators() {
    for (sql, expected) in [
        ("SELECT a @> b", BinaryOperator::Contains),
        ("SELECT a <@ b", BinaryOperator::ContainedBy),
        ("SELECT a -> b", BinaryOperator::JsonGet),
        ("SELECT a ->> b", BinaryOperator::JsonGetText),
    ] {
        let parsed =
            parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        assert_eq!(project_binary_op(&parsed), expected, "operator for {sql}");
    }
}

#[test]
fn pg_jsonb_operators_parse_to_their_binary_operators() {
    // The full PostgreSQL preset carries both `jsonb_operators` and the `#` bitwise-XOR
    // routing (`bitwise_xor: Hash`) the `#`-led members ride, so all eight lex here.
    for (sql, expected) in [
        ("SELECT a ? b", BinaryOperator::JsonExists),
        ("SELECT a ?| b", BinaryOperator::JsonExistsAny),
        ("SELECT a ?& b", BinaryOperator::JsonExistsAll),
        ("SELECT a @? b", BinaryOperator::JsonPathExists),
        ("SELECT a @@ b", BinaryOperator::JsonPathMatch),
        ("SELECT a #> b", BinaryOperator::JsonExtractPath),
        ("SELECT a #>> b", BinaryOperator::JsonExtractPathText),
        ("SELECT a #- b", BinaryOperator::JsonDeletePath),
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        assert_eq!(project_binary_op(&parsed), expected, "operator for {sql}");
    }
}

#[test]
fn pg_jsonb_operators_round_trip() {
    // Each spelling renders back to its exact source form (they fold onto no other operator).
    for sql in [
        "SELECT a ? b",
        "SELECT a ?| b",
        "SELECT a ?& b",
        "SELECT a @? b",
        "SELECT a @@ b",
        "SELECT a #> b",
        "SELECT a #>> b",
        "SELECT a #- b",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "round-trip for {sql}");
    }
}

#[test]
fn pg_jsonb_operators_sit_at_the_any_operator_rank() {
    // Engine-measured (pg_query): the `jsonb` operators bind tighter than comparison and
    // looser than additive, left-associative — the shared "any other operator" rank.
    // `a #> b = c` is `(a #> b) = c` (Op tighter than `=`).
    let eq = parse_with("SELECT a #> b = c", Postgres).expect("parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Eq(_),
        left,
        ..
    } = project_expr(&eq)
    else {
        panic!("`=` should be the root of `a #> b = c`");
    };
    assert!(matches!(
        **left,
        Expr::BinaryOp {
            op: BinaryOperator::JsonExtractPath,
            ..
        }
    ));
    // `a #> b + c` is `a #> (b + c)` (Op looser than `+`).
    let add = parse_with("SELECT a #> b + c", Postgres).expect("parses");
    let Expr::BinaryOp {
        op: BinaryOperator::JsonExtractPath,
        right,
        ..
    } = project_expr(&add)
    else {
        panic!("`#>` should be the root of `a #> b + c`");
    };
    assert!(matches!(
        **right,
        Expr::BinaryOp {
            op: BinaryOperator::Plus,
            ..
        }
    ));
    // `a #> b @@ c` is `(a #> b) @@ c` (both at the shared rank, left-associative).
    let chain = parse_with("SELECT a #> b @@ c", Postgres).expect("parses");
    let Expr::BinaryOp {
        op: BinaryOperator::JsonPathMatch,
        left,
        ..
    } = project_expr(&chain)
    else {
        panic!("`@@` should be the root of `a #> b @@ c`");
    };
    assert!(matches!(
        **left,
        Expr::BinaryOp {
            op: BinaryOperator::JsonExtractPath,
            ..
        }
    ));
}

#[test]
fn pg_hash_minus_munches_ahead_of_bitwise_xor() {
    // Engine-verified maximal munch: `5#-3` is the `jsonb` delete `5 #- 3`, while a space
    // splits `#` (bitwise XOR) from the unary `-` so `5 # -3` is XOR.
    assert!(matches!(
        project_binary_op(&parse_with("SELECT 5#-3", Postgres).expect("parses")),
        BinaryOperator::JsonDeletePath,
    ));
    assert!(matches!(
        project_binary_op(&parse_with("SELECT 5 # -3", Postgres).expect("parses")),
        BinaryOperator::BitwiseXor(_),
    ));
}

#[test]
fn pg_jsonb_operators_are_dialect_gated() {
    // With `jsonb_operators` off (ANSI carries neither these operators, an anonymous `?`
    // placeholder, a `#` comment, nor `#`-XOR routing), `?`/`#>`/`@@` are all stray bytes,
    // so each rejects. A bare `?` also rejects under PostgreSQL itself (it has no `?`
    // parameter), matching pg_query.
    use crate::dialect::Ansi;
    assert!(
        parse_with("SELECT a ? b", Ansi).is_err(),
        "`?` is not a jsonb op in ANSI"
    );
    assert!(
        parse_with("SELECT a #> b", Ansi).is_err(),
        "`#>` is not a jsonb op in ANSI"
    );
    assert!(
        parse_with("SELECT a @@ b", Ansi).is_err(),
        "`@@` is not a jsonb op in ANSI"
    );
    assert!(
        parse_with("SELECT ?", Postgres).is_err(),
        "bare `?` rejects in PostgreSQL"
    );
}

#[test]
fn pg_at_family_operators_bind_looser_than_arithmetic() {
    // PostgreSQL's "any other operator" rank is looser than `+`, so `a -> b + c` is
    // `a -> (b + c)` (the `+` binds first).
    let parsed = parse_with("SELECT a -> b + c", PG_EXPR_DIALECT).expect("json arrow parses");
    let Expr::BinaryOp {
        op: BinaryOperator::JsonGet,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("the root operator should be `->`");
    };
    assert!(
        matches!(
            **right,
            Expr::BinaryOp {
                op: BinaryOperator::Plus,
                ..
            }
        ),
        "`b + c` is the right operand of `->`",
    );
}

#[test]
fn pg_at_family_operators_are_left_associative_at_one_level() {
    // Same "any other operator" rank, left-associative: `a @> b <@ c` is
    // `(a @> b) <@ c`.
    let parsed = parse_with("SELECT a @> b <@ c", PG_EXPR_DIALECT).expect("chain parses");
    let Expr::BinaryOp {
        op: BinaryOperator::ContainedBy,
        left,
        ..
    } = project_expr(&parsed)
    else {
        panic!("the root operator should be `<@`");
    };
    assert!(
        matches!(
            **left,
            Expr::BinaryOp {
                op: BinaryOperator::Contains,
                ..
            }
        ),
        "`a @> b` is the left operand of `<@`",
    );
}

#[test]
fn pg_at_family_operators_round_trip() {
    // Each operator renders back to its exact source spelling.
    for sql in [
        "SELECT a @> b",
        "SELECT a <@ b",
        "SELECT a -> b",
        "SELECT a ->> b",
        "SELECT a -> b + c",
    ] {
        let parsed =
            parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(PG_EXPR_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "round-trip for {sql}");
    }
}

#[test]
fn pg_at_family_operators_are_inert_without_the_dialect() {
    // ANSI does not enable the containment / JSON-arrow operators, so `@>` splits
    // into a stray `@` and `<@` / `->` split back into their component bytes — every
    // form is a clean reject, mirroring how `&&` is inert under ANSI.
    for sql in [
        "SELECT a @> b",
        "SELECT a <@ b",
        "SELECT a -> b",
        "SELECT a ->> b",
    ] {
        assert!(
            parse_with(sql, TestDialect).is_err(),
            "{sql} must reject without the PostgreSQL operator flags",
        );
    }
}

#[test]
fn duckdb_composite_type_constructors_parse() {
    use crate::ast::{ArrayTypeSpelling, StructTypeSpelling};

    // STRUCT(name TYPE, ...): a named-field composite, one field per (name, type).
    let parsed = parse_with(
        "SELECT CAST(a AS STRUCT(x INTEGER, y VARCHAR))",
        DUCKDB_TYPE_DIALECT,
    )
    .expect("STRUCT type parses");
    let DataType::Struct {
        fields,
        spelling: StructTypeSpelling::Struct,
        ..
    } = cast_type(&parsed)
    else {
        panic!("expected a STRUCT data type, got {:?}", cast_type(&parsed));
    };
    assert_eq!(fields.len(), 2);
    assert!(matches!(
        &fields[0].ty,
        DataType::Integer {
            spelling: IntegerTypeName::Integer,
            ..
        }
    ));

    // ROW(...) is the same canonical Struct shape under the Row spelling tag.
    let parsed = parse_with("SELECT a::ROW(i BIGINT, j VARCHAR)", DUCKDB_TYPE_DIALECT)
        .expect("ROW type parses");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Struct {
            spelling: StructTypeSpelling::Row,
            ..
        }
    ));

    // UNION(tag T, ...): a distinct tagged-union variant sharing the field-list shape.
    let parsed = parse_with(
        "SELECT a::UNION(i SMALLINT, b VARCHAR)",
        DUCKDB_TYPE_DIALECT,
    )
    .expect("UNION type parses");
    let DataType::Union { members, .. } = cast_type(&parsed) else {
        panic!("expected a UNION data type, got {:?}", cast_type(&parsed));
    };
    assert_eq!(members.len(), 2);

    // MAP(K, V): key and value are themselves types.
    let parsed = parse_with("SELECT NULL::MAP(VARCHAR, INTEGER)", DUCKDB_TYPE_DIALECT)
        .expect("MAP type parses");
    assert!(matches!(cast_type(&parsed), DataType::Map { .. }));

    // Nested composites recurse: MAP(INTEGER[], STRUCT(x INTEGER[])).
    let parsed = parse_with(
        "SELECT NULL::MAP(INTEGER[], STRUCT(x INTEGER[]))",
        DUCKDB_TYPE_DIALECT,
    )
    .expect("nested composite parses");
    let DataType::Map { key, value, .. } = cast_type(&parsed) else {
        panic!("expected a MAP data type");
    };
    assert!(matches!(&**key, DataType::Array { .. }));
    assert!(matches!(&**value, DataType::Struct { .. }));

    // Fixed-size array `[n]` and keyword `ARRAY[n]` carry the size; `[]`/`ARRAY` do not.
    let parsed = parse_with("SELECT CAST(a AS INTEGER[3])", DUCKDB_TYPE_DIALECT)
        .expect("fixed-size array parses");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Array {
            size: Some(3),
            spelling: ArrayTypeSpelling::Bracket,
            ..
        }
    ));
    let parsed = parse_with("SELECT CAST(a AS INTEGER ARRAY[3])", DUCKDB_TYPE_DIALECT)
        .expect("keyword fixed-size array parses");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Array {
            size: Some(3),
            spelling: ArrayTypeSpelling::Keyword,
            ..
        }
    ));
    let parsed =
        parse_with("SELECT CAST(a AS INTEGER[])", DUCKDB_TYPE_DIALECT).expect("list array parses");
    assert!(matches!(
        cast_type(&parsed),
        DataType::Array {
            size: None,
            spelling: ArrayTypeSpelling::Bracket,
            ..
        }
    ));
}

#[test]
fn duckdb_try_cast_parses_as_cast_with_try_flag() {
    let parsed =
        parse_with("SELECT TRY_CAST(a AS INTEGER)", DUCKDB_TYPE_DIALECT).expect("TRY_CAST parses");
    assert!(matches!(
        project_expr(&parsed),
        Expr::Cast {
            try_cast: true,
            syntax: CastSyntax::Call,
            ..
        }
    ));
    // A plain CAST and the `::` spelling carry `try_cast: false`.
    let parsed = parse_with("SELECT CAST(a AS INTEGER)", DUCKDB_TYPE_DIALECT).expect("CAST parses");
    assert!(matches!(
        project_expr(&parsed),
        Expr::Cast {
            try_cast: false,
            ..
        }
    ));
    // A bare `TRY_CAST` with no `(` stays an ordinary column name.
    let parsed =
        parse_with("SELECT try_cast", DUCKDB_TYPE_DIALECT).expect("bare try_cast is a name");
    assert!(matches!(project_expr(&parsed), Expr::Column { .. }));
}

#[test]
fn duckdb_composite_types_and_try_cast_round_trip() {
    for sql in [
        "SELECT CAST(a AS STRUCT(x INTEGER, y VARCHAR))",
        "SELECT a::ROW(i BIGINT, j VARCHAR)",
        "SELECT a::UNION(i SMALLINT, b VARCHAR)",
        "SELECT NULL::MAP(VARCHAR, INTEGER)",
        "SELECT NULL::MAP(INTEGER[], STRUCT(x INTEGER[]))",
        "SELECT CAST(a AS INTEGER[3])",
        "SELECT CAST(a AS INTEGER ARRAY[3])",
        "SELECT CAST(a AS INTEGER[])",
        "SELECT CAST(a AS INTEGER ARRAY)",
        "SELECT CAST(a AS STRUCT(a INTEGER)[])",
        "SELECT TRY_CAST(a AS INTEGER)",
        "SELECT TRY_CAST(a AS INTEGER[3])",
        // A subscript on a `::`-cast keeps its parens: without them the following `[`
        // re-binds as an array-type suffix on the cast target (`a::INTEGER[1]` is a cast
        // to `INTEGER[1]`, not a subscript of `a::INTEGER`).
        "SELECT (a::INTEGER[])[1]",
        "SELECT (a::ROW(i BIGINT, j VARCHAR))['i']",
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "byte round-trip for {sql}");
    }
}

#[test]
fn duckdb_positional_column_reference_parses_and_round_trips() {
    // `#n` is a 1-based positional column reference (`Expr::PositionalColumn`), valid
    // wherever a value expression is — every case below was probed live on DuckDB.
    let parsed = parse_with("SELECT #1", DUCKDB_TYPE_DIALECT).expect("#1 parses");
    let Expr::PositionalColumn { index, .. } = project_expr(&parsed) else {
        panic!("expected a positional column reference");
    };
    assert_eq!(*index, 1);

    // A general primary: it composes with operators and stands in an ORDER BY item.
    for sql in [
        "SELECT #1",
        "SELECT #1 + #2",
        "SELECT a, b FROM t ORDER BY #2, #1",
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "byte round-trip for {sql}");
    }

    // DuckDB rejects `#0` at parse time ("Positional reference node needs to be >= 1");
    // our parser mirrors that with a clean error rather than a zero-index node.
    assert!(parse_with("SELECT #0", DUCKDB_TYPE_DIALECT).is_err());
    // A bare `#` with no digit is a stray byte — DuckDB's "syntax error at or near #".
    assert!(parse_with("SELECT #", DUCKDB_TYPE_DIALECT).is_err());
}

#[test]
fn positional_column_reference_rejects_without_the_dialect() {
    // The `#` lexeme is scanned only under the gate. Under ANSI, `#1` is a stray byte, so
    // the statement fails to tokenize — no dialect but DuckDB reads `#n`.
    assert!(parse_with("SELECT #1", TestDialect).is_err());
    // PostgreSQL spells `#` bitwise-XOR, and `#1` there is *not* a positional column but a
    // bare-prefix `#` operator applied to the constant `1` — PostgreSQL admits any `Op`
    // token in prefix position (`qual_Op a_expr`), and `#1` tokenizes as `#` then `1`, so
    // `SELECT #1` parses as `# 1` (engine-probed: pg_query deparses it to `SELECT # 1`),
    // never the DuckDB positional reference (pg-bare-prefix-operator-glyphs).
    let parsed = parse_with("SELECT #1", Postgres).expect("`#1` parses as a prefix operator");
    assert!(
        matches!(project_expr(&parsed), Expr::PrefixOperator { .. }),
        "`#1` under PostgreSQL is a prefix `#` operator, not a positional column",
    );
}

#[test]
fn duckdb_composite_types_and_try_cast_reject_without_the_dialect() {
    // The anonymous composite constructors and TRY_CAST are gated: under a PostgreSQL-like
    // dialect (PG cast operators, ANSI type vocabulary) they are rejected — matching a live
    // PostgreSQL server, which syntax-errors on each. `TestDialect` (plain ANSI) rejects too.
    for sql in [
        "SELECT a::STRUCT(x INTEGER)",
        "SELECT a::ROW(i BIGINT)",
        "SELECT a::UNION(i SMALLINT)",
        "SELECT a::MAP(VARCHAR, INTEGER)",
        "SELECT TRY_CAST(a AS INTEGER)",
    ] {
        assert!(
            parse_with(sql, PG_EXPR_DIALECT).is_err(),
            "{sql} must reject under the PostgreSQL-like dialect (composite_types / try_cast off)",
        );
    }
    // A bare STRUCT (no parens) is an ordinary user-defined type name even under DuckDb.
    let parsed =
        parse_with("SELECT CAST(a AS structish)", DUCKDB_TYPE_DIALECT).expect("bare name is a UDT");
    assert!(matches!(cast_type(&parsed), DataType::UserDefined { .. }));
}

#[test]
fn prefix_typed_literal_parses_as_cast_with_syntax_tag() {
    // PostgreSQL's generalized typed string constant `type 'string'` is the same
    // canonical `Expr::Cast` as `'string'::type` — a cast of the string constant to
    // the named type — bearing only the `PrefixTyped` surface tag (ADR-0011).
    let parsed = parse_with("SELECT float8 'NaN'", PG_EXPR_DIALECT).expect("typed literal parses");
    let Expr::Cast {
        expr,
        data_type,
        syntax,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected a typed-literal cast");
    };
    assert_eq!(*syntax, CastSyntax::PrefixTyped);
    // The operand is the string constant itself, not a column reference.
    let Expr::Literal { literal, .. } = &**expr else {
        panic!("the operand is a string constant");
    };
    assert!(matches!(literal.kind, LiteralKind::String));
    assert_eq!(
        literal
            .as_str(parsed.source())
            .expect("string materializes"),
        "NaN"
    );
    // A bare type alias resolves through the full type-name grammar to a
    // user-defined name — exactly what `'NaN'::float8` parses.
    let DataType::UserDefined { name, .. } = &**data_type else {
        panic!("float8 is a user-defined type name");
    };
    assert_eq!(parsed.resolver().resolve(name.0[0].sym), "float8");
}

#[test]
fn prefix_typed_literal_parses_arbitrary_type_names() {
    // Not limited to the temporal keywords: any (non-reserved) type name opens a
    // typed constant when a string follows — a bare alias, a built-in spelling, a
    // multi-word built-in, or a schema-qualified name.
    let cases: &[(&str, &str)] = &[
        ("SELECT int4 '42'", "42"),
        ("SELECT bool 'true'", "true"),
        ("SELECT real 'Infinity'", "Infinity"),
        ("SELECT double precision '1.5'", "1.5"),
        ("SELECT pg_catalog.float8 'NaN'", "NaN"),
    ];
    for (sql, value) in cases {
        let parsed =
            parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|e| panic!("{sql} parses: {e:?}"));
        let Expr::Cast { expr, syntax, .. } = project_expr(&parsed) else {
            panic!("{sql} is a typed-literal cast");
        };
        assert_eq!(*syntax, CastSyntax::PrefixTyped, "{sql}");
        let Expr::Literal { literal, .. } = &**expr else {
            panic!("{sql} operand is a string constant");
        };
        assert_eq!(
            literal.as_str(parsed.source()).expect("materializes"),
            *value,
            "{sql}",
        );
    }

    // The target type is the canonical built-in, matching the `::` form: `real`
    // and `double precision` resolve to their built-in shapes, not user-defined
    // names — so a typed literal structurally equals the corresponding cast.
    let real = parse_with("SELECT real 'Infinity'", PG_EXPR_DIALECT).expect("real parses");
    assert!(matches!(cast_type(&real), DataType::Real { .. }));
    let double =
        parse_with("SELECT double precision '1.5'", PG_EXPR_DIALECT).expect("double parses");
    assert!(matches!(cast_type(&double), DataType::Double { .. }));
}

#[test]
fn prefix_typed_literal_shares_one_shape_with_colon_and_call() {
    // The three cast spellings parse to one canonical `Expr::Cast`: identical
    // operand and target type, only the `CastSyntax` tag differs (ADR-0011). Parsed
    // in one statement so the interner is shared and the operands compare by value.
    let parsed = parse_with(
        "SELECT float8 'NaN', 'NaN'::float8, CAST('NaN' AS float8)",
        PG_EXPR_DIALECT,
    )
    .expect("all three spellings parse");
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT");
    };
    let cast = |item: &SelectItem<NoExt>| match item {
        SelectItem::Expr {
            expr:
                Expr::Cast {
                    expr,
                    data_type,
                    syntax,
                    ..
                },
            ..
        } => (expr.clone(), data_type.clone(), *syntax),
        other => panic!("expected a cast, got {other:?}"),
    };
    let (prefix_expr, prefix_type, prefix_syntax) = cast(&select.projection[0]);
    let (colon_expr, colon_type, colon_syntax) = cast(&select.projection[1]);
    let (call_expr, call_type, call_syntax) = cast(&select.projection[2]);

    // One shape: the same string operand cast to the same type (`Meta` is
    // equality-neutral, ADR-0002), the surface tag the only difference.
    assert_eq!(prefix_expr, colon_expr);
    assert_eq!(prefix_expr, call_expr);
    assert_eq!(prefix_type, colon_type);
    assert_eq!(prefix_type, call_type);
    assert_eq!(prefix_syntax, CastSyntax::PrefixTyped);
    assert_eq!(colon_syntax, CastSyntax::DoubleColon);
    assert_eq!(call_syntax, CastSyntax::Call);
}

#[test]
fn prefix_typed_literal_folds_adjacent_string_continuation() {
    // The value continues across a newline like a bare string primary (ADR-0006):
    // `float8 'x'`⏎`'y'` is the one value `xy`, matching PostgreSQL.
    let parsed =
        parse_with("SELECT float8 'x'\n'y'", PG_EXPR_DIALECT).expect("continuation parses");
    let Expr::Cast { expr, syntax, .. } = project_expr(&parsed) else {
        panic!("expected a typed-literal cast");
    };
    assert_eq!(*syntax, CastSyntax::PrefixTyped);
    let Expr::Literal { literal, .. } = &**expr else {
        panic!("operand is a string constant");
    };
    assert_eq!(literal.as_str(parsed.source()).expect("materializes"), "xy");
}

#[test]
fn prefix_typed_literal_disambiguation_and_rejects() {
    // No quotes: a type name followed by a non-string is not a typed constant and
    // not two adjacent primaries — rejected, like PostgreSQL's `func_name Sconst`.
    assert!(parse_with("SELECT float8 42", PG_EXPR_DIALECT).is_err());
    // Once the leading string commits the typed literal, a same-line second string
    // is the usual adjacency error (PostgreSQL rejects `float8 'x' 'y'` too).
    assert!(parse_with("SELECT float8 'x' 'y'", PG_EXPR_DIALECT).is_err());

    // The speculative gate must not disturb the hot paths it overlaps. An implicit
    // alias stays a column with an alias, never a typed literal.
    let aliased = parse_with("SELECT a b", PG_EXPR_DIALECT).expect("implicit alias parses");
    let Statement::Query { query, .. } = &aliased.statements()[0] else {
        panic!("expected a query");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT");
    };
    assert!(matches!(
        &select.projection[0],
        SelectItem::Expr {
            expr: Expr::Column { .. },
            alias: Some(_),
            ..
        }
    ));

    // A function call is still a call.
    let call = parse_with("SELECT count('x')", PG_EXPR_DIALECT).expect("call parses");
    assert!(matches!(project_expr(&call), Expr::Function { .. }));

    // A bare type name with no following string is an ordinary column reference.
    let col = parse_with("SELECT float8 FROM t", PG_EXPR_DIALECT).expect("bare name parses");
    assert_eq!(column_name(&col, project_expr(&col)), "float8");
}

#[test]
fn prefix_typed_literal_value_must_be_an_sconst() {
    // PG's `ConstTypename Sconst` / `func_name '(' … ')' Sconst` / `ConstDatetime Sconst`
    // productions take an `Sconst` in the value position: a bit-string (`B'1'`/`X'ab'`,
    // BCONST/XCONST) value is a distinct `bit`-typed constant PostgreSQL rejects there, so
    // the type name falls back to its bare reading and the trailing bit-string is the usual
    // adjacency parse error. Measured pg_query 6.1.1: `SELECT float8 B'1'` / `SELECT DATE
    // X'ab'` reject (18-combo matrix). All three engines that arm the prefix-typed literal
    // (pg_query, MySQL 8.4, DuckDB) reject the non-`Sconst` kinds here, so the value gate is
    // a dialect-independent classifier call (`peek_is_sconst`).
    for head in [
        "float8",
        "char(1)",
        "left(1)",
        "pg_catalog.float8",
        "DATE",
        "TIMESTAMP",
        "TIME",
        "INTERVAL",
    ] {
        for value in ["B'1'", "X'ab'"] {
            let sql = format!("SELECT {head} {value}");
            assert!(
                parse_with(&sql, PG_EXPR_DIALECT).is_err(),
                "{sql} must reject: the typed-literal value is not an Sconst"
            );
        }
        // The plain `Sconst` still folds to the typed literal (the escape/Unicode/dollar
        // Sconst spellings are exercised against the real Postgres preset in the conformance
        // differential — `PG_EXPR_DIALECT` is ANSI-based and does not arm those lexer forms).
        let sql = format!("SELECT {head} 'x'");
        assert!(
            parse_with(&sql, PG_EXPR_DIALECT).is_ok(),
            "{sql} must parse: a plain Sconst is a valid typed-literal value"
        );
    }
}

#[test]
fn parameterized_typed_literal_parses_over_modifier_list() {
    // PostgreSQL's parameterized typed string constant: a type name carrying a modifier
    // list immediately ahead of a string constant — `ConstTypename Sconst` for a built-in
    // (`char(20) 'chars'`) and `func_name '(' func_arg_list ')' Sconst` for a func-name
    // head (`foo(1) 'x'`). Both fold to the one canonical `Expr::Cast`/`PrefixTyped`, the
    // type resolving to its built-in shape exactly as the bare and `::` forms do.
    let char20 = parse_with("SELECT char(20) 'chars'", PG_EXPR_DIALECT).expect("char(20) parses");
    let Expr::Cast {
        expr,
        data_type,
        syntax,
        try_cast,
        ..
    } = project_expr(&char20)
    else {
        panic!("expected a typed-literal cast");
    };
    assert_eq!(*syntax, CastSyntax::PrefixTyped);
    assert!(!try_cast);
    // The built-in spelling resolves to its canonical DataType (with the modifier), not a
    // UserDefined name — the same shape a `'chars'::char(20)` cast produces.
    assert!(matches!(
        &**data_type,
        DataType::Character { size: Some(20), .. }
    ));
    let Expr::Literal { literal, .. } = &**expr else {
        panic!("operand is the string constant");
    };
    assert_eq!(
        literal.as_str(char20.source()).expect("materializes"),
        "chars"
    );

    // A func-name head (unreserved identifier or a `type_func_name` keyword) opens the
    // generic form; the numeric modifiers land on the UserDefined type name.
    for (sql, name) in [("SELECT foo(1) 'x'", "foo"), ("SELECT left(1) 'x'", "left")] {
        let parsed = parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
        let Expr::Cast {
            data_type, syntax, ..
        } = project_expr(&parsed)
        else {
            panic!("{sql} is a typed-literal cast");
        };
        assert_eq!(*syntax, CastSyntax::PrefixTyped, "{sql}");
        let DataType::UserDefined {
            name: type_name,
            modifiers,
            ..
        } = &**data_type
        else {
            panic!("{sql} target is a user-defined type name");
        };
        assert_eq!(parsed.resolver().resolve(type_name.0[0].sym), name, "{sql}");
        assert_eq!(modifiers.len(), 1, "{sql} carries the modifier");
        assert_eq!(
            modifiers[0]
                .as_i64(parsed.source())
                .expect("integer modifier"),
            1,
            "{sql} carries the modifier"
        );
    }

    // The canonical render round-trips: re-parsing it yields the same `PrefixTyped` cast
    // (the type name ahead of its string constant), independent of any spelling
    // normalization the render config applies to the type name itself.
    for sql in [
        "SELECT char(20) 'chars'",
        "SELECT numeric(10, 2) 'x'",
        "SELECT bit(4) 'x'",
        "SELECT foo(1) 'x'",
    ] {
        let parsed = parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
        let rendered = Renderer::new(PG_EXPR_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
        let reparsed =
            parse_with(&rendered, PG_EXPR_DIALECT).unwrap_or_else(|e| panic!("{rendered}: {e:?}"));
        assert!(
            matches!(
                project_expr(&reparsed),
                Expr::Cast {
                    syntax: CastSyntax::PrefixTyped,
                    ..
                }
            ),
            "render {rendered:?} round-trips to a prefix-typed cast",
        );
    }
}

#[test]
fn parameterized_typed_literal_boundary_and_gating() {
    // Only where PostgreSQL's grammar fires. A reserved / `col_name` keyword head is not a
    // valid `func_name`, so its call form followed by a string is not a typed literal —
    // PostgreSQL rejects these, and so must we (the type-name parse declines the head).
    for sql in [
        "SELECT substring(a) 'x'",
        "SELECT coalesce(1) 'x'",
        "SELECT int(4) 'x'",
        "SELECT integer(4) 'x'",
    ] {
        assert!(parse_with(sql, PG_EXPR_DIALECT).is_err(), "{sql} rejects");
    }

    // An ordinary call — no trailing string — is untouched by the trailing-string probe.
    for sql in [
        "SELECT foo(1)",
        "SELECT substring(a, 1)",
        "SELECT left(a, 1)",
    ] {
        let parsed = parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
        assert!(
            matches!(project_expr(&parsed), Expr::Function { .. }),
            "{sql} stays a function call",
        );
    }

    // Gated on `typed_string_literals`: SQLite has no prefix-typed literal at all, so it never
    // forms a *typed literal* here. It does, however, read `<call> '<string>'` as the call
    // aliased by the bare string (`bare_alias_string_literals`) — an accept with a different
    // meaning, engine-measured on rusqlite (`char(20) 'chars'` prepares; `foo(1) 'x'` fails only
    // to resolve `foo`). Each parses to a `Function`, never a typed-string `Literal`.
    for sql in ["SELECT char(20) 'chars'", "SELECT foo(1) 'x'"] {
        let parsed =
            parse_with(sql, Sqlite).unwrap_or_else(|e| panic!("{sql} parses under SQLite: {e:?}"));
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        // The call is aliased by the bare string, so read the item's expr (not `project_expr`,
        // which requires an unaliased item).
        let SelectItem::Expr {
            expr,
            alias: Some(_),
            ..
        } = &select.projection[0]
        else {
            panic!(
                "{sql} is a bare-aliased call, got {:?}",
                select.projection[0]
            );
        };
        assert!(
            matches!(expr, Expr::Function { .. }),
            "{sql} is a `Function` aliased by the string, not a typed-string literal",
        );
    }
}

#[test]
fn subscript_parses_index_and_slice_forms() {
    let parsed = parse_with("SELECT a[1]", PG_EXPR_DIALECT).expect("index subscript parses");
    let Expr::Subscript { subscript, .. } = project_expr(&parsed) else {
        panic!("expected a subscript");
    };
    assert_eq!(subscript.kind, SubscriptKind::Index);
    assert!(subscript.lower.is_some() && subscript.upper.is_none() && subscript.step.is_none());
    assert_eq!(column_name(&parsed, &subscript.base), "a");

    for (sql, lower, upper) in [
        ("SELECT a[1:2]", true, true),
        ("SELECT a[1:]", true, false),
        ("SELECT a[:2]", false, true),
        ("SELECT a[:]", false, false),
    ] {
        let parsed = parse_with(sql, PG_EXPR_DIALECT).expect("slice subscript parses");
        let Expr::Subscript { subscript, .. } = project_expr(&parsed) else {
            panic!("expected a slice subscript for {sql}");
        };
        assert_eq!(subscript.kind, SubscriptKind::Slice, "{sql} is a slice");
        assert_eq!(subscript.lower.is_some(), lower, "{sql} lower bound");
        assert_eq!(subscript.upper.is_some(), upper, "{sql} upper bound");
        assert!(subscript.step.is_none(), "{sql} has no step");
    }

    // An empty `[]` has no index and is rejected; ANSI rejects subscripts wholesale.
    assert!(parse_with("SELECT a[]", PG_EXPR_DIALECT).is_err());
    assert!(parse_with("SELECT a[1]", TestDialect).is_err());

    // The three-bound `[a:b:c]` slice is DuckDB-only: a two-bound dialect rejects the
    // second `:` cleanly (it is left for the `]` expectation).
    assert!(parse_with("SELECT a[1:2:3]", PG_EXPR_DIALECT).is_err());
    assert!(parse_with("SELECT a[1:-:2]", PG_EXPR_DIALECT).is_err());
}

#[test]
fn duckdb_three_bound_slice_parses_and_round_trips() {
    // (sql, lower?, upper?, step?). A `None` upper under a stepped slice is DuckDB's `-`
    // open-upper placeholder; the lower bound and the step may each be omitted. Every form
    // was accept-probed on DuckDB 1.5.4 and round-trips to its exact source spelling.
    for (sql, lower, upper, step) in [
        ("SELECT a[1:2:3]", true, true, true),
        ("SELECT a[:2:3]", false, true, true),
        ("SELECT a[1:2:]", true, true, false),
        ("SELECT a[:2:]", false, true, false),
        ("SELECT a[1:-:3]", true, false, true),
        ("SELECT a[:-:3]", false, false, true),
        ("SELECT a[1:-:]", true, false, false),
    ] {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
        let Expr::Subscript { subscript, .. } = project_expr(&parsed) else {
            panic!("expected a stepped slice for {sql}");
        };
        assert_eq!(subscript.kind, SubscriptKind::SliceWithStep, "{sql} kind");
        assert_eq!(subscript.lower.is_some(), lower, "{sql} lower");
        assert_eq!(subscript.upper.is_some(), upper, "{sql} upper");
        assert_eq!(subscript.step.is_some(), step, "{sql} step");

        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
        assert_eq!(rendered, sql, "round-trip for {sql}");
    }

    // A negative-expression bound is a bound, not the `-` placeholder: `[1:-5:2]` keeps its
    // upper (the placeholder is a bare `-` immediately before the second `:`).
    let parsed =
        parse_with("SELECT a[1:-5:2]", DUCKDB_TYPE_DIALECT).expect("negative upper parses");
    let Expr::Subscript { subscript, .. } = project_expr(&parsed) else {
        panic!("expected a stepped slice");
    };
    assert!(
        subscript.upper.is_some(),
        "-5 is a bound, not the placeholder"
    );

    // Reject boundary (all DuckDB 1.5.4 parser errors): an empty middle bound, the `-`
    // placeholder outside the middle slot, and a four-bound slice.
    for sql in [
        "SELECT a[1::2]",
        "SELECT a[::2]",
        "SELECT a[1::]",
        "SELECT a[-:2:3]",
        "SELECT a[1:2:-]",
        "SELECT a[1:-]",
        "SELECT a[1:2:3:4]",
    ] {
        assert!(
            parse_with(sql, DUCKDB_TYPE_DIALECT).is_err(),
            "{sql} must be rejected"
        );
    }
}

#[test]
fn collate_parses_with_collation_name() {
    let parsed = parse_with("SELECT a COLLATE \"C\"", PG_EXPR_DIALECT).expect("COLLATE parses");
    let Expr::Collate { collate, .. } = project_expr(&parsed) else {
        panic!("expected a COLLATE expression");
    };
    assert_eq!(column_name(&parsed, &collate.expr), "a");
    assert_eq!(collate.collation.0.len(), 1);
    assert_eq!(parsed.resolver().resolve(collate.collation.0[0].sym), "C");

    assert!(parse_with("SELECT a COLLATE \"C\"", TestDialect).is_err());
}

#[test]
fn sqlite_collate_postfix_binds_above_comparison_across_positions() {
    // SQLite spells `expr COLLATE <name>` as an expression postfix with a bare-identifier
    // collation name (`nocase`), the same shape and binding power PostgreSQL uses.
    let parsed = parse_with("SELECT a COLLATE nocase", Sqlite).expect("SQLite COLLATE parses");
    let Expr::Collate { collate, .. } = project_expr(&parsed) else {
        panic!("expected a COLLATE expression");
    };
    assert_eq!(column_name(&parsed, &collate.expr), "a");
    assert_eq!(
        parsed.resolver().resolve(collate.collation.0[0].sym),
        "nocase"
    );

    // COLLATE binds tighter than comparison: `a = b COLLATE c` is `a = (b COLLATE c)`.
    let parsed = parse_with("SELECT a = b COLLATE nocase", Sqlite).expect("precedence parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Eq(_),
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected `=` at the root, with COLLATE folded into its right operand");
    };
    assert!(
        matches!(right.as_ref(), Expr::Collate { .. }),
        "COLLATE must bind above `=`",
    );

    // The two other family positions: an ORDER BY key and a CREATE INDEX key (an index key
    // is a full expression, so the same postfix flag admits it) both parse under SQLite.
    parse_with("SELECT a FROM t ORDER BY a COLLATE nocase", Sqlite)
        .expect("ORDER BY COLLATE parses");
    parse_with("CREATE INDEX i ON t(a COLLATE nocase)", Sqlite)
        .expect("CREATE INDEX key COLLATE parses");
}

#[test]
fn at_time_zone_parses_with_zone_operand() {
    let parsed =
        parse_with("SELECT a AT TIME ZONE 'UTC'", PG_EXPR_DIALECT).expect("AT TIME ZONE parses");
    let Expr::AtTimeZone { at_time_zone, .. } = project_expr(&parsed) else {
        panic!("expected an AT TIME ZONE expression");
    };
    assert_eq!(column_name(&parsed, &at_time_zone.expr), "a");
    assert!(matches!(at_time_zone.zone, Expr::Literal { .. }));

    assert!(parse_with("SELECT a AT TIME ZONE 'UTC'", TestDialect).is_err());
}

#[test]
fn semi_structured_access_parses_path_segments_and_round_trips() {
    let sql = "SELECT src:customer[0].name";
    let parsed = parse_with(sql, SEMI_STRUCTURED_DIALECT).expect("semi-structured path parses");
    let Expr::SemiStructuredAccess {
        semi_structured_access,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected a semi-structured access expression");
    };
    assert_eq!(column_name(&parsed, &semi_structured_access.base), "src");
    assert_eq!(semi_structured_access.path.len(), 3);
    let [first, second, third] = semi_structured_access.path.as_slice() else {
        panic!("expected three path segments");
    };
    let SemiStructuredPathSegment::Key { key, .. } = first else {
        panic!("expected the colon key segment");
    };
    assert_eq!(parsed.resolver().resolve(key.sym), "customer");
    let SemiStructuredPathSegment::Index { index, .. } = second else {
        panic!("expected the bracket index segment");
    };
    assert!(matches!(&**index, Expr::Literal { .. }));
    let SemiStructuredPathSegment::Key { key, .. } = third else {
        panic!("expected the dotted key segment");
    };
    assert_eq!(parsed.resolver().resolve(key.sym), "name");

    let rendered = Renderer::new(SEMI_STRUCTURED_DIALECT)
        .render_parsed(&parsed)
        .expect("semi-structured path renders");
    assert_eq!(rendered, sql);
}

#[test]
fn semi_structured_access_precedence_and_disambiguation_are_stable() {
    let parsed =
        parse_with("SELECT a + src:customer", SEMI_STRUCTURED_DIALECT).expect("path parses");
    let Expr::BinaryOp {
        op: BinaryOperator::Plus,
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected addition with a path on the right");
    };
    assert!(matches!(&**right, Expr::SemiStructuredAccess { .. }));

    let grouped = parse_with("SELECT (a + b):customer", SEMI_STRUCTURED_DIALECT)
        .expect("grouped base parses");
    let rendered = Renderer::new(SEMI_STRUCTURED_DIALECT)
        .render_parsed(&grouped)
        .expect("grouped base renders");
    assert_eq!(rendered, "SELECT (a + b):customer");

    let cast = parse_with("SELECT src::TEXT", SEMI_STRUCTURED_POSTFIX_DIALECT)
        .expect("double-colon cast parses");
    assert!(matches!(project_expr(&cast), Expr::Cast { .. }));

    let subscript =
        parse_with("SELECT arr[1:2]", SEMI_STRUCTURED_POSTFIX_DIALECT).expect("array slice parses");
    let Expr::Subscript { subscript, .. } = project_expr(&subscript) else {
        panic!("expected a subscript slice");
    };
    assert_eq!(subscript.kind, SubscriptKind::Slice);

    let r#struct = parse_with(
        "SELECT {'customer': src:customer}",
        SEMI_STRUCTURED_POSTFIX_DIALECT,
    )
    .expect("struct key-value colon remains local to the struct parser");
    assert!(matches!(project_expr(&r#struct), Expr::Struct { .. }));
}

#[test]
fn semi_structured_access_is_gated_and_conflicts_with_named_colon_parameters() {
    assert!(parse_with("SELECT src:customer", TestDialect).is_err());
    assert!(parse_with("SELECT :customer", PARAMETER_DIALECT).is_ok());
    assert_eq!(
        FeatureSet::ANSI
            .try_with(
                FeatureDelta::EMPTY
                    .expression_syntax(ExpressionSyntax {
                        semi_structured_access: true,
                        ..ExpressionSyntax::ANSI
                    })
                    .parameters(ParameterSyntax {
                        named_colon: true,
                        ..ParameterSyntax::ANSI
                    }),
            )
            .expect_err("colon paths and colon parameters share a token trigger"),
        LexicalConflict::ColonParameterVersusSliceBound,
    );
}

#[test]
fn array_constructor_parses_elements_and_subquery() {
    let parsed =
        parse_with("SELECT ARRAY[1, 2, 3]", PG_EXPR_DIALECT).expect("array elements parse");
    let Expr::Array { array, .. } = project_expr(&parsed) else {
        panic!("expected an array constructor");
    };
    let ArrayExpr::Elements {
        elements, spelling, ..
    } = &**array
    else {
        panic!("expected an element-list array");
    };
    assert_eq!(elements.len(), 3);
    assert_eq!(*spelling, ArraySpelling::Keyword);

    let parsed =
        parse_with("SELECT ARRAY(SELECT 1)", PG_EXPR_DIALECT).expect("array subquery parses");
    let Expr::Array { array, .. } = project_expr(&parsed) else {
        panic!("expected an array constructor");
    };
    assert!(matches!(&**array, ArrayExpr::Subquery { .. }));

    // Empty `ARRAY[]` is valid; ANSI rejects the constructor and reads `array`
    // as a name (then the trailing `[...]` fails to parse).
    parse_with("SELECT ARRAY[]", PG_EXPR_DIALECT).expect("empty array parses");
    assert!(parse_with("SELECT ARRAY[1]", TestDialect).is_err());
}

#[test]
fn array_constructor_parses_multidimensional_rows() {
    // A PostgreSQL multidimensional literal: the outer `ARRAY[...]` is `Keyword`-spelled
    // and each bare-bracket sub-row is a `Bracket`-spelled element, so a nested row shapes
    // and renders like a DuckDB list level (engine-probed against pg_query 6.1 / PG-17).
    let parsed =
        parse_with("SELECT ARRAY[[1, 2], [3, 4]]", PG_EXPR_DIALECT).expect("2-D array parses");
    let Expr::Array { array, .. } = project_expr(&parsed) else {
        panic!("expected an array constructor");
    };
    let ArrayExpr::Elements {
        elements, spelling, ..
    } = &**array
    else {
        panic!("expected an element-list array");
    };
    assert_eq!(*spelling, ArraySpelling::Keyword);
    assert_eq!(elements.len(), 2);
    let Expr::Array { array: row, .. } = &elements[0] else {
        panic!("expected a nested row");
    };
    assert!(matches!(
        &**row,
        ArrayExpr::Elements {
            spelling: ArraySpelling::Bracket,
            elements,
            ..
        } if elements.len() == 2
    ));

    // Deeper nesting, empty rows, and ragged rows all parse (PostgreSQL rejects ragged
    // dimensions only at bind time, not in the grammar), and every accepted form
    // round-trips exactly.
    for sql in [
        "SELECT ARRAY[[1, 2], [3, 4]]",
        "SELECT ARRAY[[[1, 2], [3, 4]], [[5, 6], [7, 8]]]",
        "SELECT ARRAY[[1, 2], [3]]",
        "SELECT ARRAY[[], []]",
        "SELECT ARRAY[1, ARRAY[2, 3]]",
    ] {
        let parsed =
            parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(PG_EXPR_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "round-trip for {sql}");
    }

    // The uniformity rule: within a level every element is a sub-row or every element is a
    // scalar — a mix is a parse error, matching PostgreSQL's `expr_list` / `array_expr_list`
    // split. A bare `[...]` outside an array context is still not a value under this preset.
    for sql in [
        "SELECT ARRAY[[1, 2], 3]",
        "SELECT ARRAY[1, [2, 3]]",
        "SELECT ARRAY[[1, 2], ARRAY[3, 4]]",
        "SELECT ARRAY[ARRAY[1, 2], [3, 4]]",
        "SELECT [[1, 2], [3, 4]]",
    ] {
        assert!(
            parse_with(sql, PG_EXPR_DIALECT).is_err(),
            "must reject {sql}"
        );
    }
}

/// ANSI plus the DuckDB collection-literal gate and the two forms it composes
/// with — the subscript and the keyword `ARRAY[…]` constructor (so the bracket
/// and keyword spellings coexist here as they do under the DuckDb preset) —
/// isolating the new forms from the rest of that preset. Implements
/// `RenderDialect` for the exact-text round-trip checks.
const COLLECTION_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
            collection_literals: true,
            subscript: true,
            array_constructor: true,
            ..ExpressionSyntax::ANSI
        }));
    FeatureDialect {
        features: &FEATURES,
    }
};

#[test]
fn list_literal_parses_with_bracket_spelling() {
    let parsed = parse_with("SELECT [1, 2, 3]", COLLECTION_DIALECT).expect("list parses");
    let Expr::Array { array, .. } = project_expr(&parsed) else {
        panic!("expected a list literal");
    };
    let ArrayExpr::Elements {
        elements, spelling, ..
    } = &**array
    else {
        panic!("expected an element-list array");
    };
    assert_eq!(elements.len(), 3);
    assert_eq!(*spelling, ArraySpelling::Bracket);

    // Empty `[]` is valid (DuckDB accepts it; `list_value()`).
    let parsed = parse_with("SELECT []", COLLECTION_DIALECT).expect("empty list parses");
    let Expr::Array { array, .. } = project_expr(&parsed) else {
        panic!("expected a list literal");
    };
    assert!(matches!(
        &**array,
        ArrayExpr::Elements { elements, .. } if elements.is_empty()
    ));

    // DuckDB's trailing-comma tolerance is its own gate
    // (`SelectSyntax::trailing_comma`), which this collection-literal dialect leaves off,
    // so a trailing comma is still a clean reject here; the DuckDb preset accepts it
    // (`duckdb_trailing_comma_is_accepted_in_list_positions`).
    assert!(parse_with("SELECT [1, 2,]", COLLECTION_DIALECT).is_err());
}

#[test]
fn duckdb_trailing_comma_is_accepted_in_list_positions() {
    use crate::dialect::Ansi;

    // Every list position DuckDB tolerates a single trailing comma in (engine-probed on
    // 1.5.4). The comma is discarded — no AST node carries it — so each renders back
    // *without* it: rendering the trailing-comma text equals rendering the same text
    // written without the trailing comma (the canonical form), the ADR-0011 lossy-spelling
    // round-trip trade.
    let normalized_pairs = [
        ("SELECT 1, 2,", "SELECT 1, 2"),
        ("SELECT a, b, FROM t", "SELECT a, b FROM t"),
        ("VALUES (1), (2),", "VALUES (1), (2)"),
        ("VALUES (1, 2,)", "VALUES (1, 2)"),
        (
            "SELECT * FROM (VALUES (1, 2,), (3, 4,)) AS t(a, b)",
            "SELECT * FROM (VALUES (1, 2), (3, 4)) AS t(a, b)",
        ),
        (
            "SELECT a FROM VALUES (1), (2), AS t(a)",
            "SELECT a FROM VALUES (1), (2) AS t(a)",
        ),
        (
            "INSERT INTO t VALUES (1, 2,)",
            "INSERT INTO t VALUES (1, 2)",
        ),
        ("SELECT [1, 2,]", "SELECT [1, 2]"),
        ("SELECT ARRAY[1, 2,]", "SELECT ARRAY[1, 2]"),
        ("SELECT {'a': 1, 'b': 2,}", "SELECT {'a': 1, 'b': 2}"),
        ("SELECT MAP {1: 2,}", "SELECT MAP {1: 2}"),
        ("SELECT 1 IN (1, 2,)", "SELECT 1 IN (1, 2)"),
        // The `GROUP BY` key list is open-ended (no bracket closes it), so the trailing
        // comma is admitted before the clause that follows the keys — end of statement,
        // or a follower like `ORDER BY` (the `at_group_by_end` open-follower predicate).
        (
            "SELECT a, b FROM t GROUP BY a, b,",
            "SELECT a, b FROM t GROUP BY a, b",
        ),
        (
            "SELECT a FROM t GROUP BY a, ORDER BY a",
            "SELECT a FROM t GROUP BY a ORDER BY a",
        ),
        (
            "SELECT a, count(*) FROM t GROUP BY a, HAVING count(*) > 0",
            "SELECT a, count(*) FROM t GROUP BY a HAVING count(*) > 0",
        ),
        (
            "SELECT * FROM (SELECT a FROM t GROUP BY a,) x",
            "SELECT * FROM (SELECT a FROM t GROUP BY a) x",
        ),
        // …and its parenthesized `ROLLUP` / `CUBE` / `GROUPING SETS` sub-lists.
        (
            "SELECT a, b FROM t GROUP BY ROLLUP(a, b,)",
            "SELECT a, b FROM t GROUP BY ROLLUP(a, b)",
        ),
        (
            "SELECT a, b FROM t GROUP BY CUBE(a, b,)",
            "SELECT a, b FROM t GROUP BY CUBE(a, b)",
        ),
        (
            "SELECT a, b FROM t GROUP BY GROUPING SETS ((a), (b),)",
            "SELECT a, b FROM t GROUP BY GROUPING SETS ((a), (b))",
        ),
        // The wildcard-modifier lists — `EXCLUDE`, `REPLACE`, and `RENAME` all route
        // through the one parenthesized-list helper.
        (
            "SELECT * EXCLUDE (a, b,) FROM t",
            "SELECT * EXCLUDE (a, b) FROM t",
        ),
        (
            "SELECT * REPLACE (b + 1 AS b,) FROM t",
            "SELECT * REPLACE (b + 1 AS b) FROM t",
        ),
        (
            "SELECT * RENAME (a AS x,) FROM t",
            "SELECT * RENAME (a AS x) FROM t",
        ),
        // The `COALESCE` special form (a grammar special form in DuckDB, not an ordinary
        // call), for both a multi- and single-argument list.
        ("SELECT coalesce(1, 2,)", "SELECT coalesce(1, 2)"),
        ("SELECT coalesce(1,)", "SELECT coalesce(1)"),
        // The `SELECT DISTINCT ON (…)` key list — a distinct list position from the
        // shared `parse_comma_separated_exprs` (which the `PARTITION BY` / `PIVOT` callers
        // keep rejecting the comma through), so the tolerance is opted in at that site
        // only.
        (
            "SELECT DISTINCT ON (a,) a FROM t",
            "SELECT DISTINCT ON (a) a FROM t",
        ),
        (
            "SELECT DISTINCT ON (a, b,) a FROM t",
            "SELECT DISTINCT ON (a, b) a FROM t",
        ),
    ];
    for (with_tc, without_tc) in normalized_pairs {
        let with_rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(
                &parse_with(with_tc, DUCKDB_TYPE_DIALECT)
                    .unwrap_or_else(|err| panic!("{with_tc}: {err:?}")),
            )
            .unwrap_or_else(|err| panic!("{with_tc}: {err}"));
        let without_rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(
                &parse_with(without_tc, DUCKDB_TYPE_DIALECT)
                    .unwrap_or_else(|err| panic!("{without_tc}: {err:?}")),
            )
            .unwrap_or_else(|err| panic!("{without_tc}: {err}"));
        assert_eq!(
            with_rendered, without_rendered,
            "the trailing comma must normalize away for {with_tc:?}",
        );
        // ANSI, which leaves the gate off, rejects the dangling comma outright.
        assert!(
            parse_with(with_tc, Ansi).is_err(),
            "ANSI must reject the trailing comma in {with_tc:?}",
        );
    }

    // DuckDB does NOT extend the tolerance to function-argument lists, a bare
    // parenthesized / row constructor, `ORDER BY`, the `PARTITION BY` window list, or the
    // `INSERT` *column* list, nor to a second (double) trailing comma — each is
    // engine-rejected on 1.5.4, so the parser must reject them even with the gate on (the
    // over-acceptance guard the differential relies on).
    for sql in [
        "SELECT greatest(1, 2,)",
        // `COALESCE` alone among the built-ins is a trailing-comma-tolerant special form:
        // its siblings, and a quoted `"coalesce"` (an ordinary call, not the keyword
        // form), keep rejecting the comma (engine-probed on 1.5.4).
        "SELECT nullif(1, 2,)",
        "SELECT \"coalesce\"(1, 2,)",
        "SELECT (1, 2,)",
        "SELECT a FROM t ORDER BY a,",
        // `PARTITION BY` shares `parse_comma_separated_exprs` with `DISTINCT ON`; only the
        // `DISTINCT ON` site opts into the tolerance, so this route must keep rejecting.
        "SELECT a, row_number() OVER (PARTITION BY a,) FROM t",
        "INSERT INTO t (a, b,) VALUES (1, 2)",
        "SELECT 1, 2, ,",
        "SELECT [1, 2, ,]",
        "SELECT [,]",
        // A *second* (double) trailing comma stays a parse error at every accepting site.
        "SELECT a, b FROM t GROUP BY a, b,,",
        "SELECT * EXCLUDE (a,,) FROM t",
        "SELECT coalesce(1, 2,,)",
        "SELECT DISTINCT ON (a,,) a FROM t",
    ] {
        assert!(
            parse_with(sql, DUCKDB_TYPE_DIALECT).is_err(),
            "DuckDB rejects the trailing comma here, so the parser must too: {sql:?}",
        );
    }
}

#[test]
fn struct_literal_parses_fields_with_each_key_spelling() {
    let parsed = parse_with("SELECT {'a': 1, b: 2, \"c d\": [3]}", COLLECTION_DIALECT)
        .expect("struct parses");
    let Expr::Struct { r#struct, .. } = project_expr(&parsed) else {
        panic!("expected a struct literal");
    };
    let keys: Vec<_> = r#struct
        .fields
        .iter()
        .map(|field| (parsed.resolver().resolve(field.key), field.key_spelling))
        .collect();
    assert_eq!(
        keys,
        [
            ("a", StructKeySpelling::SingleQuoted),
            ("b", StructKeySpelling::Bare),
            ("c d", StructKeySpelling::DoubleQuoted),
        ],
    );
    assert!(matches!(r#struct.fields[2].value, Expr::Array { .. }));

    // A single-quoted key unescapes its doubled quote like a string body.
    let parsed = parse_with("SELECT {'it''s': 1}", COLLECTION_DIALECT).expect("escaped key");
    let Expr::Struct { r#struct, .. } = project_expr(&parsed) else {
        panic!("expected a struct literal");
    };
    assert_eq!(parsed.resolver().resolve(r#struct.fields[0].key), "it's");

    // DuckDB rejects an empty struct, a value-position key, and a reserved bare
    // key; so do we, as clean parse errors.
    assert!(parse_with("SELECT {}", COLLECTION_DIALECT).is_err());
    assert!(parse_with("SELECT {1: 'x'}", COLLECTION_DIALECT).is_err());
    assert!(parse_with("SELECT {select: 1}", COLLECTION_DIALECT).is_err());
}

#[test]
fn map_literal_parses_entries_and_bare_map_stays_a_name() {
    let parsed = parse_with("SELECT MAP {'a': 1, [2]: 'x'}", COLLECTION_DIALECT)
        .expect("map literal parses");
    let Expr::Map { map, .. } = project_expr(&parsed) else {
        panic!("expected a map literal");
    };
    assert_eq!(map.entries.len(), 2);
    // Map keys are value expressions, not field names.
    assert!(matches!(map.entries[0].key, Expr::Literal { .. }));
    assert!(matches!(map.entries[1].key, Expr::Array { .. }));

    // Empty `MAP {}` is valid (DuckDB accepts it; `map()`).
    let parsed = parse_with("SELECT MAP {}", COLLECTION_DIALECT).expect("empty map parses");
    let Expr::Map { map, .. } = project_expr(&parsed) else {
        panic!("expected a map literal");
    };
    assert!(map.entries.is_empty());

    // Without a following `{`, `map` is an ordinary name: the two-list
    // `MAP(<keys>, <values>)` spelling is a plain call (DuckDB's own treatment —
    // `map` is a case-insensitive function there), and a bare `map` a column.
    let parsed = parse_with("SELECT MAP([1], [2])", COLLECTION_DIALECT).expect("call parses");
    assert!(matches!(project_expr(&parsed), Expr::Function { .. }));
    let parsed = parse_with("SELECT map FROM t", COLLECTION_DIALECT).expect("column parses");
    assert!(matches!(project_expr(&parsed), Expr::Column { .. }));
}

#[test]
fn collection_literals_nest_and_compose_with_subscript() {
    // The ticket's sampled sweep example: structs nested in a list.
    let parsed = parse_with("SELECT [{'a': 42}, {'b': 84}]", COLLECTION_DIALECT)
        .expect("nested collections parse");
    let Expr::Array { array, .. } = project_expr(&parsed) else {
        panic!("expected a list literal");
    };
    let ArrayExpr::Elements { elements, .. } = &**array else {
        panic!("expected an element-list array");
    };
    assert!(
        elements
            .iter()
            .all(|element| matches!(element, Expr::Struct { .. }))
    );

    // A list literal is a subscriptable base: `[...]` in postfix position is the
    // subscript (index or slice), disambiguated from a fresh literal by having a
    // base to its left.
    for (sql, kind) in [
        ("SELECT [1, 2, 3][2]", SubscriptKind::Index),
        ("SELECT [1, 2][1:2]", SubscriptKind::Slice),
    ] {
        let parsed = parse_with(sql, COLLECTION_DIALECT).expect("subscripted list parses");
        let Expr::Subscript { subscript, .. } = project_expr(&parsed) else {
            panic!("expected a subscript over the list literal for {sql}");
        };
        assert_eq!(subscript.kind, kind, "{sql}");
        assert!(matches!(subscript.base, Expr::Array { .. }), "{sql}");
    }
}

#[test]
fn collection_literals_round_trip_exactly() {
    for sql in [
        "SELECT [1, 2, 3]",
        "SELECT []",
        "SELECT {'a': 1, b: 2, \"c d\": 3}",
        "SELECT {'it''s': 1}",
        "SELECT MAP {'a': 1}",
        "SELECT MAP {}",
        "SELECT MAP {[1, 2]: 'x'}",
        "SELECT [{'a': 42}, {'b': 84}]",
        "SELECT [1, 2, 3][2]",
        "SELECT ARRAY[1, 2]",
    ] {
        let parsed = parse_with(sql, COLLECTION_DIALECT).unwrap_or_else(|err| {
            panic!("{sql}: {err:?}");
        });
        let rendered = Renderer::new(COLLECTION_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn collection_literals_are_dialect_gated() {
    // Ansi, PostgreSQL, and MySQL reject every DuckDB collection spelling as a
    // clean parse error: `[`/`{` in primary position have no reading there, and
    // `MAP` falls back to a name that leaves the `{` as leftover input. The
    // ticket's trap case `[a]` is covered by the first reject; SQLite's `[`-quote
    // reading is the separate no-bleed test below.
    for sql in [
        "SELECT [1, 2, 3]",
        "SELECT [a]",
        "SELECT {'a': 1}",
        "SELECT MAP {'a': 1}",
    ] {
        assert!(parse_with(sql, TestDialect).is_err(), "ANSI rejects {sql}");
        assert!(
            parse_with(sql, Postgres).is_err(),
            "PostgreSQL rejects {sql}",
        );
        assert!(parse_with(sql, MySql).is_err(), "MySQL rejects {sql}");
    }
}

#[test]
fn bracket_identifiers_do_not_bleed_into_collection_literals() {
    use crate::dialect::{DuckDb, Sqlite};

    // SQLite's `[` is an identifier quote: `[1, 2, 3]` is a (weird) column named
    // `1, 2, 3`, never a list literal — the tokenizer claims `[` before the
    // expression grammar can (LexicalConflict::BracketIdentifierVersusArraySyntax
    // keeps the two disjoint per dialect).
    let parsed = parse_with("SELECT [1, 2, 3]", Sqlite).expect("bracket identifier parses");
    let Expr::Column { name, .. } = project_expr(&parsed) else {
        panic!("expected a bracket-quoted column identifier under SQLite");
    };
    assert_eq!(parsed.resolver().resolve(name.0[0].sym), "1, 2, 3");
    // The brace/MAP forms have no SQLite reading at all.
    assert!(parse_with("SELECT {'a': 1}", Sqlite).is_err());
    assert!(parse_with("SELECT MAP {'a': 1}", Sqlite).is_err());

    // And under the fitted DuckDb preset the same text is the list literal.
    let parsed = parse_with("SELECT [1, 2, 3]", DuckDb).expect("list literal parses");
    assert!(matches!(project_expr(&parsed), Expr::Array { .. }));
}

#[test]
fn overlaps_period_predicate_parses_row_operands() {
    // The SQL-standard `(s1, e1) OVERLAPS (s2, e2)` predicate folds onto `Expr::BinaryOp`
    // with `BinaryOperator::Overlaps`, both operands two-element rows (engine-probed
    // against pg_query 6.1 / PG-17).
    let parsed =
        parse_with("SELECT (a, b) OVERLAPS (c, d)", OVERLAPS_DIALECT).expect("OVERLAPS parses");
    let Expr::BinaryOp {
        left, op, right, ..
    } = project_expr(&parsed)
    else {
        panic!("expected a binary OVERLAPS predicate");
    };
    assert_eq!(*op, BinaryOperator::Overlaps);
    assert!(matches!(&**left, Expr::Row { row, .. } if row.fields.len() == 2));
    assert!(matches!(&**right, Expr::Row { row, .. } if row.fields.len() == 2));

    // The accepted spellings — bare pair, explicit `ROW(...)`, mixed, prefix `NOT`, and a
    // looser operator folding onto the boolean result — all round-trip exactly. PostgreSQL
    // binds `OVERLAPS` tighter than the comparisons, so `= TRUE` groups on the result, and
    // prefix `NOT` binds looser than the predicate.
    for sql in [
        "SELECT (a, b) OVERLAPS (c, d)",
        "SELECT ROW(a, b) OVERLAPS ROW(c, d)",
        "SELECT (a, b) OVERLAPS ROW(c, d)",
        "SELECT ROW(a, b) OVERLAPS (c, d)",
        "SELECT NOT (a, b) OVERLAPS (c, d)",
        "SELECT (a, b) OVERLAPS (c, d) AND (e, f) OVERLAPS (g, h)",
        "SELECT (a, b) OVERLAPS (c, d) = TRUE",
    ] {
        let parsed =
            parse_with(sql, OVERLAPS_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(OVERLAPS_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "round-trip for {sql}");
    }

    // The operand-shape and chaining rejects (engine-probed: PostgreSQL rejects each at
    // parse). Both operands must be direct two-element rows: a scalar, a single-element
    // grouping `(a)`, a re-parenthesized row `((a, b))`, or a wrong-arity row is a parse
    // error, as is an infix `NOT OVERLAPS` and any chain (the boolean result is not a row).
    for sql in [
        "SELECT a OVERLAPS b",
        "SELECT (a) OVERLAPS (b)",
        "SELECT (a, b, c) OVERLAPS (d, e, f)",
        "SELECT (a, b) OVERLAPS (c, d, e)",
        "SELECT (a, b) NOT OVERLAPS (c, d)",
        "SELECT (a, b) OVERLAPS (c, d) OVERLAPS (e, f)",
        "SELECT ((a, b) OVERLAPS (c, d)) OVERLAPS (e, f)",
        "SELECT ((a, b)) OVERLAPS (c, d)",
        "SELECT (a, b) OVERLAPS ((c, d))",
        "SELECT (a, b) OVERLAPS c",
        "SELECT a OVERLAPS (c, d)",
    ] {
        assert!(
            parse_with(sql, OVERLAPS_DIALECT).is_err(),
            "must reject {sql}"
        );
    }

    // Gated by the behaviour flag: a preset with the row constructor but not the predicate
    // (PostgreSQL expression syntax, ANSI predicate syntax) leaves `OVERLAPS` unconsumed
    // and rejects the same input.
    assert!(parse_with("SELECT (a, b) OVERLAPS (c, d)", PG_EXPR_DIALECT).is_err());
}

#[test]
fn row_constructor_parses_explicit_and_implicit_forms() {
    let parsed = parse_with("SELECT ROW(1, 2)", PG_EXPR_DIALECT).expect("explicit ROW parses");
    let Expr::Row { row, .. } = project_expr(&parsed) else {
        panic!("expected a row constructor");
    };
    assert!(row.explicit);
    assert_eq!(row.fields.len(), 2);

    let parsed = parse_with("SELECT (a, b, c)", PG_EXPR_DIALECT).expect("implicit row parses");
    let Expr::Row { row, .. } = project_expr(&parsed) else {
        panic!("expected an implicit row constructor");
    };
    assert!(!row.explicit);
    assert_eq!(row.fields.len(), 3);

    // A single parenthesized expression stays a bare grouping, not a 1-row.
    let parsed = parse_with("SELECT (a)", PG_EXPR_DIALECT).expect("grouping parses");
    assert!(matches!(project_expr(&parsed), Expr::Column { .. }));

    // ANSI rejects the implicit comma form.
    assert!(parse_with("SELECT (a, b)", TestDialect).is_err());
}

/// The PostgreSQL expression surface plus the DuckDB lambda gate and the DuckDB
/// `->` re-rank — the exact composition the DuckDb preset uses for `->`
/// (`json_arrow_operators` lexes the token, `lambda_expressions` splits the node
/// by LHS shape, and the binding-power delta ranks the token below every
/// expression operator), isolated from the rest of that preset. Implements
/// `RenderDialect` for the exact-text round-trips.
const LAMBDA_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet = FeatureSet::ANSI.with(
        FeatureDelta::EMPTY
            .expression_syntax(ExpressionSyntax::POSTGRES)
            .operator_syntax(OperatorSyntax {
                lambda_expressions: true,
                ..OperatorSyntax::POSTGRES
            })
            .call_syntax(CallSyntax::POSTGRES)
            .string_func_forms(StringFuncForms::POSTGRES)
            .aggregate_call_syntax(AggregateCallSyntax::POSTGRES)
            .binding_powers(crate::ast::precedence::STANDARD_BINDING_POWERS.with_binary(
                &BinaryOperator::JsonGet,
                BindingPower {
                    left: 4,
                    right: 5,
                    assoc: Assoc::Left,
                },
            )),
    );
    FeatureDialect {
        features: &FEATURES,
    }
};

/// The projection's lambda, or a panic naming what was there instead.
fn project_lambda<'a>(parsed: &'a Parsed, sql: &str) -> &'a crate::ast::LambdaExpr<NoExt> {
    match project_expr(parsed) {
        Expr::Lambda { lambda, .. } => lambda,
        other => panic!("expected a lambda for {sql:?}, got {other:?}"),
    }
}

#[test]
fn lambda_parses_each_param_spelling() {
    use crate::ast::LambdaParamSpelling;
    // Every parameter-list spelling DuckDB's binder admits (probed on 1.5.4:
    // `(x) -> x + 1`, `ROW(x, y) -> x + y`, and a quoted `"x"` all evaluate).
    for (sql, names, spelling) in [
        ("SELECT x -> x + 1", &["x"][..], LambdaParamSpelling::Bare),
        (
            "SELECT (x) -> x + 1",
            &["x"][..],
            LambdaParamSpelling::Parenthesized,
        ),
        (
            "SELECT (x, y) -> x + y",
            &["x", "y"][..],
            LambdaParamSpelling::Parenthesized,
        ),
        (
            "SELECT ROW(x, y) -> x + y",
            &["x", "y"][..],
            LambdaParamSpelling::RowKeyword,
        ),
    ] {
        let parsed = parse_with(sql, LAMBDA_DIALECT).unwrap_or_else(|err| {
            panic!("{sql}: {err:?}");
        });
        let lambda = project_lambda(&parsed, sql);
        let params: Vec<_> = lambda
            .params
            .iter()
            .map(|param| parsed.resolver().resolve(param.sym))
            .collect();
        assert_eq!(params, names, "params for {sql:?}");
        assert_eq!(lambda.spelling, spelling, "spelling for {sql:?}");
    }

    // A quoted parameter keeps its quote style on the ident.
    let parsed =
        parse_with("SELECT \"x\" -> \"x\" + 1", LAMBDA_DIALECT).expect("quoted param parses");
    let lambda = project_lambda(&parsed, "quoted");
    assert_eq!(lambda.params[0].quote, crate::ast::QuoteStyle::Double);
}

#[test]
fn lambda_body_captures_the_full_right_expression() {
    // `->` binds at the JSON-arrow rank, looser than arithmetic and comparison,
    // so the body swallows `x % 2 = 0` whole — the corpus `list_filter` shape.
    let parsed = parse_with("SELECT x -> x % 2 = 0", LAMBDA_DIALECT).expect("lambda body parses");
    let lambda = project_lambda(&parsed, "body");
    assert!(
        matches!(
            lambda.body,
            Expr::BinaryOp {
                op: BinaryOperator::Eq(_),
                ..
            }
        ),
        "the comparison is inside the body, got {:?}",
        lambda.body,
    );
}

#[test]
fn non_param_shaped_arrows_stay_the_json_operator() {
    // The other half of the split (each LHS probed: DuckDB's binder rejects it as
    // lambda parameters and re-reads the arrow as JSON extraction): a qualified
    // name, a constant, a computed grouping, a non-name row field, and the empty
    // `ROW()` all keep the `JsonGet` fold even with the lambda gate on.
    for sql in [
        "SELECT t.a -> 'k'",
        "SELECT 1 -> 2",
        "SELECT (a + 1) -> b",
        "SELECT (a, b.c) -> d",
        "SELECT ROW() -> 1",
    ] {
        let parsed = parse_with(sql, LAMBDA_DIALECT).unwrap_or_else(|err| {
            panic!("{sql}: {err:?}");
        });
        assert!(
            matches!(
                project_expr(&parsed),
                Expr::BinaryOp {
                    op: BinaryOperator::JsonGet,
                    ..
                }
            ),
            "expected the JSON arrow for {sql:?}, got {:?}",
            project_expr(&parsed),
        );
    }
}

#[test]
fn chained_arrows_left_associate_with_only_the_first_a_lambda() {
    // `x -> y -> z` left-associates like the JSON arrow (DuckDB serializes it as
    // LAMBDA{LAMBDA{x, y}, z} and its binder rejects the outer as lambda params),
    // so the inner `x -> y` is the lambda and the outer fold — whose LHS is now a
    // lambda, not a name — stays `JsonGet`.
    let parsed = parse_with("SELECT x -> y -> z", LAMBDA_DIALECT).expect("chain parses");
    let Expr::BinaryOp {
        op: BinaryOperator::JsonGet,
        left,
        ..
    } = project_expr(&parsed)
    else {
        panic!(
            "expected the outer JSON fold, got {:?}",
            project_expr(&parsed)
        );
    };
    assert!(matches!(**left, Expr::Lambda { .. }));
}

#[test]
fn lambda_round_trips_exactly_and_stays_json_without_the_gate() {
    for sql in [
        "SELECT x -> x + 1",
        "SELECT (x) -> x + 1",
        "SELECT (x, y) -> x + y",
        "SELECT ROW(x, y) -> x + y",
        "SELECT \"x\" -> \"x\" + 1",
        "SELECT t.a -> 'k'",
        "SELECT x -> y -> z",
        "SELECT x -> x % 2 = 0",
    ] {
        let parsed = parse_with(sql, LAMBDA_DIALECT).unwrap_or_else(|err| {
            panic!("{sql}: {err:?}");
        });
        let rendered = Renderer::new(LAMBDA_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }

    // With the gate off (plain PostgreSQL surface), the same texts parse — as
    // JSON-arrow folds — and still round-trip byte-identically, so the split is
    // purely a node-label change, never an acceptance or spelling change.
    for sql in ["SELECT x -> x + 1", "SELECT (x, y) -> x + y"] {
        let parsed = parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|err| {
            panic!("{sql}: {err:?}");
        });
        assert!(matches!(
            project_expr(&parsed),
            Expr::BinaryOp {
                op: BinaryOperator::JsonGet,
                ..
            }
        ));
        let rendered = Renderer::new(PG_EXPR_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn field_selection_parses_after_parenthesized_base() {
    let parsed = parse_with("SELECT (a).b", PG_EXPR_DIALECT).expect("field selection parses");
    let Expr::FieldSelection {
        field_selection, ..
    } = project_expr(&parsed)
    else {
        panic!("expected a field selection");
    };
    assert_eq!(column_name(&parsed, &field_selection.base), "a");
    let FieldSelector::Field { field, .. } = &field_selection.selector else {
        panic!("expected a named field selector");
    };
    assert_eq!(parsed.resolver().resolve(field.sym), "b");

    // A bare `a.b` stays a qualified column reference (the dot is consumed during
    // name parsing), so field selection only applies to a parenthesized base.
    let parsed = parse_with("SELECT a.b", PG_EXPR_DIALECT).expect("qualified column parses");
    let Expr::Column { name, .. } = project_expr(&parsed) else {
        panic!("expected a qualified column");
    };
    assert_eq!(name.0.len(), 2);

    assert!(parse_with("SELECT (a).b", TestDialect).is_err());
}

#[test]
fn field_wildcard_star_selection_in_value_positions() {
    // `(expr).*` off a non-name primary folds into a value composite-star expression.
    let parsed =
        parse_with("SELECT (f(x)).*", PG_EXPR_DIALECT).expect("composite star target parses");
    let Expr::FieldSelection {
        field_selection, ..
    } = project_expr(&parsed)
    else {
        panic!("expected a field selection");
    };
    assert!(matches!(
        field_selection.selector,
        FieldSelector::Star { .. }
    ));
    assert!(matches!(field_selection.base, Expr::Function { .. }));

    // A whole-row `t.*` written inside a value (a `ROW(...)` field) folds too, with a
    // bare column as the base.
    let parsed = parse_with("SELECT ROW(t.*)", PG_EXPR_DIALECT).expect("whole-row in ROW parses");
    let Expr::Row { row, .. } = project_expr(&parsed) else {
        panic!("expected a row constructor");
    };
    let Expr::FieldSelection {
        field_selection, ..
    } = &row.fields[0]
    else {
        panic!("expected a field-wildcard row field");
    };
    assert!(matches!(
        field_selection.selector,
        FieldSelector::Star { .. }
    ));
    assert!(matches!(field_selection.base, Expr::Column { .. }));

    // A bare select-list `t.*` stays a qualified wildcard, not a value star — the star
    // selector is suppressed at the projection-target top level.
    let parsed = parse_with("SELECT t.*", PG_EXPR_DIALECT).expect("qualified wildcard parses");
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    assert!(matches!(
        select.projection[0],
        SelectItem::QualifiedWildcard { .. }
    ));

    // The value-position `.*` round-trips through render (whole-row keeps the parens).
    let parsed = parse_with("SELECT f((t).*)", PG_EXPR_DIALECT).expect("whole-row arg parses");
    let rendered = Renderer::new(PG_EXPR_DIALECT)
        .render_parsed(&parsed)
        .expect("render");
    assert_eq!(rendered, "SELECT f((t).*)");

    // With `field_wildcard` off, the value-position `.*` is left unconsumed and rejects.
    assert!(parse_with("SELECT (f(x)).*", TestDialect).is_err());
    assert!(parse_with("SELECT ROW(t.*)", TestDialect).is_err());
}

// --- typed temporal literals ----------------------------------------------

#[test]
fn typed_temporal_literals_parse_with_kind_and_value_text() {
    let date = parse_with("SELECT DATE '1998-12-01'", PG_EXPR_DIALECT).expect("date literal");
    let Expr::Literal { literal, .. } = project_expr(&date) else {
        panic!("expected a literal");
    };
    assert_eq!(literal.kind, LiteralKind::Date);
    assert_eq!(
        literal.as_temporal_text(date.source()).expect("value text"),
        "1998-12-01",
    );

    let ts = parse_with(
        "SELECT TIMESTAMP WITH TIME ZONE '2020-01-01 00:00:00+00'",
        PG_EXPR_DIALECT,
    )
    .expect("timestamp-with-time-zone literal");
    let Expr::Literal { literal, .. } = project_expr(&ts) else {
        panic!("expected a literal");
    };
    assert_eq!(
        literal.kind,
        LiteralKind::Timestamp {
            time_zone: TimeZone::WithTimeZone,
        },
    );
    assert_eq!(
        literal.as_temporal_text(ts.source()).expect("value text"),
        "2020-01-01 00:00:00+00",
    );

    let time = parse_with("SELECT TIME '12:00:00'", PG_EXPR_DIALECT).expect("time literal");
    let Expr::Literal { literal, .. } = project_expr(&time) else {
        panic!("expected a literal");
    };
    assert_eq!(
        literal.kind,
        LiteralKind::Time {
            time_zone: TimeZone::Unspecified,
        },
    );
}

#[test]
fn interval_literal_captures_qualifier_and_precision() {
    // The leading-precision and trailing-field spellings both land on the kind tag.
    let cases: [(&str, Option<IntervalFields>, Option<u32>); 6] = [
        ("SELECT INTERVAL '90' DAY", Some(IntervalFields::Day), None),
        (
            "SELECT INTERVAL '1-2' YEAR TO MONTH",
            Some(IntervalFields::YearToMonth),
            None,
        ),
        (
            "SELECT INTERVAL '1' DAY TO SECOND",
            Some(IntervalFields::DayToSecond),
            None,
        ),
        (
            "SELECT INTERVAL '1' SECOND(3)",
            Some(IntervalFields::Second),
            Some(3),
        ),
        ("SELECT INTERVAL(6) '1'", None, Some(6)),
        ("SELECT INTERVAL '1 2:03'", None, None),
    ];
    for (sql, fields, precision) in cases {
        let parsed =
            parse_with(sql, PG_EXPR_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a literal");
        };
        assert_eq!(
            literal.kind,
            LiteralKind::Interval { fields, precision },
            "{sql}",
        );
    }
}

#[test]
fn temporal_keyword_without_trailing_string_falls_back_to_name() {
    // No string after the keyword: it reads as an ordinary column reference...
    let parsed = parse_with("SELECT date", PG_EXPR_DIALECT).expect("bare date column");
    assert_eq!(column_name(&parsed, project_expr(&parsed)), "date");

    // ...or a function call when a parenthesis follows (`date` is callable).
    let parsed = parse_with("SELECT date(a)", PG_EXPR_DIALECT).expect("date(...) call");
    assert!(matches!(project_expr(&parsed), Expr::Function { .. }));
}

#[test]
fn temporal_literals_round_trip_exact_source_spelling() {
    // The literal renders byte-for-byte from source — keyword casing and the value
    // string survive — while the surrounding SQL is canonicalized. The first case
    // is the TPC-H Q1 date arithmetic `date '1998-12-01' - interval '90' day`.
    let cases = [
        "SELECT date '1998-12-01' - interval '90' day",
        "SELECT TIMESTAMP WITH TIME ZONE '2020-01-01 00:00:00+00'",
        "SELECT timestamp without time zone '2020-01-01 00:00:00'",
        "SELECT interval '1-2' year to month",
        "SELECT interval '1' second(3)",
    ];
    for sql in cases {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn duckdb_relaxed_interval_amount_forms_parse_with_unit_qualifier() {
    // The unquoted-integer and parenthesized-expression amounts, and plural units, all
    // land on the one `Interval` kind, folding the plural onto the singular qualifier.
    let cases: [(&str, IntervalFields); 5] = [
        ("SELECT INTERVAL 1000 DAY", IntervalFields::Day),
        ("SELECT INTERVAL 3 DAYS", IntervalFields::Day),
        ("SELECT INTERVAL '1' hours", IntervalFields::Hour),
        ("SELECT INTERVAL (days) DAY", IntervalFields::Day),
        ("SELECT INTERVAL (a + 1) MINUTES", IntervalFields::Minute),
    ];
    for (sql, fields) in cases {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected an interval literal");
        };
        assert_eq!(
            literal.kind,
            LiteralKind::Interval {
                fields: Some(fields),
                precision: None,
            },
            "{sql}",
        );
    }
}

#[test]
fn duckdb_relaxed_interval_spellings_round_trip_exact_source() {
    // The whole literal — amount form, plural `s`, and casing — round-trips from the span,
    // even though the tag folds the plural onto the singular qualifier.
    let cases = [
        "SELECT interval 1000 day",
        "SELECT interval 3 days",
        "SELECT interval '1' hours",
        "SELECT interval (days) day",
    ];
    for sql in cases {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn duckdb_relaxed_interval_amount_requires_a_unit() {
    // A bare `INTERVAL <amount>` with no unit parses in DuckDB but is a *binding* error;
    // we reject it at parse time (there is no other reading of a number after `INTERVAL`).
    for sql in ["SELECT INTERVAL 3", "SELECT INTERVAL (days)"] {
        assert!(
            parse_with(sql, DUCKDB_TYPE_DIALECT).is_err(),
            "{sql} must be rejected without a unit",
        );
    }
}

#[test]
fn relaxed_interval_spellings_are_gated_off_by_default() {
    // Off the gate (PostgreSQL) the relaxed spellings are not a valid interval literal:
    // `INTERVAL` falls back to an ordinary name and the trailing tokens do not parse.
    for sql in ["SELECT INTERVAL 3 DAYS", "SELECT INTERVAL (days) DAY"] {
        assert!(
            parse_with(sql, PG_EXPR_DIALECT).is_err(),
            "{sql} must be rejected without the relaxed-interval gate",
        );
    }
    // The standard quoted form still parses under both.
    assert!(parse_with("SELECT INTERVAL '3' DAY", PG_EXPR_DIALECT).is_ok());
}

#[test]
fn duckdb_extended_interval_units_parse_singular_and_plural() {
    // DuckDB's extended units beyond the ANSI qualifiers (engine-verified on 1.5.4), each
    // as an `INTERVAL <amount> <unit>` multiplier: both the singular and plural spelling
    // fold onto one variant, carry no precision, and are admitted only under the relaxed gate.
    let cases: [(&str, IntervalFields); 14] = [
        ("SELECT INTERVAL 5 WEEK", IntervalFields::Week),
        ("SELECT INTERVAL 5 WEEKS", IntervalFields::Week),
        ("SELECT INTERVAL 5 QUARTER", IntervalFields::Quarter),
        ("SELECT INTERVAL 5 QUARTERS", IntervalFields::Quarter),
        ("SELECT INTERVAL 5 DECADE", IntervalFields::Decade),
        ("SELECT INTERVAL 5 DECADES", IntervalFields::Decade),
        ("SELECT INTERVAL 5 CENTURY", IntervalFields::Century),
        ("SELECT INTERVAL 5 CENTURIES", IntervalFields::Century),
        ("SELECT INTERVAL 5 MILLENNIUM", IntervalFields::Millennium),
        ("SELECT INTERVAL 5 MILLENNIA", IntervalFields::Millennium),
        ("SELECT INTERVAL 5 MILLISECOND", IntervalFields::Millisecond),
        (
            "SELECT INTERVAL 5 MILLISECONDS",
            IntervalFields::Millisecond,
        ),
        ("SELECT INTERVAL 5 MICROSECOND", IntervalFields::Microsecond),
        (
            "SELECT INTERVAL 5 MICROSECONDS",
            IntervalFields::Microsecond,
        ),
    ];
    for (sql, fields) in cases {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected an interval literal");
        };
        assert_eq!(
            literal.kind,
            LiteralKind::Interval {
                fields: Some(fields),
                precision: None,
            },
            "{sql}",
        );
    }
}

#[test]
fn duckdb_extended_interval_units_render_canonical_singular_in_type_position() {
    // In `CAST(... AS INTERVAL <unit>)` the type is reconstructed (not sliced from source),
    // so a plural or lowercase unit normalizes to the canonical singular the render arm emits.
    let cases: [(&str, IntervalFields, &str); 3] = [
        (
            "SELECT CAST(a AS INTERVAL weeks)",
            IntervalFields::Week,
            "WEEK",
        ),
        (
            "SELECT CAST(a AS INTERVAL centuries)",
            IntervalFields::Century,
            "CENTURY",
        ),
        (
            "SELECT CAST(a AS INTERVAL microsecond)",
            IntervalFields::Microsecond,
            "MICROSECOND",
        ),
    ];
    for (sql, fields, canonical) in cases {
        let parsed =
            parse_with(sql, DUCKDB_TYPE_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        assert!(
            matches!(
                cast_type(&parsed),
                DataType::Interval { fields: Some(f), precision: None, .. } if *f == fields
            ),
            "{sql}: unexpected cast type",
        );
        let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(
            rendered,
            format!("SELECT CAST(a AS INTERVAL {canonical})"),
            "{sql}",
        );
    }
}

#[test]
fn duckdb_extended_interval_units_gated_off_by_default() {
    // Off the relaxed gate (PostgreSQL) the extended units are not interval qualifiers.
    // In the unquoted amount form there is no other reading of a number after `INTERVAL`,
    // so the whole statement is rejected. (The quoted form `INTERVAL '5' CENTURY` is not a
    // gate signal: PostgreSQL reads it as a bare `INTERVAL '5'` aliased `CENTURY`, exactly
    // as it did before this change — the extended units add no new PostgreSQL behaviour.)
    for sql in [
        "SELECT INTERVAL 5 WEEK",
        "SELECT INTERVAL 5 WEEKS",
        "SELECT INTERVAL 5 CENTURY",
        "SELECT INTERVAL 5 MICROSECONDS",
    ] {
        assert!(
            parse_with(sql, PG_EXPR_DIALECT).is_err(),
            "{sql} must be rejected without the relaxed-interval gate",
        );
    }
    // Under the gate the quoted amount folds the extended unit onto its variant, whereas
    // off the gate that trailing word would be a column alias — this is the behavioural pivot.
    let parsed = parse_with("SELECT INTERVAL '5' CENTURY", DUCKDB_TYPE_DIALECT)
        .expect("quoted extended-unit interval parses under the relaxed gate");
    let Expr::Literal { literal, .. } = project_expr(&parsed) else {
        panic!("expected an interval literal");
    };
    assert_eq!(
        literal.kind,
        LiteralKind::Interval {
            fields: Some(IntervalFields::Century),
            precision: None,
        },
    );
}

#[test]
fn tpch_q1_date_arithmetic_keeps_literal_structure() {
    let parsed = parse_with("SELECT date '1998-12-01' - interval '90' day", Postgres)
        .expect("date arithmetic parses");
    let Expr::BinaryOp {
        left, op, right, ..
    } = project_expr(&parsed)
    else {
        panic!("expected subtraction at the projection root");
    };
    assert_eq!(*op, BinaryOperator::Minus);
    assert!(
        matches!(
            **left,
            Expr::Literal {
                literal: Literal {
                    kind: LiteralKind::Date,
                    ..
                },
                ..
            },
        ),
        "left operand is the date literal",
    );
    assert!(
        matches!(
            **right,
            Expr::Literal {
                literal: Literal {
                    kind: LiteralKind::Interval {
                        fields: Some(IntervalFields::Day),
                        precision: None,
                    },
                    ..
                },
                ..
            },
        ),
        "right operand is the `interval '90' day` literal",
    );
}

// --- PostgreSQL special literal forms -------------------------------------

#[test]
fn bit_string_literals_parse_with_radix_and_value_text() {
    let cases = [
        ("SELECT B'1010'", BitStringRadix::Binary, "1010"),
        ("SELECT b'1010'", BitStringRadix::Binary, "1010"),
        ("SELECT X'1FF'", BitStringRadix::Hex, "1FF"),
        ("SELECT x'1ff'", BitStringRadix::Hex, "1ff"),
    ];
    for (sql, radix, digits) in cases {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a literal");
        };
        assert_eq!(
            literal.kind,
            LiteralKind::BitString { radix },
            "kind for {sql}"
        );
        assert_eq!(
            literal.as_bit_text(parsed.source()).expect("digit body"),
            digits,
            "digits for {sql}",
        );
    }
}

#[test]
fn sqlite_hex_blob_literals_accept_even_hex_and_reject_malformed() {
    // SQLite's `x'…'`/`X'…'` BLOB literal reuses the canonical hex `BitString` shape, but
    // is validated *eagerly* at lex time: an even count of valid hex digits (each pair a
    // byte), the empty `x''` being a valid zero-byte blob.
    for (sql, digits) in [
        ("SELECT x'53514C'", "53514C"),
        ("SELECT X'53514c'", "53514c"),
        ("SELECT x'1A'", "1A"),
        ("SELECT x''", ""),
    ] {
        let parsed = parse_with(sql, Sqlite).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a literal");
        };
        assert_eq!(
            literal.kind,
            LiteralKind::BitString {
                radix: BitStringRadix::Hex,
            },
            "kind for {sql}",
        );
        assert_eq!(
            literal.as_bit_text(parsed.source()).expect("digit body"),
            digits,
            "digits for {sql}",
        );
    }
    // Zero over-acceptance: an odd digit count or a non-hex body is a tokenize-time
    // reject in SQLite ("unrecognized token", probed on the bundled 3.53.2 oracle),
    // unlike a PostgreSQL deferred bit-string that tolerates odd-length hex.
    for sql in [
        "SELECT x'ABC'",
        "SELECT x'0'",
        "SELECT x'XY'",
        "SELECT X'1FF'",
    ] {
        assert!(
            parse_with(sql, Sqlite).is_err(),
            "{sql} must reject as a malformed blob",
        );
    }
    // The eager blob gate claims only the `x`/`X` hex marker: PostgreSQL's `X'1FF'` stays
    // the deferred (odd-length-tolerant) bit-string, so enabling SQLite blobs does not
    // tighten the PostgreSQL bit-string surface.
    assert!(
        parse_with("SELECT X'1FF'", Postgres).is_ok(),
        "PostgreSQL X'1FF' stays a deferred bit-string",
    );
}

#[test]
fn number_literal_kind_classifies_radix_integers_apart_from_decimal_floats() {
    // Regression: a 0x/0o/0b radix integer is an Integer even when its digits include
    // E/e — a hex digit, not an exponent marker — so 0xBEEF/0x1E/0xE must not fall to
    // the decimal float scan (which tagged them Float, so as_i64 then reported
    // WrongKind). The octal form is exercised only here: no dialect lexes 0o.., so the
    // classifier is the sole reachable surface for it.
    for radix in ["0xBEEF", "0x1E", "0xE", "0XbeeF", "0o17", "0b1010"] {
        assert_eq!(
            super::number_literal_kind(radix, false),
            LiteralKind::Integer,
            "radix literal {radix} classifies as Integer",
        );
    }
    // A decimal fractional/scientific form stays a Float; a leading 0 that is not a
    // radix marker (0e5) keeps the decimal exponent reading rather than matching 0x/0o/0b.
    for float in ["1e5", "1.5", "3.14", ".5", "0e5"] {
        assert_eq!(
            super::number_literal_kind(float, false),
            LiteralKind::Float,
            "decimal float {float} classifies as Float",
        );
    }
    assert_eq!(
        super::number_literal_kind("42", false),
        LiteralKind::Integer
    );
}

#[test]
fn parse_float_as_decimal_flag_reclassifies_only_floats() {
    // With the parse-float-as-decimal request set, a fractional/scientific literal is
    // tagged Decimal; integers, radix integers, and money are untouched — the flag's
    // sole effect.
    for float in ["1e5", "1.5", "3.14", ".5", "0e5"] {
        assert_eq!(
            super::number_literal_kind(float, true),
            LiteralKind::Decimal,
            "decimal float {float} classifies as Decimal when the flag is set",
        );
    }
    for integer in ["42", "0xBEEF", "0o17", "0b1010"] {
        assert_eq!(
            super::number_literal_kind(integer, true),
            LiteralKind::Integer,
            "integer {integer} is unaffected by the flag",
        );
    }
    assert_eq!(
        super::number_literal_kind("$1234.56", true),
        LiteralKind::Money,
        "money is unaffected by the flag",
    );
}

#[test]
fn parse_options_parse_float_as_decimal_reaches_the_ast_without_touching_render() {
    // Default options: a fractional literal is a Float, and its exact spelling
    // round-trips — the historical behaviour the option must not disturb.
    let default_parsed = parse_with("SELECT 3.14", Ansi).expect("default parse");
    let Expr::Literal { literal, .. } = project_expr(&default_parsed) else {
        panic!("expected a literal");
    };
    assert_eq!(literal.kind, LiteralKind::Float);
    assert_eq!(
        Renderer::new(Ansi)
            .render_parsed(&default_parsed)
            .expect("render default"),
        "SELECT 3.14",
    );

    // Flag on: the same literal is now a Decimal, but the spelling is byte-identical
    // (classification is metadata, not spelling) and the value still materialises.
    let options = ParseOptions::default().with_parse_float_as_decimal(true);
    let decimal_parsed =
        parse_with_options("SELECT 3.14", Ansi, options).expect("float-as-decimal parse");
    let Expr::Literal { literal, .. } = project_expr(&decimal_parsed) else {
        panic!("expected a literal");
    };
    assert_eq!(literal.kind, LiteralKind::Decimal);
    assert_eq!(
        literal
            .as_decimal_text(decimal_parsed.source())
            .expect("decimal literal materialises")
            .as_ref(),
        "3.14",
    );
    assert_eq!(
        Renderer::new(Ansi)
            .render_parsed(&decimal_parsed)
            .expect("render decimal"),
        "SELECT 3.14",
        "render is unaffected by the classification",
    );

    // An integer literal is untouched even with the flag on.
    let int_parsed =
        parse_with_options("SELECT 42", Ansi, options).expect("integer parse with flag");
    let Expr::Literal { literal, .. } = project_expr(&int_parsed) else {
        panic!("expected a literal");
    };
    assert_eq!(literal.kind, LiteralKind::Integer);
}

#[test]
fn radix_integer_literals_classify_and_decode_through_as_i64() {
    // End-to-end: a hex/binary integer reaches the AST as LiteralKind::Integer and
    // round-trips through as_i64. 0xBEEF/0x1E/0xE carry E/e digits the pre-fix scan
    // misread as a float exponent, leaving as_i64 returning WrongKind (ADR-0006).
    // MySQL is the lexing dialect: it enables 0x hex and 0b binary (it has no 0o octal).
    let cases = [
        ("SELECT 0xBEEF", 48879_i64),
        ("SELECT 0x1E", 30),
        ("SELECT 0xE", 14),
        ("SELECT 0b1010", 10),
    ];
    for (sql, value) in cases {
        let parsed = parse_with(sql, MySql).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a literal");
        };
        assert_eq!(literal.kind, LiteralKind::Integer, "kind for {sql}");
        assert_eq!(
            literal.as_i64(parsed.source()).expect("integer value"),
            value,
            "value for {sql}",
        );
    }
}

#[test]
fn bit_string_marker_without_abutting_quote_is_an_identifier() {
    // `B`/`X` only opens a bit string when the quote abuts it. With a space `b` is an
    // ordinary identifier, so `b 'x'` is the generalized typed string constant `type
    // 'string'` — a cast of the plain string `'x'` to type `b`, matching PostgreSQL
    // (`func_name Sconst`) — never a bit string.
    let parsed = parse_with("SELECT b 'x'", Postgres).expect("`b 'x'` is a typed string constant");
    let Expr::Cast { expr, syntax, .. } = project_expr(&parsed) else {
        panic!("`b 'x'` is a typed-literal cast, not a bit string");
    };
    assert_eq!(*syntax, CastSyntax::PrefixTyped);
    let Expr::Literal { literal, .. } = &**expr else {
        panic!("the operand is a plain string constant");
    };
    assert!(
        matches!(literal.kind, LiteralKind::String),
        "the spaced marker leaves a plain string, not a bit string",
    );
    let parsed = parse_with("SELECT x", Postgres).expect("`x` is a column reference");
    assert!(matches!(project_expr(&parsed), Expr::Column { .. }));
}

#[test]
fn national_and_unicode_strings_parse_as_string_literals() {
    // National and Unicode-escape constants are character strings; like `E'...'`
    // and `$$…$$` they share `LiteralKind::String` and recover their value lazily.
    // The national case runs under MySQL — the PostgreSQL preset does not arm
    // `national_strings` (PG has no `N'…'` constant; `N'naive'` reads as the typed
    // literal `N '…'` there, pinned in `dialect::postgres`'s
    // `postgres_reads_a_national_string_spelling_as_a_typed_literal`) — and its value
    // materialization strips the one-byte `N` prefix like `E'…'`.
    let parsed = parse_with("SELECT N'naive'", MySql).expect("MySQL lexes the national string");
    let Expr::Literal { literal, .. } = project_expr(&parsed) else {
        panic!("SELECT N'naive': expected a literal under MySQL");
    };
    assert_eq!(literal.kind, LiteralKind::String);
    assert_eq!(
        literal.as_str(parsed.source()).expect("string value"),
        "naive",
    );

    let cases = [
        (r"SELECT U&'d\0061ta'", "data"),
        ("SELECT U&'d!0061ta' UESCAPE '!'", "data"),
        // A doubled escape (`\\`) and a doubled quote (`''`) still parse and
        // materialize unchanged by the eager escape-body check below: neither is
        // malformed, so `unicode_escape_string_is_valid` accepts both.
        (r"SELECT U&'a\\b''c'", "a\\b'c"),
    ];
    for (sql, value) in cases {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a literal");
        };
        assert_eq!(literal.kind, LiteralKind::String, "kind for {sql}");
        assert_eq!(
            literal.as_str(parsed.source()).expect("string value"),
            value,
            "value for {sql}",
        );
    }
}

#[test]
fn unicode_escaped_identifier_decodes_to_its_resolved_name() {
    // `U&"..."` is the delimited-identifier surface of the same `U&` escape facility as
    // `U&'...'`: the interned symbol is the *decoded* name, so the identifier compares equal
    // to its plain `"..."` equivalent — `U&"d0061ta"` is `data`, exactly as PostgreSQL
    // resolves it. Value probed against pg_query. The quote style is `UnicodeDouble`, which
    // carries the source spelling for a fidelity render (see the round-trip test below).
    let cases = [
        (r#"SELECT U&"d\0061ta""#, "data"),
        (r#"SELECT U&"d0061ta""#, "d0061ta"), // no `\`, so no escape decoding
        (r#"SELECT U&"real\00A7_name""#, "real\u{00A7}_name"),
        (r#"SELECT u&"x""#, "x"),     // case-insensitive prefix
        (r#"SELECT U&"a'b""#, "a'b"), // a single quote is an ordinary body byte
        (r#"SELECT U&"""""#, "\""),   // doubled `""` collapses to one `"`
        // A trailing `UESCAPE 'c'` overrides the escape character, applied to the whole
        // identifier: `*0061` decodes with `*`, and the earlier `\` is then inert.
        (r#"SELECT U&"d*0061t\+000061" UESCAPE '*'"#, "dat\\+000061"),
        (r#"SELECT U&"\ZZZZ" UESCAPE '!'"#, "\\ZZZZ"), // `!` makes `\` inert
    ];
    for (sql, name) in cases {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let expr = project_expr(&parsed);
        assert_eq!(column_name(&parsed, expr), name, "decoded name for {sql}");
        let Expr::Column { name: object, .. } = expr else {
            panic!("{sql}: expected a column reference");
        };
        assert_eq!(
            object.0[0].quote,
            crate::ast::QuoteStyle::UnicodeDouble,
            "a U&\"...\" identifier carries the UnicodeDouble spelling for {sql}",
        );
    }
}

#[test]
fn unicode_escaped_identifier_preserves_source_spelling_through_canonical_render() {
    // The `UnicodeDouble` spelling round-trips byte-for-byte through the canonical
    // (source-fidelity `PreserveSource`) render — `Parsed::to_sql` — with the `U&` prefix
    // and the whole `UESCAPE 'c'` clause intact, rather than collapsing to the decoded
    // `"..."` form. A `Renderer` (Tier-1 `TargetDialect`) re-spell still normalizes to the
    // plain `"..."` equivalent; that split is the source-spelling doctrine, and the
    // `unicode_escaped_identifier_folds_uescape_in_every_identifier_position` re-parse test
    // covers the re-spell path.
    for sql in [
        r#"SELECT U&"d\0061t\+000061""#,
        r#"SELECT U&"d*0061t\+000061" UESCAPE '*'"#,
        r#"SELECT U&"real\00A7_name""#,
        r#"SELECT * FROM U&"my\0074able""#,
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        assert_eq!(
            parsed.to_sql(),
            sql,
            "the U&\"...\" spelling must round-trip verbatim through canonical render",
        );
    }
}

#[test]
fn unicode_escaped_identifier_folds_uescape_in_every_identifier_position() {
    // The trailing `UESCAPE` fold is a lexer-layer concern (PostgreSQL's base_yylex
    // wrapper), so it works wherever an identifier does — column ref, `AS` alias, and a
    // relation name — not only in expression position. These are the exact PG regress
    // corpus lines the ticket closes; each parses and structurally round-trips.
    for sql in [
        r#"SELECT U&"real\00A7_name" FROM (select 1) AS x(real_name)"#,
        r#"SELECT U&'d\0061t\+000061' AS U&"d\0061t\+000061""#,
        r#"SELECT U&'d!0061t\+000061' UESCAPE '!' AS U&"d*0061t\+000061" UESCAPE '*'"#,
        r#"SELECT 'tricky' AS U&"\" UESCAPE '!'"#,
        r#"SELECT * FROM U&"my\0074able""#,
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: render failed: {err:?}"));
        parse_with(&rendered, Postgres)
            .unwrap_or_else(|err| panic!("{sql}: render {rendered:?} does not re-parse: {err:?}"));
    }
}

#[test]
fn unicode_escaped_identifier_rejects_like_postgres() {
    // Eager parse-time rejects that keep over-acceptance at zero (all probed against
    // pg_query): a zero-length body, an illegal `UESCAPE` delimiter, a malformed escape
    // body against the resolved escape character, and a greedily-consumed `UESCAPE`
    // keyword with no following string (PG's wrapper always reads it as the escape clause).
    for sql in [
        r#"SELECT U&"" FROM t"#,            // zero-length delimited identifier
        r#"SELECT U&"d0061" UESCAPE '+'"#,  // `+` is not a legal escape delimiter
        r#"SELECT U&"d0061" UESCAPE '5'"#,  // a hex digit is not legal
        r#"SELECT U&"d0061" UESCAPE '!!'"#, // a multi-character delimiter is not legal
        r#"SELECT U&"\ZZZZ""#,              // malformed `\`-escape (non-hex)
        r#"SELECT U&"\d800""#,              // a lone surrogate
        r#"SELECT U&"\0000""#,              // an escape decoding to NUL
        r#"SELECT U&"x" uescape FROM t"#,   // greedy UESCAPE, no following string
    ] {
        parse_with(sql, Postgres).expect_err(&format!("{sql:?} must be rejected"));
    }

    // A plain double-quoted identifier never folds a following `UESCAPE` — only the `U&`
    // form does — so `"x" UESCAPE '!'` is a syntax error, as in PostgreSQL.
    parse_with(r#"SELECT "x" UESCAPE '!'"#, Postgres)
        .expect_err("a non-U& identifier does not take a UESCAPE clause");
}

#[test]
fn unicode_string_with_invalid_escape_is_rejected_at_parse_time() {
    // eager-validate-unicode-escape-strings-for-oracle-parity: a malformed
    // `U&'...'` escape body is now rejected at parse time, mirroring `E'...'`'s
    // existing eager check, rather than only failing later at `Literal::as_str`.
    for sql in [
        r"SELECT U&'\0000'",    // an escape decoding to NUL
        r"SELECT U&'\D800'",    // a lone (unpaired) high surrogate
        r"SELECT U&'\+110000'", // a code point above U+10FFFF
        r"SELECT U&'\XYZW'",    // non-hex escape digits
        r"SELECT U&'\'",        // a dangling trailing escape
    ] {
        let err = parse_with(sql, Postgres).expect_err(&format!("{sql:?} must be rejected"));
        assert_eq!(
            err.found.to_string(),
            "invalid escape sequence in string literal",
            "for {sql:?}",
        );
    }
}

#[test]
fn unicode_string_invalid_escape_error_spans_the_whole_literal() {
    let sql = r"SELECT U&'\0000'";
    let err = parse_with(sql, Postgres).expect_err("a NUL-decoding escape is rejected");
    let literal_start = sql.find("U&").expect("fixture contains U&") as u32;
    assert_eq!(
        err.span,
        Span::new(literal_start, sql.len() as u32),
        "the error spans the whole U&'...' literal, not just the offending escape",
    );
}

#[test]
fn unicode_string_validates_against_the_resolved_uescape_character_not_default_backslash() {
    // The eager check must resolve the escape character exactly the way
    // `Literal::as_str` does — from any trailing `UESCAPE` clause — rather than
    // assume `\` before that clause is seen; see `unicode_escape_string_is_valid`'s
    // doc for why the naive "validate at scan time with `\`" shape is unsound.
    //
    // `\ZZZZ` is a malformed `\`-escape (non-hex digits), but `UESCAPE '!'` makes
    // `!` the active escape character, so `\` is just two ordinary characters here
    // and the literal parses and materializes unchanged.
    let sql = r"SELECT U&'\ZZZZ' UESCAPE '!'";
    let parsed = parse_with(sql, Postgres)
        .unwrap_or_else(|err| panic!("{sql:?} is legal under UESCAPE '!': {err:?}"));
    let Expr::Literal { literal, .. } = project_expr(&parsed) else {
        panic!("expected a literal");
    };
    assert_eq!(
        literal.as_str(parsed.source()).expect("string value"),
        r"\ZZZZ",
    );

    // Conversely, `!d800` is an ordinary run of characters under the *default* `\`
    // escape, but decodes to a lone surrogate once `UESCAPE '!'` makes `!` the
    // active escape character, and must be rejected — the same verdict as the
    // unpaired `U&'\D800'` case above, just reached through the custom escape.
    let sql = "SELECT U&'!d800' UESCAPE '!'";
    let err = parse_with(sql, Postgres)
        .expect_err("a lone surrogate under the custom escape character is rejected");
    assert_eq!(
        err.found.to_string(),
        "invalid escape sequence in string literal"
    );
}

#[test]
fn parsed_as_str_decodes_mysql_backslash_escapes_without_naming_the_dialect() {
    // A MySQL `Parsed` carries its own `StringLiteralSyntax`, so `Parsed::as_str`
    // decodes the dialect's C-style backslash escapes — `\n` -> a newline — with the
    // caller never hand-passing `StringLiteralSyntax::MYSQL`. Both the double-quoted
    // (`"a\nb"`) and single-quoted (`'a\nb'`) forms honour the escape, while the
    // ANSI-default `Literal::as_str` still reads the same source literally — proving
    // it is the parse's dialect context, not a changed default, that unescapes.
    for sql in [r#"SELECT "a\nb""#, r"SELECT 'a\nb'"] {
        let parsed = parse_with(sql, MySql).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a string literal");
        };
        assert!(
            parsed.string_literal_syntax().backslash_escapes,
            "MySQL retains backslash-escape syntax on the root",
        );
        // The Parsed path threads MySQL syntax: `\n` decodes to a newline.
        assert_eq!(
            parsed.literal_str(literal).expect("MySQL string value"),
            "a\nb",
            "Parsed::literal_str value for {sql}",
        );
        // `Literal::as_str`'s ANSI default is unchanged: the backslash stays literal.
        assert_eq!(
            literal.as_str(parsed.source()).expect("ANSI string value"),
            r"a\nb",
            "ANSI as_str value for {sql}",
        );
    }
}

#[test]
fn parsed_literal_str_keeps_backslashes_literal_under_postgres() {
    // The same `'a\nb'` source under a PostgreSQL `Parsed` materialises literally —
    // `\` and `n` stay two characters — because the root carries ANSI string syntax
    // (backslash escapes off). The dialect on the root, not the caller, decides,
    // through the identical `Parsed::literal_str` path.
    let sql = r"SELECT 'a\nb'";
    let parsed = parse_with(sql, Postgres).expect(r"`'a\nb'` parses under PostgreSQL");
    let Expr::Literal { literal, .. } = project_expr(&parsed) else {
        panic!("expected a string literal");
    };
    assert!(
        !parsed.string_literal_syntax().backslash_escapes,
        "PostgreSQL leaves backslash escapes off on the root",
    );
    assert_eq!(
        parsed.literal_str(literal).expect("PG string value"),
        r"a\nb"
    );
}

#[test]
fn parsed_literal_str_agrees_with_ansi_as_str_and_preserves_doubled_quotes() {
    // Regression: `Literal::as_str(source)`'s ANSI default and the doubled-quote `''`
    // collapse are byte-for-byte unchanged, and `Parsed::literal_str` over an
    // ANSI-family parse threads that same `StringLiteralSyntax`, so the two agree.
    let sql = "SELECT 'it''s'";
    let parsed = parse_with(sql, Postgres).expect("doubled-quote string parses");
    let Expr::Literal { literal, .. } = project_expr(&parsed) else {
        panic!("expected a string literal");
    };
    assert_eq!(literal.as_str(parsed.source()).expect("ANSI value"), "it's");
    assert_eq!(parsed.literal_str(literal).expect("Parsed value"), "it's");
}

#[test]
fn charset_introduced_strings_parse_as_string_literals_under_mysql() {
    // A MySQL `_charset'…'` introducer is a character string: like `N'…'` it shares
    // the canonical `LiteralKind::String` shape (ADR-0011), recovers its value with
    // the introducer stripped (ADR-0006), and exposes the charset name as a surface
    // tag. The exact source — introducer included — round-trips on render.
    let cases = [
        ("SELECT _utf8mb4'cafe'", "utf8mb4", "cafe"),
        ("SELECT _latin1'x'", "latin1", "x"),
        ("SELECT _utf8'it''s'", "utf8", "it's"), // doubled-quote body still unescapes
    ];
    for (sql, charset, value) in cases {
        let parsed = parse_with(sql, MySql).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a literal");
        };
        assert_eq!(literal.kind, LiteralKind::String, "kind for {sql}");
        assert_eq!(
            literal.as_str(parsed.source()).expect("string value"),
            value,
            "value for {sql}",
        );
        assert_eq!(
            literal
                .charset_introducer(parsed.source())
                .expect("string literal"),
            Some(charset),
            "introducer for {sql}",
        );
        // Exact round-trip: the `_charset` introducer renders verbatim from the span.
        // A literal renders its source slice dialect-independently, so any
        // `RenderDialect` (here `Postgres`, since the parser-only `MySql` is not one)
        // preserves the introducer surface.
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn charset_introducer_is_inert_without_the_dialect_knob() {
    // With charset introducers off (PostgreSQL), `_utf8'x'` is the word `_utf8`
    // followed by `'x'`, which re-reads as the generalized typed literal `_utf8 'x'`
    // — an `Expr::Cast`, not a string `Expr::Literal` — so the surface is unchanged
    // from today's behaviour (prod-literal-generic-typed). A plain string also has
    // no introducer.
    let parsed = parse_with("SELECT _utf8'x'", Postgres).expect("`_utf8 'x'` is a typed literal");
    assert!(
        matches!(project_expr(&parsed), Expr::Cast { .. }),
        "charset introducers off reads `_utf8'x'` as a typed-literal cast",
    );

    let parsed = parse_with("SELECT 'x'", MySql).expect("plain string parses");
    let Expr::Literal { literal, .. } = project_expr(&parsed) else {
        panic!("expected a literal");
    };
    assert_eq!(
        literal
            .charset_introducer(parsed.source())
            .expect("string literal"),
        None,
        "a plain string carries no charset introducer",
    );
}

#[test]
fn charset_introduced_string_concatenates_across_a_newline() {
    // A `_charset'…'` leading segment continues with plain `'…'` segments across a
    // newline (SQL-standard adjacent-string concatenation): the leading segment
    // carries the introducer, the continuations are plain strings, and the joined
    // value strips the introducer (ADR-0006), like the `B'…'`/`N'…'` precedents.
    let parsed = parse_with("SELECT _utf8'foo'\n'bar'", MySql)
        .expect("adjacent charset strings concatenate");
    let Expr::Literal { literal, .. } = project_expr(&parsed) else {
        panic!("expected a literal");
    };
    assert_eq!(literal.kind, LiteralKind::String);
    assert_eq!(
        literal.as_str(parsed.source()).expect("concatenated value"),
        "foobar",
    );
    assert_eq!(
        literal
            .charset_introducer(parsed.source())
            .expect("string literal"),
        Some("utf8"),
        "the introducer applies to the whole continued constant",
    );
}

#[test]
fn adjacent_string_literals_concatenate_across_a_newline() {
    // SQL-standard adjacent-string concatenation: string constants separated by
    // whitespace containing a newline are one value (`'foo'`⏎`'bar'` ≡ `'foobar'`).
    // The parser keeps one `Literal` whose span covers every segment; the value is
    // recovered, joined, at the accessor (ADR-0006).
    let cases = [
        ("SELECT 'foo'\n'bar'", "foobar"),
        ("SELECT 'a'\n'b'\n'c'", "abc"), // three segments
        ("SELECT E'a'\n'b'", "ab"),      // a leading escape-string segment
        ("SELECT 'a'  \r\n  'b'", "ab"), // CRLF with surrounding spaces
        ("SELECT 'a'\n\n'b'", "ab"),     // a blank line between
    ];
    for (sql, value) in cases {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql:?}: expected a literal");
        };
        assert_eq!(literal.kind, LiteralKind::String, "kind for {sql:?}");
        assert_eq!(
            literal.as_str(parsed.source()).expect("concatenated value"),
            value,
            "value for {sql:?}",
        );
    }
}

#[test]
fn bit_string_literals_concatenate_across_a_newline() {
    // Bit-string constants continue the same way (PostgreSQL continues its bit-string
    // lexer state too); the digit bodies join into one value.
    let parsed =
        parse_with("SELECT B'1010'\n'0101'", Postgres).expect("adjacent bit strings concatenate");
    let Expr::Literal { literal, .. } = project_expr(&parsed) else {
        panic!("expected a literal");
    };
    assert!(matches!(literal.kind, LiteralKind::BitString { .. }));
    assert_eq!(
        literal.as_bit_text(parsed.source()).expect("joined digits"),
        "10100101",
    );
}

#[test]
fn adjacent_string_literals_on_one_line_are_rejected() {
    // Without a newline in the gap the two constants are an error (PostgreSQL rejects
    // them); a comment in the gap is not a continuation either, even one that itself
    // contains a newline.
    for sql in [
        "SELECT 'foo' 'bar'",    // same line, space only
        "SELECT 'foo'\t'bar'",   // tab only, still no newline
        "SELECT 'a'/* c */'b'",  // a comment in the gap
        "SELECT 'a'/* \n */'b'", // a newline *inside* a comment does not count
        "SELECT 'a' -- c\n'b'",  // a line comment before the newline
    ] {
        assert!(parse_with(sql, Postgres).is_err(), "must reject {sql:?}");
    }
}

#[test]
fn adjacent_string_concat_requires_a_plain_continuation_segment() {
    // PostgreSQL continues a constant only with a plain `'...'`; a prefixed second
    // segment (`E'`, `U&'`) or a dollar-quoted first segment is not a continuation,
    // so these stay adjacent constants and are rejected.
    for sql in [
        "SELECT 'a'\nE'b'",  // a non-plain continuation segment
        "SELECT 'a'\nU&'b'", // ditto
        "SELECT $$a$$\n'b'", // a dollar-quoted first segment never continues
    ] {
        assert!(parse_with(sql, Postgres).is_err(), "must reject {sql:?}");
    }
}

#[test]
fn adjacent_string_concatenation_round_trips_exact_source() {
    // The literal renders its span verbatim, so the concatenated source — newline
    // and all — round-trips byte-for-byte (ADR-0006).
    for sql in ["SELECT 'foo'\n'bar'", "SELECT E'a'\n'b'\n'c'"] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql:?}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql:?}");
    }
}

#[test]
fn temporal_literal_value_string_concatenates_across_a_newline() {
    // A temporal literal's value string continues like a bare string primary
    // (PostgreSQL continues the embedded constant too): the span covers every
    // segment and `as_temporal_text` materialises the concatenated value.
    let cases = [(
        "SELECT DATE '1998'\n'-12-01'",
        LiteralKind::Date,
        "1998-12-01",
    )];
    for (sql, kind, value) in cases {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql:?}: expected a literal");
        };
        assert_eq!(literal.kind, kind, "kind for {sql:?}");
        assert_eq!(
            literal
                .as_temporal_text(parsed.source())
                .expect("value text"),
            value,
            "value for {sql:?}",
        );
    }
    // Same-line adjacency in a temporal value string is rejected, like a bare primary.
    assert!(parse_with("SELECT DATE '1998' '-12-01'", Postgres).is_err());
}

#[test]
fn pg_special_literals_round_trip_exact_source_spelling() {
    // Every form renders byte-for-byte from its span, including the `UESCAPE` clause
    // that the parser folds into the literal.
    let cases = [
        "SELECT B'1010'",
        "SELECT X'1FF'",
        r"SELECT U&'d\0061ta'",
        "SELECT U&'d!0061ta' UESCAPE '!'",
    ];
    for sql in cases {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }

    // `N'naive'` is a *literal* — and so span-verbatim — only under a dialect that arms
    // `national_strings` (MySQL/T-SQL; NATIONAL_DIALECT isolates the flag, since the
    // parser-only `MySql` dialect cannot drive the renderer). Under PostgreSQL, which
    // has no national constant (pg-national-strings-lexing-divergence), it is the typed
    // literal `N '…'` and takes the established abutting-prefix canonicalization every
    // prefix-typed literal gets (`float8'x'` → `float8 'x'`): the type name and value
    // separate, and the render re-parses to the same tree (the structural round-trip
    // contract for casts).
    let national = parse_with("SELECT N'naive'", NATIONAL_DIALECT)
        .expect("a national-arming dialect lexes the national string");
    assert_eq!(
        Renderer::new(NATIONAL_DIALECT)
            .render_parsed(&national)
            .expect("renders under the national-arming dialect"),
        "SELECT N'naive'",
        "the national literal renders span-verbatim",
    );
    let pg = parse_with("SELECT N'naive'", Postgres).expect("parses under PostgreSQL");
    assert_eq!(
        Renderer::new(Postgres)
            .render_parsed(&pg)
            .expect("renders under PostgreSQL"),
        "SELECT N 'naive'",
        "the typed-literal reading canonicalizes the abutting prefix with a space",
    );
}

#[test]
fn unicode_string_without_uescape_keyword_does_not_consume_following_tokens() {
    // The `UESCAPE` fold only fires for `U&'...'`; a plain string keeps its span.
    let parsed = parse_with(r"SELECT U&'\0041', 1", Postgres).expect("two projection items parse");
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    assert_eq!(
        select.projection.len(),
        2,
        "comma after U&'...' is the item separator"
    );
}

/// ANSI plus `N'…'` national strings, isolating the national lexer flag from the rest
/// of a dialect preset (the MySQL/T-SQL presets that arm it; PostgreSQL does not —
/// pg-national-strings-lexing-divergence). Implements `RenderDialect` for the
/// round-trip check, which the parser-only `MySql` dialect cannot drive.
const NATIONAL_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax {
            national_strings: true,
            ..StringLiteralSyntax::ANSI
        }));
    FeatureDialect {
        features: &FEATURES,
    }
};

// --- T-SQL money literals -------------------------------------------------

/// ANSI plus T-SQL money literals, isolating the `$`-prefixed money form from the
/// rest of a dialect preset. Implements `RenderDialect` for the round-trip check.
const MONEY_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.numeric_literals(NumericLiteralSyntax {
            money_literals: true,
            ..NumericLiteralSyntax::ANSI
        }));
    FeatureDialect {
        features: &FEATURES,
    }
};

#[test]
fn money_literals_parse_with_money_kind_and_value_text() {
    // The `$` rides the span; the literal is a distinct `Money` kind and the numeric
    // body materialises with the sigil stripped (ADR-0006).
    let cases = [
        ("SELECT $1234.56", "1234.56"),
        ("SELECT $100", "100"),
        ("SELECT $.5", ".5"),
    ];
    for (sql, body) in cases {
        let parsed = parse_with(sql, MONEY_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let Expr::Literal { literal, .. } = project_expr(&parsed) else {
            panic!("{sql}: expected a literal");
        };
        assert_eq!(literal.kind, LiteralKind::Money, "kind for {sql}");
        assert_eq!(
            literal.as_money_text(parsed.source()).expect("money body"),
            body,
            "body for {sql}",
        );
    }
}

#[test]
fn signed_money_is_a_unary_op_over_an_unsigned_money_literal() {
    // Like every signed numeric literal, the sign is an operator node over the
    // unsigned money literal, never folded into it (ADR-0006).
    let parsed = parse_with("SELECT -$1000", MONEY_DIALECT).expect("signed money parses");
    let Expr::UnaryOp {
        op: UnaryOperator::Minus,
        expr,
        ..
    } = project_expr(&parsed)
    else {
        panic!("expected a unary minus over a money literal");
    };
    let Expr::Literal { literal, .. } = expr.as_ref() else {
        panic!("expected a money literal operand");
    };
    assert_eq!(literal.kind, LiteralKind::Money);
    assert_eq!(
        literal.as_money_text(parsed.source()).expect("money body"),
        "1000",
    );
}

#[test]
fn money_literals_are_dialect_gated() {
    // Money is T-SQL-only. Under ANSI the `$` is a stray byte; under PostgreSQL the
    // positional `$1234` parameter leaves a dangling `.56` — both are parse errors.
    assert!(
        parse_with("SELECT $1234.56", TestDialect).is_err(),
        "ANSI rejects money literals",
    );
    assert!(
        parse_with("SELECT $1234.56", Postgres).is_err(),
        "PostgreSQL rejects money literals",
    );
}

#[test]
fn money_literals_round_trip_exact_source_spelling() {
    for sql in [
        "SELECT $1234.56",
        "SELECT $100",
        "SELECT $.5",
        "SELECT -$1000",
    ] {
        let parsed = parse_with(sql, MONEY_DIALECT).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(MONEY_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

// --- PostgreSQL named function arguments ----------------------------------

#[test]
fn named_argument_arrow_parses() {
    let parsed = parse_with("SELECT f(a => 1)", PG_EXPR_DIALECT).expect("named arg parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert_eq!(call.args.len(), 1);
    let arg = &call.args[0];
    assert_eq!(arg.syntax, ArgSyntax::Arrow);
    let name = arg.name.expect("a named argument carries a name");
    assert_eq!(parsed.resolver().resolve(name), "a");
    assert!(matches!(arg.value, Expr::Literal { .. }));
}

#[test]
fn named_argument_colon_equals_parses() {
    let parsed =
        parse_with("SELECT f(a := 1)", PG_EXPR_DIALECT).expect("deprecated named arg parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    let arg = &call.args[0];
    assert_eq!(arg.syntax, ArgSyntax::ColonEquals);
    assert_eq!(parsed.resolver().resolve(arg.name.expect("name")), "a");
}

#[test]
fn mixed_positional_and_named_arguments_parse() {
    let parsed = parse_with("SELECT f(1, b => 2)", PG_EXPR_DIALECT).expect("mixed args parse");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert_eq!(call.args.len(), 2);
    assert_eq!(call.args[0].syntax, ArgSyntax::Positional);
    assert!(call.args[0].name.is_none());
    assert_eq!(call.args[1].syntax, ArgSyntax::Arrow);
    assert_eq!(
        parsed.resolver().resolve(call.args[1].name.expect("name")),
        "b"
    );
}

#[test]
fn all_positional_arguments_stay_positional() {
    let parsed = parse_with("SELECT f(1, 2)", PG_EXPR_DIALECT).expect("positional args parse");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert_eq!(call.args.len(), 2);
    assert!(
        call.args
            .iter()
            .all(|arg| arg.name.is_none() && arg.syntax == ArgSyntax::Positional),
    );
}

#[test]
fn named_arguments_are_rejected_under_ansi() {
    // The arrow lexemes are dialect-gated, so under ANSI `=>` and `:=` split back
    // into their bytes and the argument is a malformed positional expression.
    assert!(
        parse_with("SELECT f(a => 1)", TestDialect).is_err(),
        "ANSI rejects the `=>` named-argument arrow",
    );
    assert!(
        parse_with("SELECT f(a := 1)", TestDialect).is_err(),
        "ANSI rejects the `:=` named-argument separator",
    );
}

#[test]
fn named_arguments_round_trip_each_arrow() {
    for sql in [
        "SELECT f(a => 1)",
        "SELECT f(a := 1)",
        "SELECT f(1, b => 2)",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

// --- PostgreSQL/DuckDB VARIADIC call-site argument marker -------------------

#[test]
fn variadic_argument_marks_the_last_positional() {
    let parsed =
        parse_with("SELECT f(a, VARIADIC arr)", PG_EXPR_DIALECT).expect("VARIADIC arg parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert_eq!(call.args.len(), 2);
    assert!(!call.args[0].variadic, "the leading argument is ordinary");
    assert!(call.args[1].variadic, "the final argument carries VARIADIC");
    assert_eq!(call.args[1].syntax, ArgSyntax::Positional);
    assert!(call.args[1].name.is_none());
}

#[test]
fn variadic_argument_combines_with_a_named_argument() {
    // `VARIADIC name => value` is a valid engine form: the marker precedes the arrow.
    let parsed =
        parse_with("SELECT f(VARIADIC x => arr)", PG_EXPR_DIALECT).expect("VARIADIC named parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    let arg = &call.args[0];
    assert!(arg.variadic);
    assert_eq!(arg.syntax, ArgSyntax::Arrow);
    assert_eq!(parsed.resolver().resolve(arg.name.expect("name")), "x");
}

#[test]
fn variadic_argument_admits_the_aggregate_order_by_tail() {
    // The marker composes with the in-parenthesis ORDER BY (engine-accepted).
    let parsed = parse_with(
        "SELECT string_agg(VARIADIC arr ORDER BY b)",
        PG_EXPR_DIALECT,
    )
    .expect("VARIADIC with ORDER BY parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(call.args[0].variadic);
    assert_eq!(call.order_by.len(), 1);
}

#[test]
fn variadic_argument_rejected_when_not_last() {
    // Both PostgreSQL and DuckDB parse-reject a non-final VARIADIC.
    for sql in [
        "SELECT f(VARIADIC arr, a)",
        "SELECT f(a, VARIADIC arr, b)",
        "SELECT f(VARIADIC a, VARIADIC b)",
    ] {
        assert!(
            parse_with(sql, PG_EXPR_DIALECT).is_err(),
            "VARIADIC must be the final argument: {sql}",
        );
    }
}

#[test]
fn variadic_argument_rejected_with_a_quantifier() {
    // The VARIADIC func_application productions carry no ALL/DISTINCT quantifier, so
    // both engines parse-reject the combination.
    for sql in [
        "SELECT array_agg(DISTINCT VARIADIC arr)",
        "SELECT f(ALL a, VARIADIC arr)",
    ] {
        assert!(
            parse_with(sql, PG_EXPR_DIALECT).is_err(),
            "VARIADIC cannot combine with a quantifier: {sql}",
        );
    }
}

#[test]
fn variadic_argument_rejected_off_dialect() {
    // Off the gating dialects the VARIADIC prefix is not admitted, so the marker before
    // an argument surfaces as a parse error rather than an accepted spread.
    assert!(
        parse_with("SELECT f(a, VARIADIC arr)", TestDialect).is_err(),
        "ANSI does not admit the VARIADIC argument marker",
    );
}

#[test]
fn variadic_argument_parses_under_duckdb() {
    // Engine-probed: DuckDB parses the same VARIADIC marker (bind-layer catalog checks
    // aside).
    let parsed = parse_with("SELECT f(a, VARIADIC arr)", DuckDb).expect("DuckDB admits VARIADIC");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a function call");
    };
    assert!(call.args[1].variadic);
}

#[test]
fn variadic_argument_round_trips() {
    for sql in [
        "SELECT f(VARIADIC arr)",
        "SELECT f(a, VARIADIC arr)",
        "SELECT f(VARIADIC x => arr)",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

// --- PostgreSQL OPERATOR(...) explicit-operator infix ---------------------

#[test]
fn operator_construct_parses_unqualified() {
    let parsed =
        parse_with("SELECT a OPERATOR(+) b", PG_EXPR_DIALECT).expect("OPERATOR(...) parses");
    let Expr::NamedOperator { named_operator, .. } = project_expr(&parsed) else {
        panic!("expected a named-operator expression");
    };
    assert!(
        named_operator.schema.0.is_empty(),
        "an unqualified operator has no schema",
    );
    assert_eq!(parsed.resolver().resolve(named_operator.op), "+");
    assert!(matches!(named_operator.left, Expr::Column { .. }));
    assert!(matches!(named_operator.right, Expr::Column { .. }));
}

#[test]
fn operator_construct_parses_schema_qualified() {
    let parsed = parse_with("SELECT a OPERATOR(pg_catalog.+) b", PG_EXPR_DIALECT)
        .expect("schema-qualified OPERATOR(...) parses");
    let Expr::NamedOperator { named_operator, .. } = project_expr(&parsed) else {
        panic!("expected a named-operator expression");
    };
    assert_eq!(named_operator.schema.0.len(), 1);
    assert_eq!(
        parsed.resolver().resolve(named_operator.schema.0[0].sym),
        "pg_catalog",
    );
    assert_eq!(parsed.resolver().resolve(named_operator.op), "+");
}

#[test]
fn operator_construct_binds_at_the_any_other_operator_rank() {
    // `OPERATOR(...)` binds like `||` (rank 45): looser than `*` (60), so the
    // multiplication groups first on the right.
    let tight_right = parse_with("SELECT a OPERATOR(+) b * c", PG_EXPR_DIALECT).expect("parses");
    let Expr::NamedOperator { named_operator, .. } = project_expr(&tight_right) else {
        panic!("expected a named operator at the top");
    };
    assert!(
        matches!(
            named_operator.right,
            Expr::BinaryOp {
                op: BinaryOperator::Multiply,
                ..
            }
        ),
        "`*` binds tighter, so the right operand is `b * c`",
    );

    // And looser than `+` (50): the addition groups first on the left.
    let tight_left = parse_with("SELECT a + b OPERATOR(+) c", PG_EXPR_DIALECT).expect("parses");
    let Expr::NamedOperator { named_operator, .. } = project_expr(&tight_left) else {
        panic!("expected a named operator at the top");
    };
    assert!(
        matches!(
            named_operator.left,
            Expr::BinaryOp {
                op: BinaryOperator::Plus,
                ..
            }
        ),
        "`+` binds tighter, so the left operand is `a + b`",
    );
}

#[test]
fn operator_construct_is_rejected_under_ansi() {
    assert!(
        parse_with("SELECT a OPERATOR(+) b", TestDialect).is_err(),
        "ANSI has no explicit-operator infix form",
    );
}

#[test]
fn operator_construct_round_trips() {
    for sql in [
        "SELECT a OPERATOR(+) b",
        "SELECT a OPERATOR(pg_catalog.+) b",
        "SELECT a OPERATOR(+) b * c",
        "SELECT a + b OPERATOR(+) c",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn mysql_has_no_typed_interval_literal() {
    // MySQL has no first-class interval literal (`typed_interval_literal` off): every
    // prefix-typed `INTERVAL '…'` form the operator reader declines — the unit-less
    // `INTERVAL '1'` / `INTERVAL "x"` (where `"x"` is a MySQL string) and the ANSI
    // `TO`-composite / `(p)`-precision spellings — is `ER_PARSE_ERROR` on mysql:8.4.10
    // (engine-measured), in the standalone AND the `+`/`-` operand positions alike.
    for sql in [
        "SELECT INTERVAL '1'",
        "SELECT INTERVAL \"x\"",
        "SELECT * FROM t WHERE INTERVAL \"is\" > 1",
        "SELECT INTERVAL '1' HOUR TO SECOND",
        "SELECT INTERVAL '1' SECOND(3)",
        "SELECT INTERVAL '1-2' YEAR TO MONTH",
        "SELECT '2020-01-01' - INTERVAL '1' HOUR TO SECOND",
        "SELECT '2020-01-01' - INTERVAL '1' SECOND(3)",
    ] {
        assert!(
            parse_with(sql, MySql).is_err(),
            "MySQL rejects the typed interval literal {sql:?}",
        );
    }
    // The valid MySQL spellings ride the operator reader (`mysql_interval_operator`): a
    // string amount with a simple unit in a `+`/`-` operand or a window-frame bound (both
    // engine-measured accepts) must keep parsing with the literal path off.
    for sql in [
        "SELECT '2020-01-01' + INTERVAL '1' DAY",
        "SELECT SUM(x) OVER (ORDER BY b RANGE BETWEEN INTERVAL '1' DAY PRECEDING \
         AND CURRENT ROW) FROM t",
    ] {
        assert!(
            parse_with(sql, MySql).is_ok(),
            "MySQL parses the operator-position interval {sql:?}",
        );
    }
    // The literal path is off for MySQL only: PostgreSQL keeps the ANSI interval literal,
    // including the unit-less form whose fields default from the string.
    for sql in ["SELECT INTERVAL '1'", "SELECT INTERVAL '1' HOUR TO SECOND"] {
        assert!(
            parse_with(sql, Postgres).is_ok(),
            "PostgreSQL admits the interval literal {sql:?}",
        );
    }
}

#[test]
fn mysql_interval_operator_parses_round_trips_and_shapes() {
    use crate::dialect::Lenient;
    const MYSQL_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::MYSQL,
    };
    const LENIENT_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::LENIENT,
    };
    // The motivating gap: the bare-integer operator interval `NOW() - INTERVAL 3 DAY` and its
    // whole operand/unit surface, each round-tripping exactly under both the MySql preset and
    // Lenient (the union).
    let round_trip = [
        "SELECT NOW() - INTERVAL 3 DAY",
        "SELECT NOW() + INTERVAL 3 DAY",
        "SELECT INTERVAL 3 DAY + NOW()",
        "SELECT NOW() - INTERVAL 1.5 DAY",
        "SELECT NOW() - INTERVAL -3 DAY",
        "SELECT NOW() - INTERVAL ? DAY",
        "SELECT NOW() - INTERVAL @x DAY",
        "SELECT NOW() - INTERVAL 3 + 1 DAY",
        "SELECT NOW() - INTERVAL '3' DAY",
        "SELECT NOW() + INTERVAL '3-2' YEAR_MONTH",
        "SELECT NOW() + INTERVAL '1:2:3' HOUR_SECOND",
        "SELECT NOW() + INTERVAL 1 DAY_MICROSECOND",
        "SELECT NOW() + INTERVAL 1 MICROSECOND",
        "SELECT NOW() + INTERVAL 1 WEEK",
        "SELECT NOW() + INTERVAL 1 QUARTER",
        "SELECT NOW() - INTERVAL 1 DAY - INTERVAL 1 HOUR",
        "SELECT '2020-01-01' + INTERVAL n MONTH FROM t",
    ];
    for sql in round_trip {
        for dialect in ["MySql", "Lenient"] {
            let parsed = match dialect {
                "MySql" => parse_with(sql, MySql).unwrap_or_else(|err| panic!("{sql}: {err:?}")),
                _ => parse_with(sql, Lenient).unwrap_or_else(|err| panic!("{sql}: {err:?}")),
            };
            let rendered = match dialect {
                "MySql" => Renderer::new(MYSQL_RENDER).render_parsed(&parsed),
                _ => Renderer::new(LENIENT_RENDER).render_parsed(&parsed),
            }
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
            assert_eq!(rendered, sql, "exact round-trip for {sql} under {dialect}");
        }
    }
    // Structural: the operator interval is an `Expr::Interval` (a distinct node from the
    // ANSI/PostgreSQL typed-string `LiteralKind::Interval`), carrying the amount expression and
    // the mandatory unit.
    let parsed = parse_with("SELECT NOW() - INTERVAL 3 DAY", MySql).expect("parses");
    let Expr::BinaryOp { right, .. } = project_expr(&parsed) else {
        panic!("expected a binary subtraction");
    };
    let Expr::Interval { value, unit, .. } = &**right else {
        panic!("expected the RHS to be an Expr::Interval, got {right:?}");
    };
    assert_eq!(*unit, IntervalFields::Day);
    assert!(
        matches!(&**value, Expr::Literal { literal, .. } if literal.kind == LiteralKind::Integer),
        "the amount is the integer literal 3, got {value:?}",
    );
    // A parenthesized amount is a redundant grouping the AST does not retain (ADR-0008): the
    // amount round-trips paren-free because the unit keyword unambiguously terminates it.
    let parsed = parse_with("SELECT NOW() - INTERVAL (3 + 1) DAY", MySql).expect("parses");
    let rendered = Renderer::new(MYSQL_RENDER)
        .render_parsed(&parsed)
        .expect("renders");
    assert_eq!(rendered, "SELECT NOW() - INTERVAL 3 + 1 DAY");
    // The composite units use MySQL's underscore spelling, never the ANSI `TO` form.
    let parsed = parse_with("SELECT NOW() + INTERVAL 1 DAY_HOUR", MySql).expect("parses");
    let Expr::BinaryOp { right, .. } = project_expr(&parsed) else {
        panic!("binary");
    };
    assert!(matches!(
        &**right,
        Expr::Interval {
            unit: IntervalFields::DayToHour,
            ..
        }
    ));
}

#[test]
fn mysql_interval_operator_precedence_matches_left_assoc_additive() {
    // `a - INTERVAL 1 DAY + b` groups as `(a - INTERVAL 1 DAY) + b` — the interval is a primary
    // operand of `-`, and `+` (equal precedence, left-associative) stays at the outer level.
    let parsed = parse_with("SELECT a - INTERVAL 1 DAY + b FROM t", MySql).expect("parses");
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Plus,
        ..
    } = project_expr(&parsed)
    else {
        panic!("the outermost operator must be the trailing `+`");
    };
    let Expr::BinaryOp {
        op: BinaryOperator::Minus,
        right,
        ..
    } = &**left
    else {
        panic!("its left operand must be the `a - INTERVAL 1 DAY` subtraction");
    };
    assert!(
        matches!(&**right, Expr::Interval { .. }),
        "the subtraction's RHS is the interval operand",
    );
    // Rendering re-derives the grouping from binding power (ADR-0008): no spurious parens.
    let rendered = Renderer::new(FeatureDialect {
        features: &FeatureSet::MYSQL,
    })
    .render_parsed(&parsed)
    .expect("renders");
    assert_eq!(rendered, "SELECT a - INTERVAL 1 DAY + b FROM t");
}

#[test]
fn mysql_interval_operator_boundary_rejects_and_fallthrough() {
    // The unit is mandatory: a unit-less amount falls through to the typed-string literal path,
    // which is off under MySQL (`typed_interval_literal` — `INTERVAL '1'` is 1064 on mysql:8),
    // so `INTERVAL` falls back to a name and the trailing tokens do not parse.
    for sql in ["SELECT NOW() - INTERVAL 3", "SELECT INTERVAL '1'"] {
        assert!(
            parse_with(sql, MySql).is_err(),
            "MySQL rejects the unit-less interval {sql:?}",
        );
    }
    // Off the gate (PostgreSQL), the bare-integer operator interval is not recognized — `INTERVAL`
    // falls back to a name and the trailing `3 DAY` does not parse.
    assert!(
        parse_with("SELECT NOW() - INTERVAL 3 DAY", Postgres).is_err(),
        "the operator interval is gated off for PostgreSQL",
    );
    // The ANSI `TO` composite and `(p)` unit precision are declined by the operator reader (both
    // `ER_PARSE_ERROR` on mysql:8), so under Lenient they still parse via the typed-string
    // literal path (an ANSI/PostgreSQL interval literal); under MySQL that path is off and the
    // decline is a reject, matching the engine (`mysql_has_no_typed_interval_literal`).
    for sql in [
        "SELECT INTERVAL '1' HOUR TO SECOND",
        "SELECT INTERVAL '1' SECOND(3)",
    ] {
        assert!(
            parse_with(sql, crate::dialect::Lenient).is_ok(),
            "Lenient parses the ANSI interval literal {sql:?} via the literal path",
        );
        assert!(
            parse_with(sql, MySql).is_err(),
            "MySQL rejects the ANSI interval literal {sql:?}",
        );
    }
}

#[test]
fn mysql_builtin_aggregate_rejects_an_empty_argument_list() {
    // MySQL's dedicated aggregate grammar requires an argument (or the `COUNT(*)`
    // wildcard), so an empty aggregate call is `ER_PARSE_ERROR` on mysql:8
    // (`aggregate_calls_reject_empty_arguments`).
    for sql in [
        "SELECT COUNT()",
        "SELECT SUM()",
        "SELECT GROUP_CONCAT()",
        "SELECT JSON_ARRAYAGG()",
        "SELECT BIT_AND()",
    ] {
        assert!(
            parse_with(sql, MySql).is_err(),
            "MySQL rejects the empty aggregate call {sql:?}",
        );
    }
    // The `COUNT(*)` wildcard, an argumented aggregate, a niladic non-aggregate built-in,
    // an empty user-function call, and a qualified `db.count()` (a general stored-function
    // reference — not a single-part member) all stay accepted.
    for sql in [
        "SELECT COUNT(*)",
        "SELECT COUNT(a) FROM t",
        "SELECT NOW()",
        "SELECT my_udf()",
        "SELECT db.count()",
    ] {
        assert!(parse_with(sql, MySql).is_ok(), "MySQL parses {sql:?}",);
    }
    // MySQL-only: PostgreSQL admits an empty aggregate call (arity is a bind-time check).
    assert!(
        parse_with("SELECT COUNT()", Postgres).is_ok(),
        "PostgreSQL admits an empty COUNT() at parse time",
    );
}

#[test]
fn mysql_over_clause_requires_a_windowable_function() {
    // MySQL admits `OVER` only on an aggregate ∪ window function; `OVER` on a scalar
    // built-in or user function is `ER_PARSE_ERROR` on mysql:8
    // (`over_requires_windowable_function`). `PERCENTILE_CONT`/`ABS`/`ANY_VALUE` all reach
    // the general call path and are rejected by this gate.
    for sql in [
        "SELECT PERCENTILE_CONT(x, 0.5) OVER () FROM t",
        "SELECT ABS(x) OVER () FROM t",
        "SELECT ANY_VALUE(x) OVER () FROM t",
        "SELECT my_udf(x) OVER () FROM t",
    ] {
        assert!(
            parse_with(sql, MySql).is_err(),
            "MySQL rejects OVER on the non-windowable {sql:?}",
        );
    }
    // The same call without `OVER` parses, so it is the window clause the gate rejects, not
    // the call itself.
    assert!(
        parse_with("SELECT PERCENTILE_CONT(x, 0.5) FROM t", MySql).is_ok(),
        "the bare (non-windowed) call parses; only OVER is rejected",
    );
    // Every built-in *aggregate* stays windowable — the gate must not over-reject a valid
    // windowed aggregate. (The dedicated *window* functions `ROW_NUMBER`/`RANK`/… are
    // reserved words in MySQL that are now admitted as call heads and parse with their
    // mandatory `OVER`; their dedicated grammar is covered by
    // `mysql_window_functions_*` below.)
    for sql in [
        "SELECT SUM(x) OVER () FROM t",
        "SELECT COUNT(*) OVER () FROM t",
        "SELECT COUNT(x) OVER () FROM t",
        "SELECT GROUP_CONCAT(x) OVER () FROM t",
        "SELECT BIT_XOR(x) OVER () FROM t",
        "SELECT AVG(x) OVER () FROM t",
        "SELECT MIN(x) OVER () FROM t",
        "SELECT MAX(x) OVER () FROM t",
        "SELECT STDDEV(x) OVER () FROM t",
        "SELECT VARIANCE(x) OVER () FROM t",
    ] {
        assert!(
            parse_with(sql, MySql).is_ok(),
            "MySQL parses the windowed aggregate {sql:?}",
        );
    }
    // MySQL-only: PostgreSQL attaches `OVER` to any function.
    assert!(
        parse_with("SELECT ABS(x) OVER () FROM t", Postgres).is_ok(),
        "PostgreSQL admits OVER on any function",
    );
}

#[test]
fn mysql_window_functions_parse_as_call_heads_with_over() {
    // The 11 dedicated window functions are reserved words now admitted as call heads
    // (`mysql-reserved-window-function-names`), each parsing with its mandatory `OVER` and
    // its fixed engine-verified arity: the five rank/distribution functions take zero
    // arguments, `NTILE`/`FIRST_VALUE`/`LAST_VALUE` one, `LEAD`/`LAG` one-to-three, and
    // `NTH_VALUE` two. Every form below PREPAREs on mysql:8.
    for sql in [
        "SELECT ROW_NUMBER() OVER ()",
        "SELECT RANK() OVER ()",
        "SELECT DENSE_RANK() OVER ()",
        "SELECT PERCENT_RANK() OVER ()",
        "SELECT CUME_DIST() OVER ()",
        "SELECT NTILE(4) OVER ()",
        "SELECT LEAD(a) OVER () FROM t",
        "SELECT LEAD(a, 2) OVER () FROM t",
        "SELECT LEAD(a, 2, 0) OVER () FROM t",
        "SELECT LAG(a, 1) OVER () FROM t",
        "SELECT FIRST_VALUE(a) OVER () FROM t",
        "SELECT LAST_VALUE(a) OVER () FROM t",
        "SELECT NTH_VALUE(a, 2) OVER () FROM t",
        // A named window and a full window spec attach the same way.
        "SELECT ROW_NUMBER() OVER (PARTITION BY a ORDER BY b) FROM t",
        // Case-insensitive: the reserved-word carve-out is spelling-agnostic.
        "SELECT row_number() OVER ()",
        // The name is a reserved token, so a space before `(` still reads the window
        // function (unlike the spaced *aggregate* forms), matching mysql:8.
        "SELECT row_number () OVER ()",
    ] {
        assert!(
            parse_with(sql, MySql).is_ok(),
            "MySQL parses the window function {sql:?}",
        );
    }
}

#[test]
fn mysql_window_functions_require_over_and_fixed_arity() {
    // The converse half of the dedicated grammar: each violation is `ER_PARSE_ERROR` (1064)
    // on mysql:8, so the carve-out must not trade the old over-rejection for an
    // over-acceptance. Every statement below is engine-verified to REJECT on mysql:8.
    for sql in [
        // `OVER` is mandatory on a pure window function.
        "SELECT ROW_NUMBER()",
        "SELECT RANK() FROM t",
        "SELECT LEAD(a) FROM t",
        // Fixed arity: the zero-argument functions take no arguments.
        "SELECT ROW_NUMBER(1) OVER ()",
        "SELECT RANK(a) OVER () FROM t",
        // `NTILE`/`FIRST_VALUE`/`NTH_VALUE` take exactly one / one / two.
        "SELECT NTILE() OVER ()",
        "SELECT NTILE(4, 5) OVER ()",
        "SELECT FIRST_VALUE(a, b) OVER () FROM t",
        "SELECT NTH_VALUE(a) OVER () FROM t",
        "SELECT NTH_VALUE(a, 2, 3) OVER () FROM t",
        // `LEAD`/`LAG` span one-to-three; a fourth argument is a syntax error.
        "SELECT LEAD(a, 2, 3, 4) OVER () FROM t",
        // The aggregate-only argument forms are rejected on a window function.
        "SELECT ROW_NUMBER(*) OVER ()",
        "SELECT RANK(DISTINCT a) OVER () FROM t",
        "SELECT NTILE(DISTINCT a) OVER () FROM t",
    ] {
        assert!(
            parse_with(sql, MySql).is_err(),
            "MySQL rejects the malformed window call {sql:?}",
        );
    }
}

#[test]
fn mysql_window_function_tail_admits_respect_nulls_and_from_first() {
    // The post-`)` window-function tail (`mysql-window-function-tail-grammar`): the
    // null-treatment window functions admit `RESPECT NULLS`, and `NTH_VALUE` additionally
    // admits `FROM FIRST` (before the null treatment) — the over-rejections this ticket
    // removes. Every form PREPAREs on mysql:8; the tail must render in the post-`)`
    // position mysql accepts (never DuckDB's in-paren spelling) and re-parse.
    for (sql, fragment) in [
        (
            "SELECT LEAD(a) RESPECT NULLS OVER () FROM t",
            ") RESPECT NULLS OVER",
        ),
        (
            "SELECT LAG(a) RESPECT NULLS OVER () FROM t",
            ") RESPECT NULLS OVER",
        ),
        (
            "SELECT FIRST_VALUE(a) RESPECT NULLS OVER () FROM t",
            ") RESPECT NULLS OVER",
        ),
        (
            "SELECT LAST_VALUE(a) RESPECT NULLS OVER () FROM t",
            ") RESPECT NULLS OVER",
        ),
        (
            "SELECT NTH_VALUE(a, 2) RESPECT NULLS OVER () FROM t",
            ") RESPECT NULLS OVER",
        ),
        (
            "SELECT NTH_VALUE(a, 2) FROM FIRST OVER () FROM t",
            ") FROM FIRST OVER",
        ),
        (
            "SELECT NTH_VALUE(a, 2) FROM FIRST RESPECT NULLS OVER () FROM t",
            ") FROM FIRST RESPECT NULLS OVER",
        ),
        // The tail sits after the full LEAD offset/default argument list.
        (
            "SELECT LEAD(a, 1, 0) RESPECT NULLS OVER () FROM t",
            ") RESPECT NULLS OVER",
        ),
        // Case-insensitive: the tail keywords fold like the rest of the grammar, and
        // render canonically upper-cased.
        (
            "SELECT nth_value(a, 2) from first over () FROM t",
            ") FROM FIRST OVER",
        ),
    ] {
        let parsed =
            parse_with(sql, MySql).unwrap_or_else(|err| panic!("MySQL parses {sql:?}: {err:?}"));
        // The tail render is dialect-agnostic (it does not branch on the render target),
        // so any `RenderDialect` reproduces the post-`)` position; `MySql` is parser-only.
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert!(
            rendered.contains(fragment),
            "the tail renders in the post-`)` position for {sql:?}: got {rendered:?}",
        );
        parse_with(&rendered, MySql)
            .unwrap_or_else(|err| panic!("the rendered tail {rendered:?} re-parses: {err:?}"));
    }
}

#[test]
fn mysql_window_function_tail_rejects_the_unadmitted_forms() {
    // The tail is narrow — admitting more than mysql:8 does would trade the removed
    // over-rejection for an over-ACCEPTANCE. Every statement below REJECTs on mysql:8 (a
    // 1064 parse error or a 1235 not-yet-supported feature reject), so the parser rejects
    // it too, leaving the leftover keyword for the mandatory-`OVER` gate (or the trailing
    // tokens) to surface as a clean error.
    for sql in [
        // `IGNORE NULLS` grammar-admits but feature-rejects (1235) on the five
        // null-treatment functions — kept rejected to match the oracle.
        "SELECT LEAD(a) IGNORE NULLS OVER () FROM t",
        "SELECT FIRST_VALUE(a) IGNORE NULLS OVER () FROM t",
        "SELECT NTH_VALUE(a, 2) IGNORE NULLS OVER () FROM t",
        // `FROM LAST` likewise feature-rejects (1235), even on NTH_VALUE.
        "SELECT NTH_VALUE(a, 2) FROM LAST OVER () FROM t",
        // The null treatment is a parse error (1064) on the rank/distribution functions.
        "SELECT ROW_NUMBER() RESPECT NULLS OVER ()",
        "SELECT RANK() RESPECT NULLS OVER ()",
        "SELECT NTILE(4) RESPECT NULLS OVER ()",
        // `FROM {FIRST | LAST}` is a parse error (1064) on any function but NTH_VALUE.
        "SELECT FIRST_VALUE(a) FROM FIRST OVER () FROM t",
        "SELECT LEAD(a) FROM FIRST OVER () FROM t",
        // The clauses appear in a fixed order: the null treatment cannot precede `FROM`.
        "SELECT NTH_VALUE(a, 2) RESPECT NULLS FROM FIRST OVER () FROM t",
        // The tail is strictly post-`)`; the in-paren (DuckDB) spelling is rejected.
        "SELECT LEAD(a RESPECT NULLS) OVER () FROM t",
        "SELECT NTH_VALUE(a, 2 FROM FIRST) OVER () FROM t",
        // The built-in aggregates do not admit the null treatment.
        "SELECT SUM(a) RESPECT NULLS OVER () FROM t",
    ] {
        assert!(
            parse_with(sql, MySql).is_err(),
            "MySQL rejects the unadmitted window tail {sql:?}",
        );
    }
    // The tail is MySQL-specific: a non-MySQL dialect leaves both the window-tail and the
    // null-treatment flags off, so PostgreSQL rejects the post-`)` spelling.
    assert!(
        parse_with("SELECT LEAD(a) RESPECT NULLS OVER () FROM t", Postgres).is_err(),
        "PostgreSQL rejects the MySQL window-function tail",
    );
}

#[test]
fn mysql_window_function_names_reserved_outside_the_call_head() {
    // The carve-out is scoped to the call-head position: the names stay fully reserved as a
    // column reference and as an alias, matching mysql:8 (`SELECT ROW_NUMBER` and
    // `SELECT 1 AS row_number` are both `ER_PARSE_ERROR`).
    for sql in [
        "SELECT ROW_NUMBER",
        "SELECT ROW_NUMBER FROM t",
        "SELECT rank FROM t",
        "SELECT 1 AS row_number",
        "SELECT a AS lead FROM t",
    ] {
        assert!(
            parse_with(sql, MySql).is_err(),
            "MySQL keeps the window name reserved in {sql:?}",
        );
    }
    // A non-MySQL dialect is unaffected: `row_number`/`rank` are ordinary function names in
    // PostgreSQL, so a bare (non-windowed) call and a bare column both parse there.
    assert!(
        parse_with("SELECT row_number() OVER ()", Postgres).is_ok(),
        "PostgreSQL admits row_number() as an ordinary window call",
    );
    assert!(
        parse_with("SELECT row_number()", Postgres).is_ok(),
        "PostgreSQL does not require OVER on row_number() at parse time",
    );
    assert!(
        parse_with("SELECT rank FROM t", Postgres).is_ok(),
        "PostgreSQL admits `rank` as an ordinary column name",
    );
}

#[test]
fn postgres_sqljson_constructors_reject_empty_argument_list() {
    // `JSON()`/`JSON_SCALAR()`/`JSON_SERIALIZE()` require the context-item argument on
    // PostgreSQL (dedicated `gram.y` productions); the generic call path would otherwise
    // admit the niladic form.
    for sql in [
        "SELECT JSON()",
        "SELECT JSON_SCALAR()",
        "SELECT JSON_SERIALIZE()",
    ] {
        parse_with(sql, Postgres).expect_err(&format!(
            "SQL/JSON constructor requires an argument: {sql:?}"
        ));
    }
    // A one-argument constructor parses, and a quoted-name empty call stays an ordinary
    // (general) call PostgreSQL accepts — the gate keys on a single unquoted name.
    parse_with("SELECT JSON('{}')", Postgres).expect("JSON('{}') parses");
    parse_with("SELECT JSON_SCALAR(1)", Postgres).expect("JSON_SCALAR(1) parses");
    parse_with("SELECT \"json\"()", Postgres).expect("quoted \"json\"() is a general call");
    // Off-dialect the same names are ordinary niladic calls (no PostgreSQL keyword grammar).
    parse_with("SELECT json()", MySql).expect("MySQL treats json() as a general call");
}

#[test]
fn postgres_merge_action_support_function() {
    // PostgreSQL's `merge_action()` is a dedicated zero-argument grammar production
    // (`MERGE_ACTION '(' ')'`) raw-parse-accepted anywhere an expression is (the
    // MERGE-RETURNING-only restriction is a parse-analysis check); it folds into the
    // canonical `Function` shape with the name `merge_action` and no arguments.
    let parsed = parse_with("SELECT merge_action()", Postgres).expect("merge_action() parses");
    let Expr::Function { call, .. } = project_expr(&parsed) else {
        panic!("expected a merge_action function call");
    };
    assert_eq!(
        parsed.resolver().resolve(call.name.0[0].sym),
        "merge_action"
    );
    assert!(call.args.is_empty() && !call.wildcard && call.over.is_none());

    // Round-trips through rendering (`merge_action()` with its empty parentheses).
    let rendered = Renderer::new(Postgres)
        .render_parsed(&parsed)
        .expect("merge_action() renders");
    assert_eq!(rendered, "SELECT merge_action()");

    // Valid in its home position — a `MERGE ... RETURNING` list — and, since the raw parse
    // admits it anywhere, in an ordinary SELECT/CTE too.
    for sql in [
        "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET x = 1 RETURNING merge_action()",
        "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET x = 1 RETURNING merge_action() AS act",
        "SELECT merge_action() FROM t",
    ] {
        parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
    }

    // The parens are strictly empty — an argument or an `OVER` tail is a syntax error,
    // matching PostgreSQL (`merge_action(1)` / `merge_action() OVER ()` both engine-probed
    // rejecting).
    for sql in ["SELECT merge_action(1)", "SELECT merge_action() OVER ()"] {
        assert!(parse_with(sql, Postgres).is_err(), "{sql} must be rejected");
    }

    // Lenient inherits the form; ANSI (which reserves the keyword with no call form) leaves
    // it the reject it already was — the flag does not widen off PostgreSQL.
    parse_with("SELECT merge_action()", crate::dialect::Lenient).expect("Lenient admits it");
    assert!(
        parse_with("SELECT merge_action()", crate::dialect::Ansi).is_err(),
        "ANSI has no merge_action() support function",
    );
}

#[test]
fn postgres_collation_for_expression() {
    use crate::ast::StringFunc;
    // PostgreSQL's `COLLATION FOR (<expr>)` common-subexpr: a dedicated
    // `COLLATION FOR '(' a_expr ')'` production reporting the collation name for its
    // operand. It keeps its keyword surface as `StringFunc::CollationFor` (not folded to
    // the lowered `pg_collation_for(...)` call), so it round-trips as written.
    let parsed =
        parse_with("SELECT COLLATION FOR ('foo')", Postgres).expect("COLLATION FOR (expr) parses");
    let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::StringFunc");
    };
    assert!(matches!(
        string_func.as_ref(),
        StringFunc::CollationFor { .. }
    ));

    let rendered = Renderer::new(Postgres)
        .render_parsed(&parsed)
        .expect("COLLATION FOR renders");
    assert_eq!(rendered, "SELECT COLLATION FOR ('foo')");

    // The operand is a general `a_expr` (column refs, qualified names, operators), and
    // the form is valid anywhere an expression is.
    for sql in [
        "SELECT COLLATION FOR (a.b)",
        "SELECT COLLATION FOR (col1 || col2)",
        "SELECT COLLATION FOR (x) FROM t",
    ] {
        parse_with(sql, Postgres).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
    }

    // The parentheses and single operand are mandatory — no parens, an empty list, a
    // two-argument list, or a bare `SELECT` operand all reject (engine-probed against
    // pg_query).
    for sql in [
        "SELECT COLLATION FOR 'foo'",
        "SELECT COLLATION FOR ()",
        "SELECT COLLATION FOR ('foo', 'bar')",
        "SELECT COLLATION FOR (select 1)",
    ] {
        assert!(
            parse_with(sql, Postgres).is_err(),
            "{sql:?} must be rejected"
        );
    }

    // A plain `collation(x)` call is unaffected by the special form — it stays an
    // ordinary function call under every dialect.
    parse_with("SELECT collation('foo')", Postgres).expect("plain collation() call parses");

    // Lenient inherits the form; ANSI has no `COLLATION FOR` production, so it stays the
    // reject it already was — the flag does not widen off PostgreSQL.
    parse_with("SELECT COLLATION FOR ('foo')", crate::dialect::Lenient)
        .expect("Lenient admits COLLATION FOR");
    assert!(
        parse_with("SELECT COLLATION FOR ('foo')", crate::dialect::Ansi).is_err(),
        "ANSI has no COLLATION FOR (expr) common-subexpr",
    );
}

#[test]
fn ceil_to_field_special_form() {
    use crate::ast::{CeilSpelling, StringFunc};
    use crate::dialect::Lenient;

    // `CEIL(<expr> TO <field>)` is sqlparser-rs-parity surface only — no probed oracle
    // grammar admits it (pg_query/DuckDB/mysql:8.4.10 all reject the `TO` tail), so the
    // gate is Lenient-only. The `TO`-form parses to `StringFunc::CeilTo` and round-trips.
    let parsed = parse_with("SELECT CEIL(x TO DAY)", Lenient)
        .expect("CEIL(x TO field) parses under Lenient");
    let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::StringFunc");
    };
    let StringFunc::CeilTo {
        field, spelling, ..
    } = string_func.as_ref()
    else {
        panic!("expected StringFunc::CeilTo");
    };
    assert_eq!(parsed.resolver().resolve(field.sym), "DAY");
    assert_eq!(*spelling, CeilSpelling::Ceil);
    assert_eq!(
        Renderer::new(Lenient)
            .render_parsed(&parsed)
            .expect("CEIL(x TO field) renders"),
        "SELECT CEIL(x TO DAY)",
    );

    // The `CEILING` spelling carries the same grammar and round-trips as written.
    let parsed = parse_with("SELECT CEILING(x TO HOUR)", Lenient)
        .expect("CEILING(x TO field) parses under Lenient");
    let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::StringFunc");
    };
    let StringFunc::CeilTo { spelling, .. } = string_func.as_ref() else {
        panic!("expected StringFunc::CeilTo");
    };
    assert_eq!(*spelling, CeilSpelling::Ceiling);
    assert_eq!(
        Renderer::new(Lenient)
            .render_parsed(&parsed)
            .expect("CEILING(x TO field) renders"),
        "SELECT CEILING(x TO HOUR)",
    );

    // The comma scale spelling stays an ordinary call everywhere — the special form
    // fires only on `TO`, never intercepting the comma-arity form.
    let parsed =
        parse_with("SELECT CEIL(x, 2)", Lenient).expect("CEIL(x, 2) parses as an ordinary call");
    assert!(
        matches!(project_expr(&parsed), Expr::Function { .. }),
        "CEIL(x, 2) must stay an ordinary Expr::Function, not StringFunc::CeilTo",
    );

    // With the gate off (ANSI has no `ceil_to_field`), `CEIL(x TO DAY)` is the same clean
    // parse error it is today: an unexpected `TO` where the plain-call path expects `,`
    // or `)`.
    assert!(
        parse_with("SELECT CEIL(x TO DAY)", crate::dialect::Ansi).is_err(),
        "ANSI has no CEIL TO-field special form",
    );

    // No probed oracle grammar admits the `TO` tail (decision table (a)): PostgreSQL
    // parse-rejects it too, even though it has ordinary `ceil`/`ceiling` functions.
    assert!(
        parse_with("SELECT CEIL(x TO DAY)", Postgres).is_err(),
        "PostgreSQL has no CEIL TO-field grammar (pg_query-verified)",
    );
}

#[test]
fn floor_to_field_special_form() {
    use crate::ast::StringFunc;
    use crate::dialect::Lenient;

    // `FLOOR(<expr> TO <field>)` is sqlparser-rs-parity surface only — no probed oracle
    // grammar admits it (pg_query/DuckDB/mysql:8.4.10 all reject the `TO` tail), so the
    // gate is Lenient-only. The `TO`-form parses to `StringFunc::FloorTo` and
    // round-trips. Unlike `CEIL`/`CEILING`, `FLOOR` has no synonym spelling to track.
    let parsed = parse_with("SELECT FLOOR(x TO DAY)", Lenient)
        .expect("FLOOR(x TO field) parses under Lenient");
    let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::StringFunc");
    };
    let StringFunc::FloorTo { field, .. } = string_func.as_ref() else {
        panic!("expected StringFunc::FloorTo");
    };
    assert_eq!(parsed.resolver().resolve(field.sym), "DAY");
    assert_eq!(
        Renderer::new(Lenient)
            .render_parsed(&parsed)
            .expect("FLOOR(x TO field) renders"),
        "SELECT FLOOR(x TO DAY)",
    );

    // The comma scale spelling stays an ordinary call everywhere — the special form
    // fires only on `TO`, never intercepting the comma-arity form.
    let parsed =
        parse_with("SELECT FLOOR(x, 2)", Lenient).expect("FLOOR(x, 2) parses as an ordinary call");
    assert!(
        matches!(project_expr(&parsed), Expr::Function { .. }),
        "FLOOR(x, 2) must stay an ordinary Expr::Function, not StringFunc::FloorTo",
    );

    // With the gate off (ANSI has no `floor_to_field`), `FLOOR(x TO DAY)` is the same
    // clean parse error it is today: an unexpected `TO` where the plain-call path
    // expects `,` or `)`.
    assert!(
        parse_with("SELECT FLOOR(x TO DAY)", crate::dialect::Ansi).is_err(),
        "ANSI has no FLOOR TO-field special form",
    );

    // No probed oracle grammar admits the `TO` tail (decision table (a)): PostgreSQL
    // parse-rejects it too, even though it has an ordinary `floor` function.
    assert!(
        parse_with("SELECT FLOOR(x TO DAY)", Postgres).is_err(),
        "PostgreSQL has no FLOOR TO-field grammar (pg_query-verified)",
    );
}

#[test]
fn mysql_convert_special_form() {
    use crate::ast::{CastSyntax, StringFunc};
    use crate::dialect::Lenient;

    // Comma form `CONVERT(<expr>, <type>)` folds onto the one `Expr::Cast` shape with the
    // `Convert` spelling tag — engine-verified on mysql:8.4.10.
    let parsed =
        parse_with("SELECT CONVERT(1, SIGNED)", MySql).expect("CONVERT(expr, type) parses");
    let Expr::Cast { syntax, .. } = project_expr(&parsed) else {
        panic!("expected Expr::Cast for the comma form");
    };
    assert_eq!(*syntax, CastSyntax::Convert);

    // USING form `CONVERT(<expr> USING <charset>)` is the transcoding special form —
    // `StringFunc::ConvertUsing`, not a cast.
    let parsed = parse_with("SELECT CONVERT('x' USING utf8mb4)", MySql)
        .expect("CONVERT(expr USING cs) parses");
    let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::StringFunc for the USING form");
    };
    assert!(matches!(
        string_func.as_ref(),
        StringFunc::ConvertUsing { .. }
    ));

    // Round-trip: the `CONVERT(...)` wrapper (comma and USING shapes) renders as written —
    // engine-verified accepts on mysql:8.4. Parsed and rendered under `Lenient`, which
    // inherits the special form and implements `RenderDialect` (the parser-only `MySql`
    // dialect does not). The targets here render stably under Lenient's ANSI type target;
    // the `CHAR`-spelling cases below exercise acceptance only, since ANSI canonicalizes
    // `CHAR` -> `CHARACTER` (a pre-existing type-spelling fold orthogonal to `CONVERT`).
    for sql in [
        "SELECT CONVERT(1, SIGNED)",
        "SELECT CONVERT('1.5', DECIMAL(10, 2))",
        "SELECT CONVERT(1 + 2 USING utf8mb4)",
        "SELECT CONVERT('x' USING `utf8mb4`)",
        "SELECT CONVERT('x' USING 'latin1')",
        "SELECT CONVERT('x' USING binary)",
    ] {
        let parsed = parse_with(sql, Lenient).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|e| panic!("{sql:?} renders: {e:?}")),
            sql,
            "{sql:?} round-trips"
        );
    }

    // Accepts (engine-verified on mysql:8.4): the charset-annotated `CHAR` comma target
    // rides in free via the shared type grammar, and the two forms nest.
    for sql in [
        "SELECT CONVERT('x', CHAR(10))",
        "SELECT CONVERT('x', CHAR(10) CHARACTER SET utf8mb4)",
        "SELECT CONVERT(CONVERT('x', CHAR) USING utf8mb4)",
        "SELECT CONVERT(CONVERT('x' USING utf8mb4), CHAR)",
    ] {
        parse_with(sql, MySql).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
    }

    // The comma form shares CAST's restricted `cast_type` gate: a non-`cast_type` target
    // (and the charset annotation off `CHAR`) rejects exactly as the matching CAST does,
    // and the malformed shapes reject — all engine-verified `ER_PARSE_ERROR` on mysql:8.4.
    for sql in [
        "SELECT CONVERT(1, INT)",
        "SELECT CONVERT('x', VARCHAR)",
        "SELECT CONVERT('x', VARCHAR(5) CHARACTER SET utf8mb4)",
        "SELECT CONVERT('x', NCHAR CHARACTER SET utf8mb4)",
        "SELECT CONVERT('x')",
        "SELECT CONVERT('x', CHAR, BINARY)",
        "SELECT CONVERT('x' AS CHAR)",
        "SELECT CONVERT('x' USING)",
    ] {
        assert!(parse_with(sql, MySql).is_err(), "MySQL rejects {sql:?}");
    }

    // Lenient inherits the special form. Lenient's cast targets are unrestricted, so its
    // comma form admits any type (`restricted_cast_targets` is off there).
    parse_with("SELECT CONVERT('x' USING utf8mb4)", Lenient).expect("Lenient admits CONVERT USING");
    parse_with("SELECT CONVERT(1, INT)", Lenient).expect("Lenient CONVERT admits any target");

    // Off elsewhere: with the gate off, `CONVERT` keeps its ordinary function-name
    // reading, so the special-form-only shapes are not produced. The `USING` form is a
    // parse error under ANSI (it has no such production), matching PostgreSQL.
    assert!(
        parse_with("SELECT CONVERT('x' USING utf8mb4)", Ansi).is_err(),
        "ANSI has no CONVERT USING production",
    );
}

#[test]
fn mysql_match_against_fulltext() {
    use crate::ast::{MatchSearchModifier, StringFunc};
    use crate::dialect::Lenient;

    // MySQL's full-text `MATCH (cols) AGAINST (operand [modifier])` special form parses to
    // `StringFunc::MatchAgainst` — the full grammar was PREPARE-probed on mysql:8.4.10.
    let parsed = parse_with("SELECT MATCH(a, b) AGAINST('x') FROM t", MySql)
        .expect("MATCH ... AGAINST parses");
    let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::StringFunc");
    };
    let StringFunc::MatchAgainst {
        columns, modifier, ..
    } = string_func.as_ref()
    else {
        panic!("expected StringFunc::MatchAgainst");
    };
    assert_eq!(columns.len(), 2);
    assert!(modifier.is_none());

    // Each documented modifier combination parses to its enum variant (the default — no
    // modifier words — stays `None`) and round-trips as written. Rendered under Lenient,
    // since the parser-only `MySql` dialect does not implement `RenderDialect`.
    for (sql, want) in [
        ("SELECT MATCH(a, b) AGAINST('x') FROM t", None),
        (
            "SELECT MATCH(a, b) AGAINST('x' IN NATURAL LANGUAGE MODE) FROM t",
            Some(MatchSearchModifier::NaturalLanguage),
        ),
        (
            "SELECT MATCH(a, b) AGAINST('x' IN NATURAL LANGUAGE MODE WITH QUERY EXPANSION) FROM t",
            Some(MatchSearchModifier::NaturalLanguageQueryExpansion),
        ),
        (
            "SELECT MATCH(a, b) AGAINST('x' IN BOOLEAN MODE) FROM t",
            Some(MatchSearchModifier::Boolean),
        ),
        (
            "SELECT MATCH(a, b) AGAINST('x' WITH QUERY EXPANSION) FROM t",
            Some(MatchSearchModifier::QueryExpansion),
        ),
    ] {
        let parsed = parse_with(sql, Lenient).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
            panic!("{sql:?}: expected Expr::StringFunc");
        };
        let StringFunc::MatchAgainst { modifier, .. } = string_func.as_ref() else {
            panic!("{sql:?}: expected StringFunc::MatchAgainst");
        };
        assert_eq!(*modifier, want, "{sql:?} modifier");
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|e| panic!("{sql:?} renders: {e:?}")),
            sql,
            "{sql:?} round-trips"
        );
    }

    // The column list admits qualified (1–3-part dotted) column references and the operand
    // is a full `bit_expr` (arithmetic/bitwise/params/subqueries) — all engine-accepted.
    for sql in [
        "SELECT MATCH(t.a, t.b) AGAINST('x') FROM t",
        "SELECT MATCH(a) AGAINST(1 + 2) FROM t",
        "SELECT MATCH(a, b) AGAINST(concat('x', 'y')) FROM t",
        "SELECT 1 FROM t WHERE MATCH(a, b) AGAINST('x') > 0.5",
        "SELECT 1 FROM t ORDER BY MATCH(a, b) AGAINST('x') DESC",
    ] {
        parse_with(sql, MySql).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
    }

    // Reject shapes, each `ER_PARSE_ERROR` on mysql:8.4.10: an empty column list, a
    // non-column-reference list item, an empty/absent `AGAINST` operand, a comparison
    // operand (`bit_expr` excludes it), and every malformed modifier tail.
    for sql in [
        "SELECT MATCH() AGAINST('x') FROM t",
        "SELECT MATCH(a + 1) AGAINST('x') FROM t",
        "SELECT MATCH('lit') AGAINST('x') FROM t",
        "SELECT MATCH(a, b) AGAINST() FROM t",
        "SELECT MATCH(a, b) FROM t",
        "SELECT MATCH(a, b) AGAINST(1 > 2) FROM t",
        "SELECT MATCH(a, b) AGAINST('x' IN BOOLEAN MODE WITH QUERY EXPANSION) FROM t",
        "SELECT MATCH(a, b) AGAINST('x' IN QUERY EXPANSION) FROM t",
        "SELECT MATCH(a, b) AGAINST('x' IN NATURAL LANGUAGE) FROM t",
        "SELECT MATCH(a, b) AGAINST('x' WITH EXPANSION) FROM t",
    ] {
        assert!(parse_with(sql, MySql).is_err(), "MySQL rejects {sql:?}");
    }

    // The gate is MySQL/Lenient-only. SQLite keeps its infix `<expr> MATCH <expr>`
    // operator untouched (a binding-power entry, not this prefix special form), and a
    // dialect without the flag never produces `MatchAgainst`.
    parse_with("SELECT 'abc' MATCH 'a'", Sqlite).expect("SQLite infix MATCH operator still parses");
    assert!(
        parse_with("SELECT MATCH(a, b) AGAINST('x') FROM t", Ansi).is_err(),
        "ANSI has no MATCH ... AGAINST special form",
    );
}

#[test]
fn postgres_rejects_invalid_uescape_delimiter() {
    // PostgreSQL's `check_uescapechar`: the delimiter must be a single character that is not
    // a hex digit, `+`, a single/double quote, or whitespace.
    for sql in [
        "SELECT U&'wrong: +0061' UESCAPE '+'",
        "SELECT U&'d0061' UESCAPE '5'",
        "SELECT U&'d0061' UESCAPE 'a'",
        "SELECT U&'d0061' UESCAPE ''''",
        "SELECT U&'d0061' UESCAPE '!!'",
    ] {
        parse_with(sql, Postgres).expect_err(&format!("invalid UESCAPE delimiter: {sql:?}"));
    }
    // Legal single-character delimiters — including `-` and `\`, which `check_uescapechar`
    // permits (only `+` among the sign characters is forbidden).
    for sql in [
        "SELECT U&'d!0061' UESCAPE '!'",
        "SELECT U&'d0061' UESCAPE '-'",
        "SELECT U&'d0061' UESCAPE 'g'",
        "SELECT U&'d0061' UESCAPE '\\'",
    ] {
        parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
    }
}

// --- SQL/JSON expression functions (pg-sqljson-expression-functions) ---------------

#[test]
fn json_value_query_exists_map_to_json_func() {
    use crate::ast::{JsonFuncKind, JsonWrapperBehavior};
    for (sql, kind) in [
        (
            "SELECT JSON_VALUE(js, '$' RETURNING int DEFAULT 0 ON EMPTY ERROR ON ERROR)",
            JsonFuncKind::Value,
        ),
        (
            "SELECT JSON_QUERY(js, '$' WITH CONDITIONAL WRAPPER OMIT QUOTES)",
            JsonFuncKind::Query,
        ),
        (
            "SELECT JSON_EXISTS(js, '$' PASSING 1 AS a TRUE ON ERROR)",
            JsonFuncKind::Exists,
        ),
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        let Expr::JsonFunc { json_func, .. } = project_expr(&parsed) else {
            panic!("{sql:?}: expected Expr::JsonFunc");
        };
        assert_eq!(json_func.kind, kind);
        // Only JSON_QUERY carries a wrapper; the others leave it unspecified.
        if matches!(kind, JsonFuncKind::Query) {
            assert_eq!(json_func.wrapper, JsonWrapperBehavior::Conditional);
        } else {
            assert_eq!(json_func.wrapper, JsonWrapperBehavior::Unspecified);
        }
    }
}

#[test]
fn json_object_standard_form_vs_legacy_function() {
    // A `key : value` / `key VALUE value` member list is the standard constructor.
    for sql in [
        "SELECT JSON_OBJECT('a': 1)",
        "SELECT JSON_OBJECT('a' VALUE 1, 'b': 2 ABSENT ON NULL WITH UNIQUE KEYS RETURNING jsonb)",
        "SELECT JSON_OBJECT(RETURNING jsonb)",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        assert!(
            matches!(project_expr(&parsed), Expr::JsonObject { .. }),
            "{sql:?}: expected Expr::JsonObject",
        );
    }
    // A plain argument list (no `:`/`VALUE`) is the legacy `json_object(text[])` call.
    for sql in [
        "SELECT JSON_OBJECT('{a,1}')",
        "SELECT JSON_OBJECT('{a}', '{1}')",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        assert!(
            matches!(project_expr(&parsed), Expr::Function { .. }),
            "{sql:?}: expected the legacy Expr::Function",
        );
    }
}

#[test]
fn is_json_predicate_parses() {
    use crate::ast::JsonItemType;
    let parsed = parse_with("SELECT js IS NOT JSON ARRAY WITH UNIQUE KEYS", Postgres)
        .expect("IS JSON predicate parses");
    let Expr::IsJson { is_json, .. } = project_expr(&parsed) else {
        panic!("expected Expr::IsJson");
    };
    assert!(is_json.negated);
    assert_eq!(is_json.item_type, JsonItemType::Array);
    assert!(is_json.unique_keys);
}

#[test]
fn sqljson_over_accept_boundary_rejects() {
    // Each rejects at PostgreSQL's raw parse (engine-verified against pg_query), so
    // accepting any of these would be an over-acceptance.
    for sql in [
        "SELECT JSON_OBJECT(1 + 2 VALUE 3)", // VALUE key must be a `c_expr`
        "SELECT JSON('1' FORMAT JSON ENCODING foo)", // encoding is validated at parse
        "SELECT JSON('1' FORMAT JSONB)",     // only FORMAT JSON exists
        "SELECT JSON_EXISTS(js, '$' RETURNING int)", // JSON_EXISTS takes no RETURNING
        "SELECT JSON_VALUE(js, '$' WITH WRAPPER)", // wrapper is JSON_QUERY-only
        "SELECT JSON_SCALAR(1 FORMAT JSON)", // JSON_SCALAR takes a plain arg
        "SELECT JSON_QUERY(js, '$' WITH ARRAY)", // WRAPPER keyword is mandatory
        "SELECT JSON_OBJECTAGG('k': v ORDER BY v)", // JSON_OBJECTAGG has no ORDER BY
    ] {
        assert!(
            parse_with(sql, Postgres).is_err(),
            "{sql:?}: PostgreSQL rejects this at raw parse, so we must too",
        );
    }
}

#[test]
fn sqljson_gated_off_leaves_keywords_as_names() {
    // The special forms are PostgreSQL/Lenient only; under ANSI/MySQL the clause-tail
    // forms are left unconsumed and reject (the keyword heads stay ordinary names).
    for sql in [
        "SELECT JSON_VALUE(js, '$' RETURNING int)",
        "SELECT JSON_QUERY(js, '$' WITH WRAPPER)",
        "SELECT JSON_OBJECT('a': 1)",
    ] {
        parse_with(sql, TestDialect).expect_err(&format!("{sql:?}: ANSI has no SQL/JSON forms"));
        parse_with(sql, MySql)
            .expect_err(&format!("{sql:?}: MySQL has no SQL/JSON standard forms"));
    }
}

#[test]
fn sqljson_forms_round_trip() {
    for sql in [
        "SELECT JSON_VALUE(js, '$' RETURNING int DEFAULT 0 ON EMPTY ERROR ON ERROR)",
        "SELECT JSON_QUERY(js, '$' RETURNING jsonb WITH WRAPPER OMIT QUOTES)",
        "SELECT JSON_EXISTS(js, '$' PASSING 1 AS a UNKNOWN ON ERROR)",
        "SELECT JSON_OBJECT('a': 1 ABSENT ON NULL WITH UNIQUE KEYS RETURNING jsonb)",
        "SELECT JSON_ARRAY(1, 2 NULL ON NULL RETURNING jsonb)",
        "SELECT JSON_OBJECTAGG('k' VALUE v RETURNING jsonb) FILTER (WHERE v > 0)",
        "SELECT JSON_ARRAYAGG(v ORDER BY v RETURNING jsonb) OVER ()",
        "SELECT JSON('1' FORMAT JSON ENCODING UTF8 WITH UNIQUE KEYS)",
        "SELECT JSON_SERIALIZE('1' RETURNING text FORMAT JSON)",
        "SELECT js IS NOT JSON OBJECT WITH UNIQUE KEYS",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|e| panic!("{sql:?}: {e}"));
        // The render must re-parse to the same string (stable round-trip).
        let reparsed = parse_with(&rendered, Postgres)
            .unwrap_or_else(|e| panic!("re-parse of {rendered:?}: {e:?}"));
        let rerendered = Renderer::new(Postgres)
            .render_parsed(&reparsed)
            .unwrap_or_else(|e| panic!("{rendered:?}: {e}"));
        assert_eq!(rendered, rerendered, "stable round-trip for {sql:?}");
    }
}

// --- SQL/XML expression functions (pg-xml-expression-functions) --------------------

#[test]
fn xml_functions_map_to_xml_func_variants() {
    use crate::ast::{XmlFunc, XmlStandalone};
    // Each head lowers to the matching `XmlFunc` variant.
    let parsed = parse_with(
        "SELECT xmlelement(NAME root, xmlattributes('v' AS a, 1 + 1 AS n), 'body', xmlelement(NAME leaf))",
        Postgres,
    )
    .expect("xmlelement parses");
    let Expr::XmlFunc { xml_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::XmlFunc");
    };
    let XmlFunc::Element {
        attributes,
        content,
        ..
    } = xml_func.as_ref()
    else {
        panic!("expected XmlFunc::Element");
    };
    assert_eq!(attributes.len(), 2);
    // The attribute AS-name is a ColLabel (an ordinary keyword, `a`/`n` here); its presence
    // is what distinguishes an aliased attribute from a bare value.
    assert!(attributes.iter().all(|attr| attr.name.is_some()));
    assert_eq!(content.len(), 2); // 'body' + the nested xmlelement

    // xmlroot's mandatory VERSION as NO VALUE, plus a STANDALONE NO VALUE tail.
    let parsed = parse_with(
        "SELECT xmlroot(x, version no value, standalone no value)",
        Postgres,
    )
    .expect("xmlroot parses");
    let Expr::XmlFunc { xml_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::XmlFunc");
    };
    let XmlFunc::Root {
        version,
        standalone,
        ..
    } = xml_func.as_ref()
    else {
        panic!("expected XmlFunc::Root");
    };
    assert!(version.is_none(), "VERSION NO VALUE is a None version");
    assert_eq!(*standalone, XmlStandalone::NoValue);
}

#[test]
fn is_document_predicate_parses() {
    let parsed =
        parse_with("SELECT xml '<a/>' IS NOT DOCUMENT", Postgres).expect("IS NOT DOCUMENT parses");
    let Expr::IsDocument { negated, .. } = project_expr(&parsed) else {
        panic!("expected Expr::IsDocument");
    };
    assert!(negated);
}

#[test]
fn xml_over_accept_boundary_rejects() {
    // Each rejects at PostgreSQL's raw parse (engine-verified against pg_query), so
    // accepting any of these would be an over-acceptance.
    for sql in [
        "SELECT xmlelement(foo)", // NAME keyword mandatory
        "SELECT xmlelement(NAME foo, 'c', xmlattributes(1 AS a))", // attributes must precede content
        "SELECT xmlforest()",                                      // element list is non-empty
        "SELECT xmlparse(x)",                                      // DOCUMENT/CONTENT mandatory
        "SELECT xmlparse(document x whitespace)", // WHITESPACE needs PRESERVE/STRIP
        "SELECT xmlpi(name foo, 'a', 'b')",       // xmlpi takes a single content
        "SELECT xmlroot(x)",                      // VERSION clause mandatory
        "SELECT xmlroot(x, version '1.0', standalone maybe)", // STANDALONE value is closed
        "SELECT xmlserialize(x as text)",         // DOCUMENT/CONTENT mandatory
        "SELECT xmlserialize(document x)",        // AS <type> mandatory
        "SELECT xmlexists('//a')",                // PASSING mandatory
        "SELECT xmlexists('a' || 'b' passing x)", // path is a `c_expr`, not `a_expr`
    ] {
        assert!(
            parse_with(sql, Postgres).is_err(),
            "{sql:?}: PostgreSQL rejects this at raw parse, so we must too",
        );
    }
}

#[test]
fn xml_gated_off_leaves_keywords_as_names() {
    // The special forms are PostgreSQL/Lenient only; under ANSI/MySQL the clause-tail
    // forms are left unconsumed and reject (the keyword heads stay ordinary names).
    for sql in [
        "SELECT xmlelement(NAME root, 'body')",
        "SELECT xmlserialize(DOCUMENT x AS text)",
        "SELECT x IS DOCUMENT",
    ] {
        parse_with(sql, TestDialect).expect_err(&format!("{sql:?}: ANSI has no SQL/XML forms"));
        parse_with(sql, MySql).expect_err(&format!("{sql:?}: MySQL has no SQL/XML forms"));
    }
}

#[test]
fn xmlagg_stays_an_ordinary_aggregate() {
    // `xmlagg` is not a keyword special form; it parses through the ordinary aggregate call
    // path (with ORDER BY) as an `Expr::Function`, in every dialect.
    let parsed =
        parse_with("SELECT xmlagg(x ORDER BY y)", Postgres).expect("xmlagg parses as a call");
    assert!(
        matches!(project_expr(&parsed), Expr::Function { .. }),
        "xmlagg is an ordinary aggregate call, not a special form",
    );
}

#[test]
fn xml_forms_round_trip() {
    for sql in [
        "SELECT xmlelement(NAME root, xmlattributes('v' AS a, 1 AS n), 'body')",
        "SELECT xmlelement(NAME foo)",
        "SELECT xmlforest(a, b AS y)",
        "SELECT xmlconcat(a, b, c)",
        "SELECT xmlparse(DOCUMENT x STRIP WHITESPACE)",
        "SELECT xmlparse(CONTENT x)",
        "SELECT xmlpi(NAME php, 'echo')",
        "SELECT xmlroot(x, VERSION '1.0', STANDALONE YES)",
        "SELECT xmlroot(x, VERSION no value)",
        "SELECT xmlserialize(DOCUMENT x AS text INDENT)",
        "SELECT xmlserialize(CONTENT x AS text NO INDENT)",
        "SELECT xmlexists('//a' PASSING BY REF doc BY REF)",
        "SELECT xmlexists('//a' PASSING doc)",
        "SELECT x IS DOCUMENT",
        "SELECT x IS NOT DOCUMENT",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|e| panic!("{sql:?}: {e}"));
        // The render must re-parse to the same string (stable round-trip).
        let reparsed = parse_with(&rendered, Postgres)
            .unwrap_or_else(|e| panic!("re-parse of {rendered:?}: {e:?}"));
        let rerendered = Renderer::new(Postgres)
            .render_parsed(&reparsed)
            .unwrap_or_else(|e| panic!("{rendered:?}: {e}"));
        assert_eq!(rendered, rerendered, "stable round-trip for {sql:?}");
    }
}

// --- standard string special forms (planner-parity-expr-substring/position/overlay/trim)

#[test]
fn string_special_forms_map_to_string_func_variants() {
    use crate::ast::{StringFunc, TrimSide};
    // SUBSTRING: the reversed `FOR … FROM …` order folds onto the same fields.
    for sql in [
        "SELECT SUBSTRING('abcdef' FROM 2 FOR 3)",
        "SELECT SUBSTRING('abcdef' FOR 3 FROM 2)",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
            panic!("{sql:?}: expected Expr::StringFunc");
        };
        let StringFunc::Substring { start, count, .. } = string_func.as_ref() else {
            panic!("{sql:?}: expected StringFunc::Substring");
        };
        assert!(start.is_some() && count.is_some(), "{sql:?}: both operands");
    }

    let parsed = parse_with("SELECT POSITION('b' IN 'abc')", Postgres).expect("POSITION parses");
    assert!(matches!(
        project_expr(&parsed),
        Expr::StringFunc { string_func, .. } if matches!(**string_func, StringFunc::Position { .. }),
    ));

    let parsed = parse_with("SELECT OVERLAY('abc' PLACING 'X' FROM 2 FOR 1)", Postgres)
        .expect("OVERLAY parses");
    let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::StringFunc");
    };
    let StringFunc::Overlay { count, .. } = string_func.as_ref() else {
        panic!("expected StringFunc::Overlay");
    };
    assert!(count.is_some());

    // TRIM: the `from` bit distinguishes `TRIM(TRAILING ' foo ')` from
    // `TRIM(TRAILING FROM ' foo ')` (both valid PostgreSQL, different meanings).
    let parsed = parse_with("SELECT TRIM(TRAILING ' foo ')", Postgres).expect("TRIM parses");
    let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::StringFunc");
    };
    let StringFunc::Trim {
        side,
        trim_chars,
        from,
        sources,
        ..
    } = string_func.as_ref()
    else {
        panic!("expected StringFunc::Trim");
    };
    assert_eq!(*side, Some(TrimSide::Trailing));
    assert!(trim_chars.is_none());
    assert!(!from);
    assert_eq!(sources.len(), 1);

    let parsed = parse_with("SELECT TRIM(BOTH 'x' FROM 'y', 'z')", Postgres).expect("TRIM parses");
    let Expr::StringFunc { string_func, .. } = project_expr(&parsed) else {
        panic!("expected Expr::StringFunc");
    };
    let StringFunc::Trim {
        side,
        trim_chars,
        from,
        sources,
        ..
    } = string_func.as_ref()
    else {
        panic!("expected StringFunc::Trim");
    };
    assert_eq!(*side, Some(TrimSide::Both));
    assert!(trim_chars.is_some());
    assert!(from);
    assert_eq!(sources.len(), 2, "PostgreSQL's trim_list is an expr_list");
}

#[test]
fn string_special_plain_calls_stay_ordinary_calls() {
    // The comma plain-call spellings keep their ordinary `Expr::Function` reading —
    // the special forms must not capture them (every probed engine accepts these).
    for sql in [
        "SELECT SUBSTRING('abcdef', 2, 3)",
        "SELECT SUBSTR('abcdef', 2, 3)",
        "SELECT TRIM('abc')",
        "SELECT TRIM('a', 'b')",
        "SELECT OVERLAY('abc', 'X', 2, 1)",
        "SELECT SUBSTRING()",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        assert!(
            matches!(project_expr(&parsed), Expr::Function { .. }),
            "{sql:?}: the plain call stays an ordinary Expr::Function",
        );
    }
    // SQLite has no keyword form at all, so even `position(a, b)` is an ordinary call
    // there (it fails only at binding, not parse).
    let parsed = parse_with("SELECT position('b', 'abc')", Sqlite).expect("sqlite plain call");
    assert!(matches!(project_expr(&parsed), Expr::Function { .. }));
}

#[test]
fn string_special_over_accept_boundary_rejects() {
    // Each rejects at PostgreSQL's raw parse (engine-verified against pg_query), so
    // accepting any of these would be an over-acceptance.
    for sql in [
        "SELECT TRIM()",                              // trim_list is non-empty
        "SELECT TRIM(LEADING TRAILING 'x' FROM 'y')", // one side word only
        "SELECT TRIM(LEADING 'x' 'y' FROM 'z')",      // no same-line adjacent concat
        "SELECT SUBSTRING('a' FROM 2 FOR 3 FOR 4)",   // one FOR clause
        "SELECT SUBSTRING('a' FROM 2 FOR 3 FROM 4)",  // one FROM clause
        "SELECT SUBSTRING('a' FROM 2, 3)",            // no comma after the keyword tail
        "SELECT SUBSTRING('a' SIMILAR 'p')",          // ESCAPE is mandatory
        "SELECT OVERLAY('a' PLACING 'b')",            // FROM is mandatory
        "SELECT OVERLAY('a' PLACING 'b' FOR 1)",      // FROM is mandatory
        "SELECT POSITION('b')",                       // IN is mandatory
        "SELECT POSITION()",                          // position_list is not empty
        "SELECT POSITION('b', 'abc')",                // no comma plain-call fallback
        "SELECT POSITION(1 IN 2 OR 3)",               // operands are b_expr
        "SELECT POSITION('b' NOT IN 'abc')",          // IN only, no NOT form
    ] {
        assert!(
            parse_with(sql, Postgres).is_err(),
            "{sql:?}: PostgreSQL rejects this at raw parse, so we must too",
        );
    }
}

#[test]
fn string_special_gated_off_leaves_heads_ordinary() {
    // SQLite has none of the keyword forms: the heads stay ordinary call names, so the
    // keyword tail inside the parens is a clean parse error.
    for sql in [
        "SELECT SUBSTRING('abcdef' FROM 2)",
        "SELECT POSITION('b' IN 'abc')",
        "SELECT OVERLAY('abc' PLACING 'X' FROM 2)",
        "SELECT TRIM(BOTH 'x' FROM 'y')",
        "SELECT TRIM(FROM 'y')",
    ] {
        parse_with(sql, Sqlite).expect_err(&format!("{sql:?}: SQLite has no keyword forms"));
    }
    // ANSI takes the standard shapes only: FROM-first substring, single-source trim,
    // no SIMILAR, no plain overlay call (the standard defines none).
    for sql in [
        "SELECT SUBSTRING('abcdef' FOR 3)",
        "SELECT SUBSTRING('abcdef' SIMILAR 'a' ESCAPE '#')",
        "SELECT TRIM(TRAILING ' foo ')",
        "SELECT TRIM(FROM 'y')",
        "SELECT TRIM('a', 'b')",
        "SELECT OVERLAY('abc', 'X', 2, 1)",
    ] {
        parse_with(sql, TestDialect)
            .expect_err(&format!("{sql:?}: ANSI takes the standard shapes only"));
    }
    parse_with("SELECT SUBSTRING('abcdef' FROM 2 FOR 3)", TestDialect)
        .expect("ANSI accepts the standard FROM/FOR form");
}

#[test]
fn string_special_mysql_flavor_matches_engine() {
    // Engine-measured on mysql:8.4 (see the probe matrix in the CallSyntax docs).
    for sql in [
        "SELECT SUBSTRING('abcdef' FROM 2 FOR 3)",
        "SELECT SUBSTR('abcdef' FROM 2 FOR 3)", // SUBSTR is a full keyword synonym
        "SELECT SUBSTRING('abcdef', 2, 3)",
        "SELECT POSITION('b' IN 'abc')",
        "SELECT POSITION(1 IN 2 OR 3)", // the haystack is a full expression
        "SELECT TRIM(LEADING 'x' FROM 'y')",
        "SELECT TRIM('x' FROM 'y')",
        "SELECT TRIM ('abc')", // a spaced head demotes to the generic call
        "SELECT SUBSTRING ('abcdef', 2)", // ditto — any arity parses when spaced
    ] {
        parse_with(sql, MySql).unwrap_or_else(|e| panic!("{sql:?}: mysql accepts: {e:?}"));
    }
    for sql in [
        "SELECT SUBSTRING('abcdef' FOR 3)", // FROM-first only
        "SELECT SUBSTRING('abcdef' FOR 3 FROM 2)",
        "SELECT SUBSTRING('abcdef' SIMILAR 'a' ESCAPE '#')",
        "SELECT SUBSTRING('abcdef')", // the 2-3 plain-call arity floor
        "SELECT SUBSTRING('a', 2, 3, 4)",
        "SELECT SUBSTR('abcdef')",           // the floor covers the synonym
        "SELECT POSITION('a' = 'b' IN 'c')", // the needle is a bit_expr
        "SELECT POSITION('b', 'abc')",
        "SELECT OVERLAY('abc' PLACING 'X' FROM 2)", // MySQL has no OVERLAY grammar
        "SELECT TRIM(FROM 'y')",                    // side/chars required before FROM
        "SELECT TRIM(TRAILING ' foo ')",            // side requires FROM
        "SELECT TRIM('a', 'b')",                    // no comma trim
        "SELECT TRIM('a' FROM 'b', 'c')",           // one source only
        "SELECT TRIM (LEADING 'x' FROM 'y')",       // spaced head demotes; LEADING rejects
        "SELECT SUBSTRING ('abcdef' FROM 2)",       // ditto for the keyword tail
    ] {
        parse_with(sql, MySql).expect_err(&format!("{sql:?}: mysql:8.4 parse-rejects (1064)"));
    }
    // The engine-observed composition with adjacent literal concatenation: MySQL folds
    // `'x' 'y'` into one literal, so the keyword form accepts what PostgreSQL rejects.
    parse_with("SELECT TRIM(LEADING 'x' 'y' FROM 'z')", MySql)
        .expect("adjacent literals concatenate inside the MySQL trim chars");
}

#[test]
fn string_special_duckdb_flavor_matches_engine() {
    // Engine-probed on DuckDB 1.5.4: the PG-fork grammar minus SIMILAR, and OVERLAY
    // kept only the PLACING production (no plain-call fallback).
    for sql in [
        "SELECT SUBSTRING('abcdef' FOR 3 FROM 2)",
        "SELECT OVERLAY('abc' PLACING 'X' FROM 2 FOR 1)",
        "SELECT TRIM(BOTH FROM 'a', 'b')",
        "SELECT TRIM('a', 'b')",
        "SELECT POSITION('a' || 'b' IN 'abc')",
    ] {
        parse_with(sql, DuckDb).unwrap_or_else(|e| panic!("{sql:?}: duckdb accepts: {e:?}"));
    }
    for sql in [
        "SELECT SUBSTRING('abcdef' SIMILAR 'a' ESCAPE '#')",
        "SELECT OVERLAY('abc', 'X', 2, 1)",
        "SELECT OVERLAY('abc')",
        "SELECT OVERLAY()",
    ] {
        parse_with(sql, DuckDb).expect_err(&format!("{sql:?}: duckdb parse-rejects"));
    }
}

#[test]
fn string_special_forms_round_trip() {
    for sql in [
        "SELECT SUBSTRING('abcdef' FROM 2 FOR 3)",
        "SELECT SUBSTRING('abcdef' FROM 2)",
        "SELECT SUBSTRING('abcdef' FOR 3)",
        "SELECT SUBSTRING('abcdef' FOR 3 FROM 2)", // renders canonically FROM-first
        "SELECT SUBSTRING('abcdef' SIMILAR 'a' ESCAPE '#')",
        "SELECT POSITION('b' IN 'abc')",
        "SELECT POSITION('a' || 'b' IN 'abc')",
        "SELECT OVERLAY('abc' PLACING 'X' FROM 2 FOR 1)",
        "SELECT OVERLAY('abc' PLACING 'X' FROM 2)",
        "SELECT TRIM(BOTH 'x' FROM 'xxabc')",
        "SELECT TRIM(LEADING FROM 'xxabc')",
        "SELECT TRIM(TRAILING ' foo ')",
        "SELECT TRIM('x' FROM 'xxabc')",
        "SELECT TRIM(FROM 'xxabc')",
        "SELECT TRIM(BOTH 'x')",
        "SELECT TRIM(LEADING 'x', 'y')",
        "SELECT TRIM('a' FROM 'b', 'c')",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .unwrap_or_else(|e| panic!("{sql:?}: {e}"));
        // The render must re-parse to the same string (stable round-trip).
        let reparsed = parse_with(&rendered, Postgres)
            .unwrap_or_else(|e| panic!("re-parse of {rendered:?}: {e:?}"));
        let rerendered = Renderer::new(Postgres)
            .render_parsed(&reparsed)
            .unwrap_or_else(|e| panic!("{rendered:?}: {e}"));
        assert_eq!(rendered, rerendered, "stable round-trip for {sql:?}");
    }
    // The MySQL SUBSTR keyword form renders canonically as SUBSTRING (MySQL is not a
    // render target, so the neutral Lenient spelling stands in) and the canonical
    // form re-parses under MySQL itself.
    let parsed = parse_with("SELECT SUBSTR('abcdef' FROM 2 FOR 3)", MySql).expect("parses");
    let rendered = Renderer::new(crate::dialect::Lenient)
        .render_parsed(&parsed)
        .expect("renders");
    assert_eq!(rendered, "SELECT SUBSTRING('abcdef' FROM 2 FOR 3)");
    parse_with(&rendered, MySql).expect("the canonical render re-parses");
}

/// ANSI plus the PostgreSQL predicate + operator surface, isolating the postfix null
/// tests, `BETWEEN SYMMETRIC`, and `IS NORMALIZED` from the rest of the preset.
const PG_PREDICATE_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet = FeatureSet::ANSI.with(
        FeatureDelta::EMPTY
            .predicate_syntax(PredicateSyntax::POSTGRES)
            .operator_syntax(OperatorSyntax::POSTGRES),
    );
    FeatureDialect {
        features: &FEATURES,
    }
};

#[test]
fn between_symmetric_is_recorded_and_round_trips() {
    let parsed =
        parse_with("SELECT a BETWEEN SYMMETRIC 1 AND 2", PG_PREDICATE_DIALECT).expect("parses");
    let Expr::Between { symmetric, .. } = project_expr(&parsed) else {
        panic!("expected a BETWEEN, got {:?}", project_expr(&parsed));
    };
    assert!(*symmetric, "SYMMETRIC is recorded");

    let rendered = Renderer::new(PG_PREDICATE_DIALECT)
        .render_parsed(&parsed)
        .expect("renders");
    assert_eq!(rendered, "SELECT a BETWEEN SYMMETRIC 1 AND 2");

    // The default `ASYMMETRIC` is a noise word: recorded as non-symmetric and dropped on
    // render, leaving the bare form.
    let asym = parse_with("SELECT a BETWEEN ASYMMETRIC 1 AND 2", PG_PREDICATE_DIALECT)
        .expect("ASYMMETRIC parses");
    let Expr::Between { symmetric, .. } = project_expr(&asym) else {
        panic!("expected a BETWEEN");
    };
    assert!(!*symmetric, "ASYMMETRIC is the non-symmetric default");
    let rendered = Renderer::new(PG_PREDICATE_DIALECT)
        .render_parsed(&asym)
        .expect("renders");
    assert_eq!(rendered, "SELECT a BETWEEN 1 AND 2");
}

#[test]
fn between_symmetric_is_rejected_where_the_gate_is_off() {
    // ANSI/MySQL (and every preset reusing `PredicateSyntax::ANSI`) leaves the modifier
    // unconsumed, so it surfaces as a clean parse error.
    assert!(parse_with("SELECT a BETWEEN SYMMETRIC 1 AND 2", TestDialect).is_err());
    assert!(parse_with("SELECT a BETWEEN SYMMETRIC 1 AND 2", MySql).is_err());
    // The bare BETWEEN is unaffected by the gate.
    assert!(parse_with("SELECT a BETWEEN 1 AND 2", TestDialect).is_ok());
}

#[test]
fn postfix_isnull_notnull_fold_onto_is_null_with_a_spelling() {
    use crate::ast::NullTestSpelling;
    for (sql, negated) in [("SELECT a ISNULL", false), ("SELECT a NOTNULL", true)] {
        let parsed =
            parse_with(sql, PG_PREDICATE_DIALECT).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
        let Expr::IsNull {
            negated: got,
            spelling,
            ..
        } = project_expr(&parsed)
        else {
            panic!("{sql}: expected IsNull, got {:?}", project_expr(&parsed));
        };
        assert_eq!(*got, negated, "negated flag for {sql}");
        assert_eq!(
            *spelling,
            NullTestSpelling::Postfix,
            "postfix spelling for {sql}"
        );
        let rendered = Renderer::new(PG_PREDICATE_DIALECT)
            .render_parsed(&parsed)
            .expect("renders");
        assert_eq!(rendered, sql, "postfix spelling round-trips for {sql}");
    }

    // The standard `IS NULL` keeps the `Is` spelling and its own text.
    let is_null = parse_with("SELECT a IS NOT NULL", PG_PREDICATE_DIALECT).expect("parses");
    let Expr::IsNull { spelling, .. } = project_expr(&is_null) else {
        panic!("expected IsNull");
    };
    assert_eq!(*spelling, NullTestSpelling::Is);
}

#[test]
fn postfix_isnull_notnull_are_rejected_where_the_gate_is_off() {
    // ANSI/MySQL have no postfix synonym, so the trailing keyword is a parse error; SQLite
    // (which sets the flag) accepts it.
    assert!(parse_with("SELECT a FROM t WHERE c ISNULL", TestDialect).is_err());
    assert!(parse_with("SELECT a FROM t WHERE c NOTNULL", MySql).is_err());
    assert!(parse_with("SELECT a FROM t WHERE c ISNULL", Sqlite).is_ok());
}

// ANSI plus SQLite's predicate surface, isolating the two-word `NOT NULL` postfix gate for
// the round-trip assertion (a `FeatureDialect` implements `RenderDialect`, the bare parser
// `Sqlite` does not).
const SQLITE_PREDICATE_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.predicate_syntax(PredicateSyntax::SQLITE));
    FeatureDialect {
        features: &FEATURES,
    }
};

#[test]
fn two_word_not_null_postfix_folds_onto_is_null_and_round_trips() {
    use crate::ast::NullTestSpelling;
    // SQLite's two-word `<expr> NOT NULL` postfix (a synonym for `IS NOT NULL`) folds onto
    // `Expr::IsNull` with the distinct `PostfixNotNull` spelling and round-trips verbatim —
    // not collapsed onto the one-word `NOTNULL` (`Postfix`).
    let parsed = parse_with("SELECT a NOT NULL", SQLITE_PREDICATE_DIALECT).expect("parses");
    let Expr::IsNull {
        negated, spelling, ..
    } = project_expr(&parsed)
    else {
        panic!("expected IsNull, got {:?}", project_expr(&parsed));
    };
    assert!(*negated, "two-word NOT NULL is the negated null test");
    assert_eq!(*spelling, NullTestSpelling::PostfixNotNull);
    let rendered = Renderer::new(SQLITE_PREDICATE_DIALECT)
        .render_parsed(&parsed)
        .expect("renders");
    assert_eq!(
        rendered, "SELECT a NOT NULL",
        "two-word spelling round-trips"
    );
}

#[test]
fn two_word_not_null_postfix_round_trips_exact_under_duckdb() {
    use crate::ast::NullTestSpelling;
    // DuckDB admits the same two-word `<expr> NOT NULL` postfix (engine-measured on 1.5.4);
    // the DuckDb preset's `null_test_two_word_postfix` gate folds it onto `Expr::IsNull` with
    // the `PostfixNotNull` spelling and renders it back byte-identically (Exact).
    let parsed = parse_with("SELECT a NOT NULL", DUCKDB_TYPE_DIALECT).expect("parses");
    let Expr::IsNull {
        negated, spelling, ..
    } = project_expr(&parsed)
    else {
        panic!("expected IsNull, got {:?}", project_expr(&parsed));
    };
    assert!(*negated, "two-word NOT NULL is the negated null test");
    assert_eq!(*spelling, NullTestSpelling::PostfixNotNull);
    let rendered = Renderer::new(DUCKDB_TYPE_DIALECT)
        .render_parsed(&parsed)
        .expect("renders");
    assert_eq!(
        rendered, "SELECT a NOT NULL",
        "two-word spelling round-trips exact under DuckDB"
    );
}

#[test]
fn two_word_not_null_postfix_is_rejected_where_the_gate_is_off() {
    // PostgreSQL accepts the one-word `NOTNULL` (engine-measured) but rejects the two-word
    // `NOT NULL` postfix, so the surfaces do not co-travel: under Postgres the two-word form
    // must not parse, while the one-word form still does. ANSI has neither.
    assert!(parse_with("SELECT a NOT NULL", Postgres).is_err());
    assert!(parse_with("SELECT a NOTNULL", Postgres).is_ok());
    assert!(parse_with("SELECT a NOT NULL", Ansi).is_err());
}

#[test]
fn two_word_not_null_does_not_disturb_the_not_led_predicate_family() {
    // The two-word `NOT NULL` trigger is a bounded `NOT`-then-`NULL` lookahead; the rest of
    // the `NOT`-led family (and the prefix `NOT`) must be untouched under the same SQLite gate.
    for sql in [
        "SELECT a NOT IN (1, 2)",
        "SELECT a NOT LIKE b",
        "SELECT a NOT BETWEEN 1 AND 2",
        "SELECT a NOT GLOB b",
        "SELECT NOT a",
        "SELECT NOT a NOT NULL",
    ] {
        parse_with(sql, Sqlite).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
    }
    // `NOT a` prefix and the postfix `NOT NULL` compose: `NOT (a NOT NULL)`.
    let parsed = parse_with("SELECT NOT a NOT NULL", Sqlite).expect("parses");
    let Expr::UnaryOp { expr, .. } = project_expr(&parsed) else {
        panic!("expected a prefix NOT, got {:?}", project_expr(&parsed));
    };
    assert!(
        matches!(**expr, Expr::IsNull { negated: true, .. }),
        "prefix NOT wraps the postfix NOT NULL"
    );
}

#[test]
fn is_normalized_records_the_form_and_round_trips() {
    use crate::ast::NormalizationForm;
    for (sql, want_form, want_neg) in [
        ("SELECT a IS NORMALIZED", None, false),
        (
            "SELECT a IS NFC NORMALIZED",
            Some(NormalizationForm::Nfc),
            false,
        ),
        (
            "SELECT a IS NOT NFKD NORMALIZED",
            Some(NormalizationForm::Nfkd),
            true,
        ),
    ] {
        let parsed =
            parse_with(sql, PG_PREDICATE_DIALECT).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
        let Expr::IsNormalized { form, negated, .. } = project_expr(&parsed) else {
            panic!(
                "{sql}: expected IsNormalized, got {:?}",
                project_expr(&parsed)
            );
        };
        assert_eq!(*form, want_form, "form for {sql}");
        assert_eq!(*negated, want_neg, "negated for {sql}");
        let rendered = Renderer::new(PG_PREDICATE_DIALECT)
            .render_parsed(&parsed)
            .expect("renders");
        assert_eq!(rendered, sql, "round-trip for {sql}");
    }
}

#[test]
fn is_normalized_is_rejected_where_the_gate_is_off() {
    assert!(parse_with("SELECT a IS NORMALIZED", TestDialect).is_err());
    assert!(parse_with("SELECT a IS NFC NORMALIZED", MySql).is_err());
    // A bare `nfc`/`normalized` word without the predicate stays an ordinary name where a
    // name is admissible — the gate never steals the unreserved word.
    assert!(parse_with("SELECT normalized FROM t", TestDialect).is_ok());
}

#[test]
fn pg_range_predicates_bind_tighter_than_comparison() {
    // PostgreSQL ranks `[NOT] BETWEEN`/`IN`/`LIKE` one tier ABOVE the comparison
    // operators (gram.y `%nonassoc BETWEEN IN_P LIKE …`), so the predicate is the RIGHT
    // operand of a preceding comparison — engine-measured on pg_query and DuckDB. Each
    // form round-trips through PostgreSQL rendering to its minimal (paren-free) spelling.
    let pg = Renderer::new(Postgres);

    // The four pg-regress corpus lines: `<expr> <cmp> <expr> BETWEEN low AND high`.
    for sql in [
        "SELECT true <> -1 BETWEEN 1 AND 1",
        "SELECT false <= -1 BETWEEN 1 AND 1",
        "SELECT false >= -1 BETWEEN 1 AND 1",
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("PG accepts {sql}: {e}"));
        let Expr::BinaryOp { op, right, .. } = project_expr(&parsed) else {
            panic!("{sql}: root is the comparison operator");
        };
        assert!(
            matches!(
                op,
                BinaryOperator::NotEq(_) | BinaryOperator::LtEq | BinaryOperator::GtEq
            ),
            "{sql}: root is a comparison",
        );
        assert!(
            matches!(**right, Expr::Between { .. }),
            "{sql}: the BETWEEN is the comparison's right operand",
        );
        let rendered = pg.render_parsed(&parsed).expect("PG renders");
        assert_eq!(rendered, sql, "{sql}: minimal paren-free round-trip");
    }

    // IN and LIKE share the tier: `a = b IN (c)` -> `a = (b IN (c))`, `a = b LIKE 'x'`
    // -> `a = (b LIKE 'x')`.
    for (sql, is_range) in [
        (
            "SELECT a = b IN (c)",
            (|e: &Expr<NoExt>| matches!(e, Expr::InList { .. })) as fn(&Expr<NoExt>) -> bool,
        ),
        (
            "SELECT a = b LIKE 'x'",
            (|e: &Expr<NoExt>| matches!(e, Expr::Like { .. })) as fn(&Expr<NoExt>) -> bool,
        ),
    ] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|e| panic!("PG accepts {sql}: {e}"));
        let Expr::BinaryOp {
            op: BinaryOperator::Eq(_),
            right,
            ..
        } = project_expr(&parsed)
        else {
            panic!("{sql}: root is `=`");
        };
        assert!(
            is_range(right),
            "{sql}: the range predicate is `=`'s right operand"
        );
        assert_eq!(
            pg.render_parsed(&parsed).expect("renders"),
            sql,
            "{sql}: round-trip"
        );
    }

    // A grouped comparison as the BETWEEN principal keeps its parentheses (it binds
    // looser than the range tier): `(a = b) BETWEEN c AND d`.
    let sql = "SELECT (a = b) BETWEEN c AND d";
    let parsed = parse_with(sql, Postgres).expect("PG accepts the grouped form");
    assert!(matches!(project_expr(&parsed), Expr::Between { .. }));
    assert_eq!(
        pg.render_parsed(&parsed).expect("renders"),
        sql,
        "grouped principal keeps parens"
    );
}

#[test]
fn range_predicate_precedence_above_comparison_is_dialect_data() {
    // The tighter rank is `BindingPowerTable::range_predicate_override`, carried by the
    // PostgreSQL preset. Under the ANSI default the field is `None` and tracks comparison,
    // so a range predicate directly after a comparison is a non-associative-chain reject —
    // proving the behaviour is driven by dialect data, not hard-coded.
    for sql in [
        "SELECT a = b BETWEEN c AND d",
        "SELECT true <> -1 BETWEEN 1 AND 1",
    ] {
        assert!(
            parse_with(sql, Postgres).is_ok(),
            "PostgreSQL accepts {sql}"
        );
        assert!(
            parse_with(sql, Ansi).is_err(),
            "ANSI (default rank) rejects {sql}"
        );
    }
}

#[test]
fn pg_range_predicate_chains_reject_like_comparisons() {
    // The range tier is non-associative, so a second same-tier predicate chained onto the
    // first is a clean reject in PostgreSQL (a bound cannot swallow it — the parser parses
    // bounds/patterns at the tier's own right power). The comparison chain rejects too.
    for sql in [
        "SELECT a BETWEEN b AND c BETWEEN d AND e",
        "SELECT a NOT BETWEEN b AND c BETWEEN d AND e",
        "SELECT a LIKE 'x' LIKE 'y'",
        "SELECT a = b = c",
    ] {
        assert!(
            parse_with(sql, Postgres).is_err(),
            "PostgreSQL rejects the chain {sql}"
        );
    }

    // A range predicate FOLLOWED by a looser comparison is fine — the comparison folds onto
    // the whole predicate: `a BETWEEN b AND c = d` -> `(a BETWEEN b AND c) = d`.
    let parsed =
        parse_with("SELECT a BETWEEN b AND c = d", Postgres).expect("comparison after BETWEEN");
    let Expr::BinaryOp {
        op: BinaryOperator::Eq(_),
        left,
        ..
    } = project_expr(&parsed)
    else {
        panic!("root is `=`");
    };
    assert!(
        matches!(**left, Expr::Between { .. }),
        "the BETWEEN is `=`'s left operand"
    );
}

#[test]
fn pg_and_duckdb_is_family_ranks_below_comparison() {
    // PostgreSQL and DuckDB rank the `IS`-family predicates one tier BELOW the comparison
    // operators (`%nonassoc IS ISNULL NOTNULL` under `%nonassoc '<' '>' '='`), so a comparison
    // is the LEFT operand of a following `IS` predicate: `a <> b IS NULL` is `(a <> b) IS NULL`
    // (engine-measured: PostgreSQL 16 `pg_get_viewdef`, DuckDB 1.5.4 `json_serialize_sql`).
    // Each form round-trips to its minimal paren-free spelling. The three DuckDB corpus gaps
    // (`is-predicate-precedence-below-comparison`) close here.
    let pg = Renderer::new(Postgres);
    let duck = Renderer::new(DUCKDB_TYPE_DIALECT);

    // Comparison then postfix `IS [NOT] NULL`: the comparison is the null test's operand.
    for sql in [
        "SELECT a <> b IS NULL",
        "SELECT a = b IS NULL",
        "SELECT a < b IS NOT NULL",
    ] {
        let pg_parsed =
            parse_with(sql, Postgres).unwrap_or_else(|e| panic!("PG accepts {sql}: {e}"));
        let duck_parsed = parse_with(sql, DUCKDB_TYPE_DIALECT)
            .unwrap_or_else(|e| panic!("DuckDB accepts {sql}: {e}"));
        assert!(
            matches!(project_expr(&pg_parsed), Expr::IsNull { .. }),
            "PG {sql}: root is the null test",
        );
        assert!(
            matches!(project_expr(&duck_parsed), Expr::IsNull { .. }),
            "DuckDB {sql}: root is the null test",
        );
        assert_eq!(
            pg.render_parsed(&pg_parsed).expect("renders"),
            sql,
            "PG {sql}: minimal paren-free round-trip",
        );
        assert_eq!(
            duck.render_parsed(&duck_parsed).expect("renders"),
            sql,
            "DuckDB {sql}: minimal paren-free round-trip",
        );
    }

    // Postfix `IS NULL` then a looser comparison: the null test is the comparison's LEFT
    // operand (`a IS NULL = b` -> `(a IS NULL) = b`), a chain both engines accept but our
    // parser rejected while the family sat at comparison rank.
    let parsed = parse_with("SELECT a IS NULL = b", Postgres).expect("PG accepts IS NULL = b");
    let Expr::BinaryOp {
        op: BinaryOperator::Eq(_),
        left,
        ..
    } = project_expr(&parsed)
    else {
        panic!("root is `=`");
    };
    assert!(
        matches!(**left, Expr::IsNull { .. }),
        "IS NULL is `=`'s left operand"
    );
    // The null test binds looser than `=`, so as its LEFT operand it renders parenthesized —
    // matching PostgreSQL's own `pg_get_viewdef` deparse `(a IS NULL) = b`, and round-tripping
    // to the same tree.
    assert_eq!(
        pg.render_parsed(&parsed).expect("renders"),
        "SELECT (a IS NULL) = b",
        "round-trip",
    );

    // Infix `IS DISTINCT FROM` sits at the same below-comparison tier: a comparison folds in
    // on EITHER side. `a = b IS DISTINCT FROM c` -> `(a = b) IS DISTINCT FROM c`;
    // `a IS DISTINCT FROM b = c` -> `a IS DISTINCT FROM (b = c)`.
    let parsed = parse_with("SELECT a = b IS DISTINCT FROM c", Postgres).expect("parses");
    let Expr::BinaryOp {
        op: BinaryOperator::IsDistinctFrom(_),
        left,
        ..
    } = project_expr(&parsed)
    else {
        panic!("root is IS DISTINCT FROM");
    };
    assert!(matches!(
        **left,
        Expr::BinaryOp {
            op: BinaryOperator::Eq(_),
            ..
        }
    ));
    assert_eq!(
        pg.render_parsed(&parsed).expect("renders"),
        "SELECT a = b IS DISTINCT FROM c"
    );

    let parsed = parse_with("SELECT a IS DISTINCT FROM b = c", Postgres).expect("parses");
    let Expr::BinaryOp {
        op: BinaryOperator::IsDistinctFrom(_),
        right,
        ..
    } = project_expr(&parsed)
    else {
        panic!("root is IS DISTINCT FROM");
    };
    assert!(matches!(
        **right,
        Expr::BinaryOp {
            op: BinaryOperator::Eq(_),
            ..
        }
    ));
    assert_eq!(
        pg.render_parsed(&parsed).expect("renders"),
        "SELECT a IS DISTINCT FROM b = c"
    );
}

#[test]
fn is_family_below_comparison_is_dialect_data() {
    // The rank is `BindingPowerTable::is_predicate_override`, carried by PostgreSQL/DuckDB/
    // Lenient. Under the ANSI default the field is `None` and tracks comparison, and comparison
    // is non-associative there, so a comparison directly followed by an `IS` predicate is a
    // non-associative-chain reject — proving the behaviour is dialect data, not hard-coded.
    for sql in ["SELECT a <> b IS NULL", "SELECT a = b IS DISTINCT FROM c"] {
        assert!(
            parse_with(sql, Postgres).is_ok(),
            "PostgreSQL accepts {sql}"
        );
        assert!(
            parse_with(sql, DUCKDB_TYPE_DIALECT).is_ok(),
            "DuckDB accepts {sql}"
        );
        assert!(
            parse_with(sql, Ansi).is_err(),
            "ANSI (default rank) rejects {sql}"
        );
    }
    // MySQL also tracks comparison (`None` override) but its comparison tier is
    // LEFT-associative, so it accepts `a <> b IS NULL` by left-folding to `(a <> b) IS NULL` —
    // the same tree the below-comparison rank produces, reached by a different route.
    assert!(
        parse_with("SELECT a <> b IS NULL", MySql).is_ok(),
        "MySQL accepts via left-associative comparison",
    );
}

/// ANSI plus the BigQuery `STRUCT(...)` value constructor alone, isolating the
/// `struct_constructor` gate from the rest of the BigQuery preset. Implements
/// `RenderDialect` for the exact-text round-trips (the stock BigQuery preset has no
/// Tier-1 render target yet).
const STRUCT_CONSTRUCTOR_DIALECT: FeatureDialect = {
    const FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
            struct_constructor: true,
            ..ExpressionSyntax::ANSI
        }));
    FeatureDialect {
        features: &FEATURES,
    }
};

/// The projection's struct constructor, or a panic naming what was there instead.
fn project_struct_constructor<'a>(
    parsed: &'a Parsed,
    sql: &str,
) -> &'a crate::ast::StructConstructorExpr<NoExt> {
    match project_expr(parsed) {
        Expr::StructConstructor { constructor, .. } => constructor,
        other => panic!("expected a struct constructor for {sql:?}, got {other:?}"),
    }
}

#[test]
fn struct_constructor_parses_typeless_forms() {
    // Positional values, no field names: `fields` empty marks the typeless form.
    let sql = "SELECT STRUCT(1, 2)";
    let parsed = parse_with(sql, STRUCT_CONSTRUCTOR_DIALECT).expect("positional STRUCT parses");
    let ctor = project_struct_constructor(&parsed, sql);
    assert!(ctor.fields.is_empty(), "typeless form carries no fields");
    assert_eq!(ctor.args.len(), 2);
    assert!(ctor.args.iter().all(|arg| arg.alias.is_none()));

    // `AS`-named values: the alias lands on each argument.
    let sql = "SELECT STRUCT(x AS a, y AS b)";
    let parsed = parse_with(sql, STRUCT_CONSTRUCTOR_DIALECT).expect("named STRUCT parses");
    let ctor = project_struct_constructor(&parsed, sql);
    assert!(ctor.fields.is_empty());
    let aliases: Vec<_> = ctor
        .args
        .iter()
        .map(|arg| {
            parsed
                .resolver()
                .resolve(arg.alias.as_ref().expect("alias").sym)
        })
        .collect();
    assert_eq!(aliases, ["a", "b"]);

    // `STRUCT()` parse-accepts: BigQuery's at-least-one-value rule is an analysis
    // reject (the parse-vs-bind split), matching sqlparser-rs's accept.
    let parsed =
        parse_with("SELECT STRUCT()", STRUCT_CONSTRUCTOR_DIALECT).expect("empty STRUCT parses");
    let ctor = project_struct_constructor(&parsed, "SELECT STRUCT()");
    assert!(ctor.fields.is_empty() && ctor.args.is_empty());
}

#[test]
fn struct_constructor_parses_the_typed_form() {
    // Named typed fields: `STRUCT<a INT64, b STRING>(1, 'x')` — the field list rides
    // the constructor, and the value list stays positional (no aliases).
    let sql = "SELECT STRUCT<a INT64, b STRING>(1, 'x')";
    let parsed = parse_with(sql, STRUCT_CONSTRUCTOR_DIALECT).expect("typed STRUCT parses");
    let ctor = project_struct_constructor(&parsed, sql);
    assert_eq!(ctor.fields.len(), 2);
    assert_eq!(ctor.args.len(), 2);
    let names: Vec<_> = ctor
        .fields
        .iter()
        .map(|field| {
            parsed
                .resolver()
                .resolve(field.name.as_ref().expect("field name").sym)
        })
        .collect();
    assert_eq!(names, ["a", "b"]);
    assert!(
        ctor.fields
            .iter()
            .all(|field| matches!(field.ty, DataType::UserDefined { .. })),
        "INT64/STRING resolve through the user-defined type path",
    );

    // An anonymous typed field: BigQuery admits `STRUCT<INT64>(5)` (no field name).
    let sql = "SELECT STRUCT<INT64>(5)";
    let parsed = parse_with(sql, STRUCT_CONSTRUCTOR_DIALECT).expect("anonymous typed field");
    let ctor = project_struct_constructor(&parsed, sql);
    assert_eq!(ctor.fields.len(), 1);
    assert!(ctor.fields[0].name.is_none());

    // A parameterized anonymous type keeps the whole `(…)` on the type, not a name:
    // `NUMERIC` abuts `(` (not a word), so the two-word `name TYPE` reading never fires.
    let sql = "SELECT STRUCT<NUMERIC(10, 2)>(x)";
    let parsed = parse_with(sql, STRUCT_CONSTRUCTOR_DIALECT).expect("parameterized typed field");
    let ctor = project_struct_constructor(&parsed, sql);
    assert!(ctor.fields[0].name.is_none());

    // `STRUCT<>` never reaches the constructor: `<>` munches as the one NotEq token,
    // so the dispatch (which requires an abutting `<` operator) declines and the input
    // reads as the comparison `struct <> (1)` — the BigQuery "empty field list" error
    // is a reservation concern this token-level boundary already keeps out of the
    // constructor. An unclosed field list rejects cleanly at the missing `>`.
    let parsed =
        parse_with("SELECT STRUCT<>(1)", STRUCT_CONSTRUCTOR_DIALECT).expect("`<>` comparison");
    assert!(matches!(
        project_expr(&parsed),
        Expr::BinaryOp {
            op: BinaryOperator::NotEq(_),
            ..
        }
    ));
    assert!(parse_with("SELECT STRUCT<a INT64(1)", STRUCT_CONSTRUCTOR_DIALECT).is_err());
}

#[test]
fn struct_constructor_round_trips() {
    for sql in [
        "SELECT STRUCT(1, 2)",
        "SELECT STRUCT(x AS a, y AS b)",
        "SELECT STRUCT<a INT64, b STRING>(1, 'x')",
        "SELECT STRUCT<INT64>(5)",
        "SELECT STRUCT()",
    ] {
        let parsed = parse_with(sql, STRUCT_CONSTRUCTOR_DIALECT)
            .unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        let rendered = Renderer::new(STRUCT_CONSTRUCTOR_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        assert_eq!(rendered, sql, "exact round-trip for {sql}");
    }
}

#[test]
fn struct_constructor_dispatch_is_bounded_to_the_paren_or_angle_lead() {
    // A bare `struct` — no `(`/`<` — stays an ordinary column name even under the gate.
    let parsed =
        parse_with("SELECT struct", STRUCT_CONSTRUCTOR_DIALECT).expect("bare struct is a column");
    assert_eq!(column_name(&parsed, project_expr(&parsed)), "struct");
}

#[test]
fn struct_call_stays_an_ordinary_function_without_the_gate() {
    // The non-interference boundary: in a dialect without the BigQuery form
    // (PostgreSQL keeps `struct(...)` an ordinary catalog-function call), the gate-off
    // parse is byte-for-byte the plain call path — never a struct constructor.
    for sql in ["SELECT struct(1, 2)", "SELECT STRUCT(x, y)"] {
        let parsed = parse_with(sql, Postgres).unwrap_or_else(|err| panic!("{sql}: {err:?}"));
        assert!(
            matches!(project_expr(&parsed), Expr::Function { .. }),
            "{sql} must stay an ordinary Expr::Function under PostgreSQL",
        );
    }
    // ANSI too: the baseline keeps the call reading.
    let parsed = parse_with("SELECT struct(1)", TestDialect).expect("ANSI call parses");
    assert!(matches!(project_expr(&parsed), Expr::Function { .. }));

    // The typed form has no gate-off reading: `STRUCT<a INT64>(1)` under PostgreSQL is
    // the comparison `struct < a`, whose right side `a INT64` is the usual parse error.
    assert!(parse_with("SELECT STRUCT<a INT64>(1)", Postgres).is_err());
    // And `struct < x` stays an ordinary comparison when the gate is off.
    let parsed = parse_with("SELECT struct < x", Postgres).expect("comparison parses");
    assert!(matches!(
        project_expr(&parsed),
        Expr::BinaryOp {
            op: BinaryOperator::Lt,
            ..
        }
    ));
}

#[test]
fn struct_constructor_is_on_for_the_bigquery_preset() {
    use crate::dialect::BigQuery;
    let sql = "SELECT STRUCT(1 AS a)";
    let parsed = parse_with(sql, BigQuery).expect("the BigQuery preset admits STRUCT(...)");
    let ctor = project_struct_constructor(&parsed, sql);
    assert_eq!(ctor.args.len(), 1);
    let typed = "SELECT STRUCT<a INT64>(1)";
    let parsed = parse_with(typed, BigQuery).expect("the BigQuery preset admits STRUCT<...>()");
    assert_eq!(project_struct_constructor(&parsed, typed).fields.len(), 1);
}

#[test]
fn list_comprehension_single_and_multi_var_parse_under_duckdb() {
    use crate::ast::{ArrayExpr, Expr};
    use crate::dialect::{Ansi, DuckDb};
    use crate::render::Renderer;

    // Single-var form (already supported surface).
    let sql = "SELECT [x * 2 FOR x IN [1, 2, 3]]";
    let parsed = parse_with(sql, DuckDb).expect("single-var comprehension");
    // Multi-var value+index — the tranche-2 gap (engine-accept on 1.5.4).
    let multi = "SELECT [x + i FOR x, i IN [10, 9, 8]]";
    let parsed_multi = parse_with(multi, DuckDb).expect("multi-var comprehension");
    // Extract comprehension from SELECT projection
    fn first_array(parsed: &crate::parser::Parsed) -> &ArrayExpr {
        use crate::ast::{SelectItem, SetExpr, Statement};
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("query");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("select");
        };
        let SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!("proj {:?}", select.projection[0]);
        };
        let Expr::Array { array, .. } = expr else {
            panic!("array {:?}", expr);
        };
        array.as_ref()
    }
    match first_array(&parsed) {
        ArrayExpr::Comprehension { comprehension, .. } => {
            assert_eq!(comprehension.vars.len(), 1);
        }
        other => panic!("expected comprehension, got {other:?}"),
    }
    match first_array(&parsed_multi) {
        ArrayExpr::Comprehension { comprehension, .. } => {
            assert_eq!(comprehension.vars.len(), 2, "for x, i must yield two vars");
        }
        other => panic!("expected multi-var comprehension, got {other:?}"),
    }
    // Round-trip render under DuckDB features
    let rendered = Renderer::new(crate::parser::FeatureDialect {
        features: &crate::ast::dialect::FeatureSet::DUCKDB,
    })
    .render_parsed(&parsed_multi)
    .expect("render");
    // Keywords fold to lower-case on render; structural reparse is the fidelity check.
    parse_with(&rendered, DuckDb).expect("reparse rendered multi-var comprehension");
    assert!(
        rendered.eq_ignore_ascii_case(multi),
        "rendered {rendered:?} should match {multi:?} ignoring keyword case"
    );
    // Gate: ANSI has no collection_literals → reject
    parse_with(multi, Ansi).expect_err("ANSI rejects list comprehensions");
    // Filter form still works
    parse_with("SELECT [x FOR x IN [1, 2, 3] IF x > 1]", DuckDb).expect("filter form");
    // Nested / multi-var with filter from corpus
    parse_with(
        "WITH base AS (SELECT [4, 5, 6] AS l) SELECT [x FOR x, i IN l IF i != 2] AS filtered FROM base",
        DuckDb,
    )
    .expect("corpus multi-var with filter");
}
