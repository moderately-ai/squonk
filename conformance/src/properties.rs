// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Proptest strategies and structural oracles over generated ASTs.
//!
//! The generators deliberately produce a legal subset instead of arbitrary enum
//! soup, but that subset tracks the production grammar: query bodies (SELECT, set
//! ops, VALUES, CTEs), the literal/expression forms (including function calls and
//! `OVER` window clauses), joins, and the DDL/DML statement families (CREATE
//! TABLE, INSERT, UPDATE, DELETE). Generated trees are rendered, reparsed, remapped
//! through a shared test interner, and then compared with ordinary structural
//! equality. Resolver-aware normalization is retained as failure context.
//!
//! Trees are kept *legal* — they survive render -> parse -> structural-equal — so
//! the round-trip property is a real oracle rather than something mostly filtered
//! by assumptions. Where a shape cannot round-trip through the synthetic-span
//! renderer (e.g. a `FLOAT` literal, whose synthesized spelling `0` reparses as an
//! integer), it is documented at the generator that omits it rather than silently
//! dropped.

pub mod dialect_features;
mod generators;
mod normalize;
mod normalized;

pub use dialect_features::{
    BIGQUERY_FEATURE_PROBES, BIGQUERY_FEATURE_SEEDS, CLICKHOUSE_FEATURE_PROBES,
    CLICKHOUSE_FEATURE_SEEDS, DUCKDB_FEATURE_PROBES, DUCKDB_FEATURE_SEEDS,
    FEATURE_SCHEMA_SETUP_SQL, FeatureProbe, MYSQL_FEATURE_PROBES, MYSQL_FEATURE_SEEDS,
    POSTGRES_FEATURE_PROBES, POSTGRES_FEATURE_SEEDS, SQLITE_FEATURE_PROBES,
    SQLITE_MISFEATURE_SEEDS, applicable_probes, arb_feature_statement,
};
pub use generators::{GENERATED_RESOLVER, GeneratedResolver, arb_statement, render_generated};
pub use normalize::normalize_statement;
pub use normalized::{
    NormalizedCreateTable, NormalizedDelete, NormalizedInsert, NormalizedQuery,
    NormalizedStatement, NormalizedUpdate,
};

pub(crate) use generators::{
    binary, frame_current_row, frame_following, frame_preceding, frame_unbounded_following,
    frame_unbounded_preceding, function_call, function_expr, ident, literal_expr, meta,
    object_name, on_commit_option, query_of, set_select, set_values, table_factor, unary,
    values_item_expr,
};
// The remaining `pub(crate)` builder toolkit is exercised only by the test module below,
// so these re-exports are gated to keep the plain-lib build free of unused-import findings.
#[cfg(test)]
pub(crate) use generators::{
    ALIAS_SYM, TABLE_SYM, column_expr_from_name, integer_literal, named_window,
    statement_create_table, statement_delete, statement_insert, statement_query, statement_update,
    window_definition, window_frame,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_interner;
    use proptest::prelude::*;
    use squonk::dialect::Ansi;
    use squonk::parse_with;
    use squonk_ast::render::RenderMode;
    use squonk_ast::{
        AliasSpelling, BinaryOperator, ColumnConstraint, ColumnDef, ColumnOption, CreateTable,
        CreateTableBody, DataType, Delete, DmlSelection, DmlTarget, EqualsSpelling, Expr,
        IdentityColumn, IdentityGeneration, IdentityOption, IndexColumn, Insert, InsertSource,
        InsertTarget, InsertValue, InsertValues, InsertVerb, IntervalFields, Join, JoinOperator,
        LiteralKind, NamedWindow, NoExt, ObjectName, OrderByExpr, Query, RelationInheritance,
        Select, SelectItem, SelectSpelling, Statement, TableConstraint, TableElement,
        TableWithJoins, TimeZone, Update, UpdateAssignment, UpdateValue, WindowFrameExclusion,
        WindowFrameUnits, WindowSpec,
    };
    use thin_vec::{ThinVec, thin_vec};

    const CORPUS: &[&str] = &[
        "SELECT 1",
        "SELECT a, b, *",
        "SELECT a AS x FROM t",
        "SELECT a + b * c FROM t WHERE a = b",
        "SELECT * FROM t LEFT JOIN t AS x ON a = b",
        "SELECT a FROM t GROUP BY a HAVING a > 0",
        "SELECT a FROM t UNION SELECT b FROM t EXCEPT ALL SELECT c FROM t",
        "SELECT a FROM t ORDER BY a DESC NULLS LAST LIMIT 0 OFFSET 0",
        "VALUES (1, 2), (3, 4)",
        "VALUES (1, DEFAULT), (DEFAULT, 2)",
        "WITH x(a) AS MATERIALIZED (VALUES (1)) SELECT a FROM x",
        "SELECT TRUE, FALSE, NULL",
        "SELECT CAST(a AS INT), CAST(b AS public.geometry(4326))",
        "SELECT CAST(a AS TIMESTAMP(3) WITH TIME ZONE)",
        "CREATE TABLE t (id INT PRIMARY KEY, name TEXT NOT NULL DEFAULT 'x')",
        "CREATE TABLE t (id BIGINT GENERATED ALWAYS AS IDENTITY (START WITH 10 INCREMENT BY 2 NO MINVALUE MAXVALUE 100 CACHE 5 NO CYCLE), n INT GENERATED ALWAYS AS (id + 1) STORED)",
        "CREATE TABLE t (id INT REFERENCES parent (id), CONSTRAINT pk PRIMARY KEY (id))",
        "CREATE TEMP TABLE IF NOT EXISTS t (id) ON COMMIT DROP AS SELECT 1 WITH NO DATA",
        "INSERT INTO t (id, name) VALUES (1, DEFAULT), (2, 'b')",
        "INSERT INTO t DEFAULT VALUES",
        "WITH src AS (SELECT 1) INSERT INTO t SELECT * FROM src",
        "INSERT INTO t AS target (id) OVERRIDING SYSTEM VALUE VALUES (1)",
        "UPDATE t AS target SET a = 1, b = DEFAULT FROM u WHERE target.id = u.id",
        "WITH src AS (SELECT 1) UPDATE t target SET a = 1 WHERE EXISTS (SELECT 1)",
        "DELETE FROM t AS target USING u WHERE target.id = u.id",
        "WITH src AS (SELECT 1) DELETE FROM t target WHERE EXISTS (SELECT 1)",
    ];

    proptest! {
        #[test]
        fn generated_ast_render_never_panics(statement in arb_statement()) {
            let canonical = render_generated(&statement, RenderMode::Canonical);
            let parenthesized = render_generated(&statement, RenderMode::Parenthesized);
            let redacted = render_generated(&statement, RenderMode::Redacted);

            prop_assert!(!canonical.is_empty());
            prop_assert!(!parenthesized.is_empty());
            prop_assert!(!redacted.is_empty());
        }

        #[test]
        fn generated_ast_round_trips_structurally(statement in arb_statement()) {
            let rendered = render_generated(&statement, RenderMode::Parenthesized);
            let reparsed = parse_with(&rendered, squonk::ParseConfig::new(Ansi))
                .map_err(|err| TestCaseError::fail(format!("rendered SQL did not parse: {rendered:?}: {err:?}")))?;

            let [reparsed_statement] = reparsed.statements() else {
                return Err(TestCaseError::fail(format!(
                    "rendered SQL should parse to one statement: {rendered:?}"
                )));
            };

            let comparison = shared_interner::compare_statement_with_shared_symbols(
                &statement,
                &GENERATED_RESOLVER,
                reparsed_statement,
                reparsed.resolver(),
            );
            if !comparison.structurally_equal() {
                let left_normalized = normalize_statement(&statement, &GENERATED_RESOLVER);
                let right_normalized = normalize_statement(reparsed_statement, reparsed.resolver());
                return Err(TestCaseError::fail(comparison.failure_message(
                    "generated AST round-trip structural mismatch",
                    &[("rendered SQL", &rendered)],
                    Some((&left_normalized, &right_normalized)),
                )));
            }
        }
    }

    #[test]
    fn corpus_rendering_is_idempotent() {
        for sql in CORPUS {
            for mode in [RenderMode::Canonical, RenderMode::Parenthesized] {
                let first = parse_and_render(sql, mode);
                let second = parse_and_render(&first, mode);
                assert_eq!(
                    first, second,
                    "rendering {sql:?} should be stable in {mode:?} mode",
                );
            }
        }
    }

    #[test]
    fn generated_structural_oracle_catches_dropped_projection_alias() {
        let statement = statement_query(Query {
            with: None,
            body: set_select(Select {
                distinct: None,
                straight_join: false,
                projection: thin_vec![SelectItem::Expr {
                    expr: column_expr_from_name(object_name(&[1])),
                    alias: Some(ident(ALIAS_SYM)),
                    alias_spelling: AliasSpelling::As,
                    meta: meta(),
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
                meta: meta(),
            }),
            order_by: ThinVec::new(),
            order_by_all: None,
            limit_by: None,
            limit: None,
            settings: ThinVec::new(),
            format: None,
            locking: ThinVec::new(),
            pipe_operators: ThinVec::new(),
            for_clause: None,
            meta: meta(),
        });
        let alias_dropped = statement_query(Query {
            with: None,
            body: set_select(Select {
                distinct: None,
                straight_join: false,
                projection: thin_vec![SelectItem::Expr {
                    expr: column_expr_from_name(object_name(&[1])),
                    alias: None,
                    alias_spelling: AliasSpelling::As,
                    meta: meta(),
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
                meta: meta(),
            }),
            order_by: ThinVec::new(),
            order_by_all: None,
            limit_by: None,
            limit: None,
            settings: ThinVec::new(),
            format: None,
            locking: ThinVec::new(),
            pipe_operators: ThinVec::new(),
            for_clause: None,
            meta: meta(),
        });

        let comparison = shared_interner::compare_statement_with_shared_symbols(
            &statement,
            &GENERATED_RESOLVER,
            &alias_dropped,
            &GENERATED_RESOLVER,
        );
        assert!(!comparison.structurally_equal());

        let rendered = render_generated(&statement, RenderMode::Parenthesized);
        let reparsed =
            parse_with(&rendered, squonk::ParseConfig::new(Ansi)).expect("rendered SQL parses");
        let comparison = shared_interner::compare_statement_with_shared_symbols(
            &statement,
            &GENERATED_RESOLVER,
            &reparsed.statements()[0],
            reparsed.resolver(),
        );
        assert!(
            comparison.structurally_equal(),
            "{}",
            comparison.failure_message(
                "generated alias round-trip shared-symbol comparison failed",
                &[("rendered SQL", &rendered)],
                None,
            ),
        );
    }

    #[test]
    fn generated_shared_symbol_oracle_ignores_raw_symbol_ids() {
        let statement = statement_query(Query {
            with: None,
            body: set_select(Select {
                distinct: None,
                straight_join: false,
                projection: thin_vec![SelectItem::Expr {
                    expr: column_expr_from_name(object_name(&[2])),
                    alias: None,
                    alias_spelling: AliasSpelling::As,
                    meta: meta(),
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
                meta: meta(),
            }),
            order_by: ThinVec::new(),
            order_by_all: None,
            limit_by: None,
            limit: None,
            settings: ThinVec::new(),
            format: None,
            locking: ThinVec::new(),
            pipe_operators: ThinVec::new(),
            for_clause: None,
            meta: meta(),
        });

        let rendered = render_generated(&statement, RenderMode::Canonical);
        assert_eq!(rendered, "SELECT b");
        let reparsed =
            parse_with(&rendered, squonk::ParseConfig::new(Ansi)).expect("rendered SQL parses");

        assert_ne!(
            statement,
            reparsed.statements()[0],
            "raw symbols must differ so this test proves remapping is active",
        );

        let comparison = shared_interner::compare_statement_with_shared_symbols(
            &statement,
            &GENERATED_RESOLVER,
            &reparsed.statements()[0],
            reparsed.resolver(),
        );
        assert!(
            comparison.structurally_equal(),
            "{}",
            comparison.failure_message(
                "generated shared-symbol comparison failed",
                &[("rendered SQL", &rendered)],
                None,
            ),
        );
    }

    #[test]
    fn normalizer_remains_available_as_cross_interner_diagnostic() {
        let statement = statement_query(Query {
            with: None,
            body: set_select(Select {
                distinct: None,
                straight_join: false,
                projection: thin_vec![SelectItem::Expr {
                    expr: column_expr_from_name(object_name(&[2])),
                    alias: None,
                    alias_spelling: AliasSpelling::As,
                    meta: meta(),
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
                meta: meta(),
            }),
            order_by: ThinVec::new(),
            order_by_all: None,
            limit_by: None,
            limit: None,
            settings: ThinVec::new(),
            format: None,
            locking: ThinVec::new(),
            pipe_operators: ThinVec::new(),
            for_clause: None,
            meta: meta(),
        });

        let rendered = render_generated(&statement, RenderMode::Canonical);
        let reparsed =
            parse_with(&rendered, squonk::ParseConfig::new(Ansi)).expect("rendered SQL parses");

        assert_eq!(
            normalize_statement(&statement, &GENERATED_RESOLVER),
            normalize_statement(&reparsed.statements()[0], reparsed.resolver()),
        );
    }

    /// Assert one constructed statement survives render -> parse with structural
    /// equality through the shared test interner — the same oracle the property
    /// uses, but on a fixed, replayable tree.
    fn assert_round_trips(label: &str, statement: &Statement<NoExt>) {
        let rendered = render_generated(statement, RenderMode::Parenthesized);
        let reparsed = parse_with(&rendered, squonk::ParseConfig::new(Ansi))
            .unwrap_or_else(|err| panic!("{label}: rendered {rendered:?} did not parse: {err:?}"));
        let statements = reparsed.statements();
        assert_eq!(
            statements.len(),
            1,
            "{label}: rendered {rendered:?} parsed to {} statements",
            statements.len(),
        );
        let comparison = shared_interner::compare_statement_with_shared_symbols(
            statement,
            &GENERATED_RESOLVER,
            &statements[0],
            reparsed.resolver(),
        );
        assert!(
            comparison.structurally_equal(),
            "{}",
            comparison.failure_message(
                &format!("{label}: round-trip structural mismatch"),
                &[("rendered SQL", &rendered)],
                None,
            ),
        );
    }

    fn insert_of(source: InsertSource<NoExt>) -> Statement<NoExt> {
        statement_insert(Insert {
            verb: InsertVerb::Insert,
            or_action: None,
            column_matching: None,
            with: None,
            target: InsertTarget {
                name: object_name(&[TABLE_SYM]),
                alias: None,
                alias_spelling: AliasSpelling::As,
                columns: ThinVec::new(),
                meta: meta(),
            },
            overriding: None,
            source,
            row_alias: None,
            upsert: None,
            returning: None,
            meta: meta(),
        })
    }

    /// Locks the INSERT-source canonicalization boundary the generator relies on:
    /// `INSERT ... VALUES (...)` always reparses as [`InsertSource::Values`], so a
    /// query source must render as a `SELECT` (not a bare `VALUES`) to stay an
    /// [`InsertSource::Query`]. This is why `arb_insert_overriding_and_source`
    /// restricts the query arm to `arb_select`.
    #[test]
    fn insert_values_and_query_round_trip_to_their_own_kind() {
        assert_round_trips(
            "INSERT ... VALUES",
            &insert_of(InsertSource::Values {
                values: Box::new(InsertValues {
                    rows: thin_vec![thin_vec![InsertValue::Expr {
                        expr: column_expr_from_name(object_name(&[1])),
                        meta: meta(),
                    }]],
                    meta: meta(),
                }),
                meta: meta(),
            }),
        );

        assert_round_trips(
            "INSERT ... SELECT",
            &insert_of(InsertSource::Query {
                query: Box::new(query_of(set_select(Select {
                    distinct: None,
                    straight_join: false,
                    projection: thin_vec![SelectItem::Expr {
                        expr: integer_literal(),
                        alias: None,
                        alias_spelling: AliasSpelling::As,
                        meta: meta(),
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
                    meta: meta(),
                }))),
                meta: meta(),
            }),
        );
    }

    /// A fixed example per newly-generated family, so a regression that breaks one
    /// family's legality surfaces deterministically rather than only via a rare
    /// proptest draw. The shapes mirror what the generators emit.
    #[test]
    fn new_family_anchors_round_trip() {
        // Temporal literals: the time-zone flag and interval qualifier must survive.
        for literal in [
            LiteralKind::String,
            LiteralKind::Date,
            LiteralKind::Time {
                time_zone: TimeZone::WithTimeZone,
            },
            LiteralKind::Timestamp {
                time_zone: TimeZone::WithoutTimeZone,
            },
            LiteralKind::Interval {
                fields: Some(IntervalFields::DayToSecond),
                precision: None,
            },
        ] {
            assert_round_trips("literal", &select_expr(literal_expr(literal)));
        }

        // A window function with an inline definition, frame, and exclusion.
        assert_round_trips(
            "window function",
            &select_expr(function_expr(function_call(
                object_name(&[1]),
                None,
                ThinVec::new(),
                true,
                Some(WindowSpec::Inline {
                    definition: Box::new(window_definition(
                        vec![column_expr_from_name(object_name(&[2]))],
                        vec![order_key(3)],
                        Some(window_frame(
                            WindowFrameUnits::Rows,
                            frame_unbounded_preceding(),
                            Some(frame_current_row()),
                            Some(WindowFrameExclusion::Ties),
                        )),
                    )),
                    meta: meta(),
                }),
            ))),
        );

        // A SELECT carrying a `WINDOW` clause referenced by `OVER name`.
        assert_round_trips(
            "named window clause",
            &select_with_windows(
                function_expr(function_call(
                    object_name(&[1]),
                    None,
                    thin_vec![column_expr_from_name(object_name(&[2]))],
                    false,
                    Some(WindowSpec::Named {
                        name: ident(TABLE_SYM),
                        meta: meta(),
                    }),
                )),
                vec![named_window(
                    4,
                    window_definition(Vec::new(), vec![order_key(3)], None),
                )],
            ),
        );

        // USING and NATURAL join constraints.
        assert_round_trips(
            "join using",
            &select_from_join(squonk_ast::JoinConstraint::Using {
                columns: thin_vec![ident(1)],
                alias: None,
                meta: meta(),
            }),
        );
        assert_round_trips(
            "join natural",
            &select_from_join(squonk_ast::JoinConstraint::Natural { meta: meta() }),
        );

        // CREATE TABLE with an identity column and a table constraint.
        assert_round_trips("create table", &create_table_anchor());

        // UPDATE and DELETE with a WHERE filter.
        assert_round_trips(
            "update",
            &statement_update(Update {
                with: None,
                or_action: None,
                target: DmlTarget {
                    name: object_name(&[TABLE_SYM]),
                    inheritance: RelationInheritance::Plain,
                    alias: None,
                    alias_spelling: AliasSpelling::As,
                    meta: meta(),
                },
                assignments: thin_vec![UpdateAssignment::Single {
                    target: object_name(&[1]),
                    value: UpdateValue::Expr {
                        expr: integer_literal(),
                        meta: meta(),
                    },
                    meta: meta(),
                }],
                from: ThinVec::new(),
                selection: Some(DmlSelection::Where {
                    condition: binary(
                        column_expr_from_name(object_name(&[1])),
                        BinaryOperator::Eq(EqualsSpelling::Single),
                        integer_literal(),
                    ),
                    meta: meta(),
                }),
                order_by: ThinVec::new(),
                limit: None,
                returning: None,
                meta: meta(),
            }),
        );
        assert_round_trips(
            "delete",
            &statement_delete(Delete {
                with: None,
                target: DmlTarget {
                    name: object_name(&[TABLE_SYM]),
                    inheritance: RelationInheritance::Plain,
                    alias: None,
                    alias_spelling: AliasSpelling::As,
                    meta: meta(),
                },
                using: ThinVec::new(),
                selection: Some(DmlSelection::Where {
                    condition: binary(
                        column_expr_from_name(object_name(&[1])),
                        BinaryOperator::Eq(EqualsSpelling::Single),
                        integer_literal(),
                    ),
                    meta: meta(),
                }),
                order_by: ThinVec::new(),
                limit: None,
                returning: None,
                meta: meta(),
            }),
        );
    }

    fn select_expr(expr: Expr<NoExt>) -> Statement<NoExt> {
        select_projection(thin_vec![SelectItem::Expr {
            expr,
            alias: None,
            alias_spelling: AliasSpelling::As,
            meta: meta(),
        }])
    }

    fn select_projection(projection: ThinVec<SelectItem<NoExt>>) -> Statement<NoExt> {
        statement_query(query_of(set_select(Select {
            distinct: None,
            straight_join: false,
            projection,
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
            meta: meta(),
        })))
    }

    fn select_with_windows(
        expr: Expr<NoExt>,
        windows: Vec<NamedWindow<NoExt>>,
    ) -> Statement<NoExt> {
        statement_query(query_of(set_select(Select {
            distinct: None,
            straight_join: false,
            projection: thin_vec![SelectItem::Expr {
                expr,
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: meta(),
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
            windows: windows.into_iter().collect(),
            qualify: None,
            sample: None,
            spelling: SelectSpelling::Select,
            meta: meta(),
        })))
    }

    fn select_from_join(constraint: squonk_ast::JoinConstraint<NoExt>) -> Statement<NoExt> {
        let table = TableWithJoins {
            relation: table_factor(object_name(&[TABLE_SYM]), None),
            joins: thin_vec![Join {
                relation: table_factor(object_name(&[TABLE_SYM]), Some(ident(ALIAS_SYM))),
                operator: JoinOperator::Inner {
                    straight: false,
                    inner: false,
                    constraint,
                    meta: meta(),
                },
                meta: meta(),
            }],
            meta: meta(),
        };
        statement_query(query_of(set_select(Select {
            distinct: None,
            straight_join: false,
            projection: thin_vec![SelectItem::Wildcard {
                options: None,
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: meta()
            }],
            into: None,
            from: thin_vec![table],
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
            meta: meta(),
        })))
    }

    fn order_key(sym: u32) -> OrderByExpr<NoExt> {
        OrderByExpr {
            expr: column_expr_from_name(object_name(&[sym])),
            asc: None,
            using: None,
            nulls_first: None,
            meta: meta(),
        }
    }

    fn create_table_anchor() -> Statement<NoExt> {
        let identity_column = ColumnDef {
            name: ident(1),
            data_type: Some(DataType::BigInt {
                display_width: None,
                meta: meta(),
            }),
            storage: None,
            compression: None,
            constraints: thin_vec![ColumnConstraint {
                name: None,
                conflict: None,
                option: ColumnOption::Identity {
                    identity: Box::new(IdentityColumn {
                        generation: IdentityGeneration::Always,
                        options: thin_vec![
                            IdentityOption::StartWith {
                                expr: integer_literal(),
                                meta: meta(),
                            },
                            IdentityOption::MinValue {
                                value: None,
                                meta: meta(),
                            },
                        ],
                        meta: meta(),
                    }),
                    meta: meta(),
                },
                characteristics: None,
                meta: meta(),
            }],
            meta: meta(),
        };
        let constraint = squonk_ast::TableConstraintDef {
            name: Some(ident(ALIAS_SYM)),
            constraint: TableConstraint::PrimaryKey {
                columns: thin_vec![IndexColumn {
                    expr: Expr::Column {
                        name: ObjectName(thin_vec![ident(1)]),
                        meta: meta(),
                    },
                    asc: None,
                    nulls_first: None,
                    meta: meta(),
                }],
                include: ThinVec::new(),
                meta: meta(),
            },
            no_inherit: false,
            not_valid: false,
            characteristics: None,
            meta: meta(),
        };
        statement_create_table(CreateTable {
            or_replace: false,
            temporary: None,
            unlogged: false,
            if_not_exists: false,
            name: object_name(&[TABLE_SYM]),
            body: CreateTableBody::Definition {
                elements: thin_vec![
                    TableElement::Column {
                        column: identity_column,
                        meta: meta(),
                    },
                    TableElement::Constraint {
                        constraint,
                        meta: meta(),
                    },
                ],
                meta: meta(),
            },
            inherits: ThinVec::new(),
            partition_by: None,
            access_method: None,
            options: ThinVec::new(),
            meta: meta(),
        })
    }

    fn parse_and_render(sql: &str, mode: RenderMode) -> String {
        let parsed = parse_with(sql, squonk::ParseConfig::new(Ansi))
            .unwrap_or_else(|err| panic!("expected {sql:?} to parse: {err:?}"));
        crate::render_statements(&parsed, mode)
    }
}
