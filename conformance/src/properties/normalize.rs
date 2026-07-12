// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! `normalize_statement` and the `normalize_*` conversions from the AST into the Normalized* model.

use super::normalized::*;
use squonk_ast::{
    ColumnOption, CreateTable, CreateTableBody, CreateTableOptionKind, CteBody, DataType, Delete,
    DmlSelection, DmlTarget, Expr, FunctionCall, GroupByItem, Ident, IdentityOption, IndexColumn,
    Insert, InsertSource, InsertValue, Join, JoinOperator, Limit, NamedWindow, NoExt, ObjectName,
    OrderByExpr, PivotExpr, Query, Resolver, Select, SelectDistinct, SelectItem, SetExpr,
    SetQuantifier, Statement, TableAlias, TableConstraint, TableElement, TableFactor,
    TableFunctionColumn, TableOptionValue, TableSample, TableWithJoins, Update, UpdateAssignment,
    UpdateTupleSource, UpdateValue, ValuesItem, WindowDefinition, WindowFrame, WindowFrameBound,
    WindowSpec, With,
};

/// Normalize a statement by resolving symbols to text and dropping spans/node ids.
pub fn normalize_statement(
    statement: &Statement<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedStatement {
    match statement {
        Statement::Query { query, .. } => {
            NormalizedStatement::Query(Box::new(normalize_query(query, resolver)))
        }
        Statement::CreateTable { create, .. } => {
            NormalizedStatement::CreateTable(normalize_create_table(create, resolver))
        }
        Statement::Insert { insert, .. } => {
            NormalizedStatement::Insert(normalize_insert(insert, resolver))
        }
        Statement::Update { update, .. } => {
            NormalizedStatement::Update(normalize_update(update, resolver))
        }
        Statement::Delete { delete, .. } => {
            NormalizedStatement::Delete(normalize_delete(delete, resolver))
        }
        Statement::Other { ext, .. } => match *ext {},
        _ => unreachable!("unknown stock statement variant"),
    }
}

fn normalize_insert(insert: &Insert<NoExt>, resolver: &dyn Resolver) -> NormalizedInsert {
    NormalizedInsert {
        with: insert
            .with
            .as_ref()
            .map(|with| normalize_with(with, resolver)),
        target: NormalizedInsertTarget {
            name: resolve_name(&insert.target.name, resolver),
            alias: insert
                .target
                .alias
                .as_ref()
                .map(|alias| resolve_ident(alias, resolver)),
            columns: normalize_idents(&insert.target.columns, resolver),
        },
        overriding: insert.overriding,
        source: normalize_insert_source(&insert.source, resolver),
    }
}

fn normalize_insert_source(
    source: &InsertSource<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedInsertSource {
    match source {
        InsertSource::DefaultValues { .. } => NormalizedInsertSource::DefaultValues,
        InsertSource::Values { values, .. } => NormalizedInsertSource::Values(
            values
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|item| normalize_insert_value(item, resolver))
                        .collect()
                })
                .collect(),
        ),
        InsertSource::Query { query, .. } => {
            NormalizedInsertSource::Query(Box::new(normalize_query(query, resolver)))
        }
        InsertSource::Set { assignments, .. } => NormalizedInsertSource::Set(
            assignments
                .iter()
                .map(|assignment| normalize_update_assignment(assignment, resolver))
                .collect(),
        ),
    }
}

fn normalize_insert_value(
    value: &InsertValue<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedInsertValue {
    match value {
        InsertValue::Expr { expr, .. } => {
            NormalizedInsertValue::Expr(normalize_expr(expr, resolver))
        }
        InsertValue::Default { .. } => NormalizedInsertValue::Default,
    }
}

fn normalize_values_item(
    item: &ValuesItem<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedValuesItem {
    match item {
        ValuesItem::Expr { expr, .. } => NormalizedValuesItem::Expr(normalize_expr(expr, resolver)),
        ValuesItem::Default { .. } => NormalizedValuesItem::Default,
    }
}

fn normalize_update(update: &Update<NoExt>, resolver: &dyn Resolver) -> NormalizedUpdate {
    NormalizedUpdate {
        with: update
            .with
            .as_ref()
            .map(|with| normalize_with(with, resolver)),
        target: normalize_dml_target(&update.target, resolver),
        assignments: update
            .assignments
            .iter()
            .map(|assignment| normalize_update_assignment(assignment, resolver))
            .collect(),
        from: update
            .from
            .iter()
            .map(|table| normalize_table_with_joins(table, resolver))
            .collect(),
        selection: update
            .selection
            .as_ref()
            .map(|selection| normalize_dml_selection(selection, resolver)),
    }
}

fn normalize_delete(delete: &Delete<NoExt>, resolver: &dyn Resolver) -> NormalizedDelete {
    NormalizedDelete {
        with: delete
            .with
            .as_ref()
            .map(|with| normalize_with(with, resolver)),
        target: normalize_dml_target(&delete.target, resolver),
        using: delete
            .using
            .iter()
            .map(|table| normalize_table_with_joins(table, resolver))
            .collect(),
        selection: delete
            .selection
            .as_ref()
            .map(|selection| normalize_dml_selection(selection, resolver)),
    }
}

fn normalize_dml_target(target: &DmlTarget, resolver: &dyn Resolver) -> NormalizedDmlTarget {
    NormalizedDmlTarget {
        name: resolve_name(&target.name, resolver),
        inheritance: target.inheritance.clone(),
        alias: target
            .alias
            .as_ref()
            .map(|alias| resolve_ident(alias, resolver)),
    }
}

fn normalize_dml_selection(
    selection: &DmlSelection<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedDmlSelection {
    match selection {
        DmlSelection::Where { condition, .. } => {
            NormalizedDmlSelection::Where(normalize_expr(condition, resolver))
        }
        DmlSelection::CurrentOf { cursor, .. } => {
            NormalizedDmlSelection::CurrentOf(resolve_ident(cursor, resolver))
        }
    }
}

fn normalize_update_assignment(
    assignment: &UpdateAssignment<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedUpdateAssignment {
    match assignment {
        UpdateAssignment::Single { target, value, .. } => NormalizedUpdateAssignment::Single {
            target: resolve_name(target, resolver),
            value: normalize_update_value(value, resolver),
        },
        UpdateAssignment::Tuple {
            targets, source, ..
        } => NormalizedUpdateAssignment::Tuple {
            targets: targets
                .iter()
                .map(|target| resolve_name(target, resolver))
                .collect(),
            source: normalize_update_tuple_source(source, resolver),
        },
    }
}

fn normalize_update_value(
    value: &UpdateValue<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedUpdateValue {
    match value {
        UpdateValue::Expr { expr, .. } => {
            NormalizedUpdateValue::Expr(normalize_expr(expr, resolver))
        }
        UpdateValue::Default { .. } => NormalizedUpdateValue::Default,
    }
}

fn normalize_update_tuple_source(
    source: &UpdateTupleSource<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedUpdateTupleSource {
    match source {
        UpdateTupleSource::Row {
            explicit, values, ..
        } => NormalizedUpdateTupleSource::Row {
            explicit: *explicit,
            values: values
                .iter()
                .map(|value| normalize_update_value(value, resolver))
                .collect(),
        },
        UpdateTupleSource::Subquery { query, .. } => {
            NormalizedUpdateTupleSource::Subquery(Box::new(normalize_query(query, resolver)))
        }
        UpdateTupleSource::Default { .. } => NormalizedUpdateTupleSource::Default,
    }
}

fn normalize_create_table(
    create: &CreateTable<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedCreateTable {
    NormalizedCreateTable {
        temporary: create.temporary,
        if_not_exists: create.if_not_exists,
        name: resolve_name(&create.name, resolver),
        body: normalize_create_table_body(&create.body, resolver),
        options: create
            .options
            .iter()
            .map(|option| NormalizedCreateTableOption {
                kind: normalize_create_table_option_kind(&option.kind, resolver),
            })
            .collect(),
    }
}

fn normalize_create_table_body(
    body: &CreateTableBody<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedCreateTableBody {
    match body {
        CreateTableBody::Definition { elements, .. } => NormalizedCreateTableBody::Definition(
            elements
                .iter()
                .map(|element| normalize_table_element(element, resolver))
                .collect(),
        ),
        CreateTableBody::AsQuery {
            columns,
            query,
            with_data,
            ..
        } => NormalizedCreateTableBody::AsQuery {
            columns: normalize_idents(columns, resolver),
            query: Box::new(normalize_query(query, resolver)),
            with_data: *with_data,
        },
        // The property generators never emit declarative partitioning (a `PartitionOf` body or a
        // `PARTITION BY` spec) nor a typed `OF <type>` body, so this normalization is only
        // reached from generator output and its reparse — neither of which produces them.
        CreateTableBody::PartitionOf { .. } => {
            unreachable!("the property generators do not produce declarative partitioning")
        }
        CreateTableBody::OfType { .. } => {
            unreachable!("the property generators do not produce typed (OF) tables")
        }
        CreateTableBody::AsExecute { .. } => {
            unreachable!("the property generators do not produce AS EXECUTE bodies")
        }
        CreateTableBody::LikeSource { .. } => {
            unreachable!("the property generators do not produce statement-level LIKE clone bodies")
        }
    }
}

fn normalize_table_element(
    element: &TableElement<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedTableElement {
    match element {
        TableElement::Column { column, .. } => {
            NormalizedTableElement::Column(NormalizedColumnDef {
                name: resolve_ident(&column.name, resolver),
                data_type: column
                    .data_type
                    .as_ref()
                    .map(|data_type| normalize_data_type(data_type, resolver)),
                constraints: column
                    .constraints
                    .iter()
                    .map(|constraint| NormalizedColumnConstraint {
                        name: constraint
                            .name
                            .as_ref()
                            .map(|name| resolve_ident(name, resolver)),
                        option: normalize_column_option(&constraint.option, resolver),
                    })
                    .collect(),
            })
        }
        TableElement::Constraint { constraint, .. } => {
            NormalizedTableElement::Constraint(normalize_table_constraint_def(constraint, resolver))
        }
        // The property generators never emit a `LIKE src …` copy element, so this normalization
        // (reached only from generator output and its reparse) never sees one.
        TableElement::Like { .. } => {
            unreachable!("the property generators do not produce a LIKE table element")
        }
    }
}

fn normalize_column_option(
    option: &ColumnOption<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedColumnOption {
    match option {
        ColumnOption::Null { .. } => NormalizedColumnOption::Null,
        ColumnOption::NotNull { .. } => NormalizedColumnOption::NotNull,
        ColumnOption::Default { expr, .. } => {
            NormalizedColumnOption::Default(normalize_expr(expr, resolver))
        }
        ColumnOption::Generated { generated, .. } => NormalizedColumnOption::Generated {
            expr: normalize_expr(&generated.expr, resolver),
            storage: generated.storage,
        },
        ColumnOption::Identity { identity, .. } => NormalizedColumnOption::Identity {
            generation: identity.generation,
            options: identity
                .options
                .iter()
                .map(|option| normalize_identity_option(option, resolver))
                .collect(),
        },
        ColumnOption::PrimaryKey { .. } => NormalizedColumnOption::PrimaryKey,
        ColumnOption::Unique { .. } => NormalizedColumnOption::Unique,
        ColumnOption::AutoIncrement { .. } => NormalizedColumnOption::AutoIncrement,
        ColumnOption::Collate { collation, .. } => {
            NormalizedColumnOption::Collate(resolve_name(collation, resolver))
        }
        ColumnOption::Check { expr, .. } => {
            NormalizedColumnOption::Check(normalize_expr(expr, resolver))
        }
        ColumnOption::References { reference, .. } => {
            NormalizedColumnOption::References(normalize_foreign_key_ref(reference, resolver))
        }
        ColumnOption::Bare { .. } => {
            unreachable!("the property generators do not produce a bare CONSTRAINT name")
        }
        ColumnOption::Other { ext, .. } => match *ext {},
    }
}

fn normalize_identity_option(
    option: &IdentityOption<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedIdentityOption {
    match option {
        IdentityOption::StartWith { expr, .. } => {
            NormalizedIdentityOption::StartWith(normalize_expr(expr, resolver))
        }
        IdentityOption::IncrementBy { expr, .. } => {
            NormalizedIdentityOption::IncrementBy(normalize_expr(expr, resolver))
        }
        IdentityOption::MinValue { value, .. } => NormalizedIdentityOption::MinValue(
            value.as_ref().map(|expr| normalize_expr(expr, resolver)),
        ),
        IdentityOption::MaxValue { value, .. } => NormalizedIdentityOption::MaxValue(
            value.as_ref().map(|expr| normalize_expr(expr, resolver)),
        ),
        IdentityOption::Cache { expr, .. } => {
            NormalizedIdentityOption::Cache(normalize_expr(expr, resolver))
        }
        IdentityOption::Cycle { cycle, .. } => NormalizedIdentityOption::Cycle(*cycle),
    }
}

fn normalize_table_constraint_def(
    constraint: &squonk_ast::TableConstraintDef<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedTableConstraintDef {
    NormalizedTableConstraintDef {
        name: constraint
            .name
            .as_ref()
            .map(|name| resolve_ident(name, resolver)),
        constraint: normalize_table_constraint(&constraint.constraint, resolver),
    }
}

fn normalize_table_constraint(
    constraint: &TableConstraint<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedTableConstraint {
    match constraint {
        TableConstraint::PrimaryKey { columns, .. } => {
            NormalizedTableConstraint::PrimaryKey(normalize_index_columns(columns, resolver))
        }
        TableConstraint::Unique { columns, .. } => {
            NormalizedTableConstraint::Unique(normalize_index_columns(columns, resolver))
        }
        TableConstraint::Check { expr, .. } => {
            NormalizedTableConstraint::Check(normalize_expr(expr, resolver))
        }
        TableConstraint::ForeignKey {
            columns,
            references,
            ..
        } => NormalizedTableConstraint::ForeignKey {
            columns: normalize_idents(columns, resolver),
            references: normalize_foreign_key_ref(references, resolver),
        },
        TableConstraint::Exclude { .. } => {
            unreachable!("the property generators do not produce EXCLUDE constraints")
        }
        TableConstraint::Bare { .. } => {
            unreachable!("the property generators do not produce a bare CONSTRAINT name")
        }
        TableConstraint::Other { ext, .. } => match *ext {},
    }
}

fn normalize_foreign_key_ref(
    reference: &squonk_ast::ForeignKeyRef,
    resolver: &dyn Resolver,
) -> NormalizedForeignKeyRef {
    NormalizedForeignKeyRef {
        table: resolve_name(&reference.table, resolver),
        columns: normalize_idents(&reference.columns, resolver),
        match_type: reference.match_type,
        on_delete: reference
            .on_delete
            .as_deref()
            .map(|action| normalize_referential_action(action, resolver)),
        on_update: reference
            .on_update
            .as_deref()
            .map(|action| normalize_referential_action(action, resolver)),
    }
}

fn normalize_referential_action(
    action: &squonk_ast::ReferentialAction,
    resolver: &dyn Resolver,
) -> NormalizedReferentialAction {
    use squonk_ast::ReferentialAction;
    match action {
        ReferentialAction::NoAction { .. } => NormalizedReferentialAction::NoAction,
        ReferentialAction::Restrict { .. } => NormalizedReferentialAction::Restrict,
        ReferentialAction::Cascade { .. } => NormalizedReferentialAction::Cascade,
        ReferentialAction::SetNull { columns, .. } => {
            NormalizedReferentialAction::SetNull(normalize_idents(columns, resolver))
        }
        ReferentialAction::SetDefault { columns, .. } => {
            NormalizedReferentialAction::SetDefault(normalize_idents(columns, resolver))
        }
    }
}

fn normalize_create_table_option_kind(
    kind: &CreateTableOptionKind<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedCreateTableOptionKind {
    match kind {
        CreateTableOptionKind::With { params, .. } => NormalizedCreateTableOptionKind::With(
            params
                .iter()
                .map(|param| NormalizedTableStorageParameter {
                    name: resolve_name(&param.name, resolver),
                    value: param
                        .value
                        .as_ref()
                        .map(|expr| normalize_expr(expr, resolver)),
                })
                .collect(),
        ),
        CreateTableOptionKind::OnCommit { action, .. } => {
            NormalizedCreateTableOptionKind::OnCommit(*action)
        }
        CreateTableOptionKind::Tablespace { tablespace, .. } => {
            NormalizedCreateTableOptionKind::Tablespace(resolve_ident(tablespace, resolver))
        }
        CreateTableOptionKind::KeyValue { option, .. } => {
            NormalizedCreateTableOptionKind::KeyValue {
                name: resolve_ident(&option.name, resolver),
                value: match &option.value {
                    TableOptionValue::Word { word, .. } => {
                        NormalizedTableOptionValue::Word(resolve_ident(word, resolver))
                    }
                    TableOptionValue::String { value, .. }
                    | TableOptionValue::Number { value, .. } => {
                        NormalizedTableOptionValue::Literal(value.kind.clone())
                    }
                },
            }
        }
        CreateTableOptionKind::WithoutRowid { .. } => NormalizedCreateTableOptionKind::WithoutRowid,
        CreateTableOptionKind::Strict { .. } => NormalizedCreateTableOptionKind::Strict,
        // The property generators never emit the PostgreSQL legacy `WITHOUT OIDS` option, so this
        // normalization (reached only from generator output and its reparse) never sees one.
        CreateTableOptionKind::WithoutOids { .. } => {
            unreachable!("the property generators do not produce WITHOUT OIDS")
        }
    }
}

fn normalize_query(query: &Query<NoExt>, resolver: &dyn Resolver) -> NormalizedQuery {
    NormalizedQuery {
        with: query
            .with
            .as_ref()
            .map(|with| normalize_with(with, resolver)),
        body: normalize_set_expr(&query.body, resolver),
        order_by: query
            .order_by
            .iter()
            .map(|item| normalize_order_by_item(item, resolver))
            .collect(),
        limit: query
            .limit
            .as_ref()
            .map(|limit| normalize_limit(limit, resolver)),
    }
}

/// Shared by the query and pivot-statement `LIMIT` positions so the mapping
/// cannot drift between them.
fn normalize_limit(limit: &Limit<NoExt>, resolver: &dyn Resolver) -> NormalizedLimit {
    NormalizedLimit {
        limit: limit
            .limit
            .as_ref()
            .map(|expr| normalize_expr(expr, resolver)),
        offset: limit
            .offset
            .as_ref()
            .map(|expr| normalize_expr(expr, resolver)),
        syntax: limit.syntax,
        with_ties: limit.with_ties,
        percent: limit.percent,
    }
}

/// Shared by every `ORDER BY` position (query-level, window spec, aggregate
/// function call) so the `using` mapping cannot drift between them.
fn normalize_order_by_item(
    item: &OrderByExpr<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedOrderBy {
    NormalizedOrderBy {
        expr: normalize_expr(&item.expr, resolver),
        asc: item.asc,
        using: item.using.as_ref().map(|using| {
            let mut parts = using
                .schema
                .as_ref()
                .map(|schema| normalize_idents(&schema.0, resolver))
                .unwrap_or_default();
            parts.push(resolver.resolve(using.op).into());
            parts
        }),
        nulls_first: item.nulls_first,
    }
}

fn normalize_with(with: &With<NoExt>, resolver: &dyn Resolver) -> NormalizedWith {
    NormalizedWith {
        recursive: with.recursive,
        ctes: with
            .ctes
            .iter()
            .map(|cte| NormalizedCte {
                name: resolver.resolve(cte.name.sym).into(),
                columns: cte
                    .columns
                    .iter()
                    .map(|ident| resolver.resolve(ident.sym).into())
                    .collect(),
                using_key: cte.using_key.as_ref().map(|key_columns| {
                    key_columns
                        .iter()
                        .map(|ident| resolver.resolve(ident.sym).into())
                        .collect()
                }),
                materialized: cte.materialized,
                body: match &cte.body {
                    CteBody::Query { query, .. } => {
                        NormalizedCteBody::Query(Box::new(normalize_query(query, resolver)))
                    }
                    CteBody::Insert { insert, .. } => {
                        NormalizedCteBody::Insert(Box::new(normalize_insert(insert, resolver)))
                    }
                    CteBody::Update { update, .. } => {
                        NormalizedCteBody::Update(Box::new(normalize_update(update, resolver)))
                    }
                    CteBody::Delete { delete, .. } => {
                        NormalizedCteBody::Delete(Box::new(normalize_delete(delete, resolver)))
                    }
                    // The normalized model covers what the property generators can
                    // build, and no generator (nor the top-level statement arm)
                    // produces a MERGE — mirroring `normalize_statement`'s treatment
                    // of the out-of-model statement kinds.
                    CteBody::Merge { .. } => {
                        unreachable!("the property generators never build a MERGE CTE body")
                    }
                },
            })
            .collect(),
    }
}

fn normalize_set_expr(set: &SetExpr<NoExt>, resolver: &dyn Resolver) -> NormalizedSetExpr {
    match set {
        SetExpr::Select { select, .. } => {
            NormalizedSetExpr::Select(normalize_select(select, resolver))
        }
        SetExpr::Values { values, .. } => NormalizedSetExpr::Values(
            values
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|item| normalize_values_item(item, resolver))
                        .collect()
                })
                .collect(),
        ),
        SetExpr::Query { query, .. } => {
            NormalizedSetExpr::Query(Box::new(normalize_query(query, resolver)))
        }
        SetExpr::SetOperation {
            op,
            all,
            left,
            right,
            ..
        } => NormalizedSetExpr::SetOperation {
            op: op.clone(),
            all: *all,
            left: Box::new(normalize_set_expr(left, resolver)),
            right: Box::new(normalize_set_expr(right, resolver)),
        },
        // The generated-AST round-trip builds only the ANSI reparse subset (the
        // generators quarantine every dialect-gated form), so a DuckDB PIVOT/UNPIVOT
        // query body is never synthesized here.
        SetExpr::Pivot { .. } | SetExpr::Unpivot { .. } => {
            unreachable!(
                "PIVOT/UNPIVOT query bodies are a DuckDB-only form the ANSI generators never build"
            )
        }
    }
}

fn normalize_select(select: &Select<NoExt>, resolver: &dyn Resolver) -> NormalizedSelect {
    NormalizedSelect {
        // Explicit `ALL` is the default, so it normalizes the same as no quantifier;
        // only `DISTINCT` / `DISTINCT ON` set the flag.
        distinct: matches!(
            select.distinct,
            Some(
                SelectDistinct::Quantifier {
                    quantifier: SetQuantifier::Distinct,
                    ..
                } | SelectDistinct::On { .. }
            )
        ),
        projection: select
            .projection
            .iter()
            .map(|item| normalize_select_item(item, resolver))
            .collect(),
        // `SELECT … INTO` is dialect-gated and never produced by the generators that
        // feed this structural oracle, so the create-table target needs no normalized
        // projection here; the real-SQL round-trip oracle compares it via the AST's
        // derived structural equality.
        from: select
            .from
            .iter()
            .map(|table| normalize_table_with_joins(table, resolver))
            .collect(),
        selection: select
            .selection
            .as_ref()
            .map(|expr| normalize_expr(expr, resolver)),
        group_by: select
            .group_by
            .iter()
            .map(|item| normalize_group_by_item(item, resolver))
            .collect(),
        having: select
            .having
            .as_ref()
            .map(|expr| normalize_expr(expr, resolver)),
        windows: select
            .windows
            .iter()
            .map(|window| normalize_named_window(window, resolver))
            .collect(),
    }
}

fn normalize_group_by_item(
    item: &GroupByItem<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedGroupByItem {
    let exprs = |exprs: &[Expr<NoExt>]| exprs.iter().map(|e| normalize_expr(e, resolver)).collect();
    match item {
        GroupByItem::Expr { expr, .. } => {
            NormalizedGroupByItem::Expr(normalize_expr(expr, resolver))
        }
        GroupByItem::Rollup { exprs: items, .. } => NormalizedGroupByItem::Rollup(exprs(items)),
        GroupByItem::Cube { exprs: items, .. } => NormalizedGroupByItem::Cube(exprs(items)),
        GroupByItem::GroupingSets { sets, .. } => NormalizedGroupByItem::GroupingSets(
            sets.iter()
                .map(|set| normalize_group_by_item(set, resolver))
                .collect(),
        ),
        GroupByItem::Empty { .. } => NormalizedGroupByItem::Empty,
    }
}

fn normalize_named_window(
    window: &NamedWindow<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedNamedWindow {
    NormalizedNamedWindow {
        name: resolver.resolve(window.name.sym).into(),
        definition: normalize_window_definition(&window.definition, resolver),
    }
}

fn normalize_window_spec(
    spec: &WindowSpec<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedWindowSpec {
    match spec {
        WindowSpec::Named { name, .. } => {
            NormalizedWindowSpec::Named(resolver.resolve(name.sym).into())
        }
        WindowSpec::Inline { definition, .. } => {
            NormalizedWindowSpec::Inline(normalize_window_definition(definition, resolver))
        }
    }
}

fn normalize_window_definition(
    definition: &WindowDefinition<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedWindowDefinition {
    NormalizedWindowDefinition {
        existing: definition
            .existing
            .as_ref()
            .map(|ident| resolver.resolve(ident.sym).into()),
        partition_by: definition
            .partition_by
            .iter()
            .map(|expr| normalize_expr(expr, resolver))
            .collect(),
        order_by: definition
            .order_by
            .iter()
            .map(|item| normalize_order_by_item(item, resolver))
            .collect(),
        frame: definition
            .frame
            .as_ref()
            .map(|frame| normalize_window_frame(frame, resolver)),
    }
}

fn normalize_window_frame(
    frame: &WindowFrame<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedWindowFrame {
    NormalizedWindowFrame {
        units: frame.units,
        start: normalize_window_frame_bound(&frame.start, resolver),
        end: frame
            .end
            .as_ref()
            .map(|bound| normalize_window_frame_bound(bound, resolver)),
        exclusion: frame.exclusion,
    }
}

fn normalize_window_frame_bound(
    bound: &WindowFrameBound<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedWindowFrameBound {
    match bound {
        WindowFrameBound::CurrentRow { .. } => NormalizedWindowFrameBound::CurrentRow,
        WindowFrameBound::UnboundedPreceding { .. } => {
            NormalizedWindowFrameBound::UnboundedPreceding
        }
        WindowFrameBound::UnboundedFollowing { .. } => {
            NormalizedWindowFrameBound::UnboundedFollowing
        }
        WindowFrameBound::Preceding { offset, .. } => {
            NormalizedWindowFrameBound::Preceding(normalize_expr(offset, resolver))
        }
        WindowFrameBound::Following { offset, .. } => {
            NormalizedWindowFrameBound::Following(normalize_expr(offset, resolver))
        }
    }
}

fn normalize_select_item(
    item: &SelectItem<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedSelectItem {
    match item {
        SelectItem::Wildcard { .. } => NormalizedSelectItem::Wildcard,
        SelectItem::QualifiedWildcard { name, .. } => {
            NormalizedSelectItem::QualifiedWildcard(resolve_name(name, resolver))
        }
        SelectItem::Expr { expr, alias, .. } => NormalizedSelectItem::Expr {
            expr: normalize_expr(expr, resolver),
            alias: alias
                .as_ref()
                .map(|ident| resolver.resolve(ident.sym).into()),
        },
    }
}

fn normalize_table_with_joins(
    table: &TableWithJoins<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedTableWithJoins {
    NormalizedTableWithJoins {
        relation: normalize_table_factor(&table.relation, resolver),
        joins: table
            .joins
            .iter()
            .map(|join| normalize_join(join, resolver))
            .collect(),
    }
}

fn normalize_table_factor(
    factor: &TableFactor<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedTableFactor {
    match factor {
        TableFactor::Table {
            name,
            inheritance,
            alias,
            sample,
            ..
        } => NormalizedTableFactor::Table {
            name: resolve_name(name, resolver),
            inheritance: inheritance.clone(),
            alias: alias
                .as_ref()
                .map(|alias| normalize_table_alias(alias, resolver)),
            sample: sample
                .as_ref()
                .map(|sample| normalize_table_sample(sample, resolver)),
        },
        TableFactor::Derived {
            lateral,
            subquery,
            alias,
            ..
        } => NormalizedTableFactor::Derived {
            lateral: *lateral,
            subquery: Box::new(normalize_query(subquery, resolver)),
            alias: alias
                .as_ref()
                .map(|alias| normalize_table_alias(alias, resolver)),
        },
        TableFactor::Function {
            lateral,
            function,
            with_ordinality,
            alias,
            column_defs,
            ..
        } => NormalizedTableFactor::Function {
            lateral: *lateral,
            function: Box::new(normalize_function_call(function, resolver)),
            with_ordinality: *with_ordinality,
            alias: alias
                .as_ref()
                .map(|alias| normalize_table_alias(alias, resolver)),
            column_defs: column_defs
                .iter()
                .map(|column| normalize_table_function_column(column, resolver))
                .collect(),
        },
        TableFactor::RowsFrom {
            lateral,
            functions,
            with_ordinality,
            alias,
            ..
        } => NormalizedTableFactor::RowsFrom {
            lateral: *lateral,
            functions: functions
                .iter()
                .map(|item| NormalizedRowsFromItem {
                    function: normalize_function_call(&item.function, resolver),
                    column_defs: item
                        .column_defs
                        .iter()
                        .map(|column| normalize_table_function_column(column, resolver))
                        .collect(),
                })
                .collect(),
            with_ordinality: *with_ordinality,
            alias: alias
                .as_ref()
                .map(|alias| normalize_table_alias(alias, resolver)),
        },
        TableFactor::Unnest {
            lateral,
            array_exprs,
            with_ordinality,
            alias,
            column_defs,
            with_offset,
            with_offset_alias,
            ..
        } => NormalizedTableFactor::Unnest {
            lateral: *lateral,
            array_exprs: array_exprs
                .iter()
                .map(|expr| normalize_expr(expr, resolver))
                .collect(),
            with_ordinality: *with_ordinality,
            alias: alias
                .as_ref()
                .map(|alias| normalize_table_alias(alias, resolver)),
            column_defs: column_defs
                .iter()
                .map(|column| normalize_table_function_column(column, resolver))
                .collect(),
            with_offset: *with_offset,
            with_offset_alias: with_offset_alias
                .as_ref()
                .map(|ident| resolver.resolve(ident.sym).into()),
        },
        TableFactor::NestedJoin { table, alias, .. } => NormalizedTableFactor::NestedJoin {
            table: Box::new(normalize_table_with_joins(table, resolver)),
            alias: alias
                .as_ref()
                .map(|alias| normalize_table_alias(alias, resolver)),
        },
        TableFactor::SpecialFunction {
            keyword,
            precision,
            alias,
            ..
        } => NormalizedTableFactor::SpecialFunction {
            keyword: *keyword,
            precision: *precision,
            alias: alias
                .as_ref()
                .map(|alias| normalize_table_alias(alias, resolver)),
        },
        TableFactor::Pivot { pivot, alias, .. } => {
            NormalizedTableFactor::Pivot(Box::new(NormalizedPivot {
                source: Box::new(normalize_table_factor(&pivot.source, resolver)),
                aggregates: pivot
                    .aggregates
                    .iter()
                    .map(|aggregate| normalize_pivot_expr(aggregate, resolver))
                    .collect(),
                pivot_on: pivot
                    .pivot_on
                    .iter()
                    .map(|column| NormalizedPivotColumn {
                        expr: normalize_expr(&column.expr, resolver),
                        values: column
                            .values
                            .iter()
                            .map(|value| normalize_pivot_expr(value, resolver))
                            .collect(),
                        enum_source: column
                            .enum_source
                            .as_ref()
                            .map(|name| resolve_ident(name, resolver)),
                    })
                    .collect(),
                group_by: pivot
                    .group_by
                    .iter()
                    .map(|expr| normalize_expr(expr, resolver))
                    .collect(),
                with: pivot
                    .with
                    .as_ref()
                    .map(|with| normalize_with(with, resolver)),
                order_by: pivot
                    .order_by
                    .iter()
                    .map(|item| normalize_order_by_item(item, resolver))
                    .collect(),
                order_by_all: pivot
                    .order_by_all
                    .as_ref()
                    .map(|all| (all.asc, all.nulls_first)),
                limit: pivot
                    .limit
                    .as_ref()
                    .map(|limit| normalize_limit(limit, resolver)),
                spelling: pivot.spelling,
                alias: alias
                    .as_ref()
                    .map(|alias| normalize_table_alias(alias, resolver)),
            }))
        }
        TableFactor::Unpivot { unpivot, alias, .. } => {
            NormalizedTableFactor::Unpivot(Box::new(NormalizedUnpivot {
                source: Box::new(normalize_table_factor(&unpivot.source, resolver)),
                value: normalize_idents(&unpivot.value, resolver),
                name: normalize_idents(&unpivot.name, resolver),
                columns: unpivot
                    .columns
                    .iter()
                    .map(|column| NormalizedUnpivotColumn {
                        columns: column
                            .columns
                            .iter()
                            .map(|expr| normalize_expr(expr, resolver))
                            .collect(),
                        alias: column
                            .alias
                            .as_ref()
                            .map(|alias| resolve_ident(alias, resolver)),
                    })
                    .collect(),
                null_inclusion: unpivot.null_inclusion,
                with: unpivot
                    .with
                    .as_ref()
                    .map(|with| normalize_with(with, resolver)),
                order_by: unpivot
                    .order_by
                    .iter()
                    .map(|item| normalize_order_by_item(item, resolver))
                    .collect(),
                order_by_all: unpivot
                    .order_by_all
                    .as_ref()
                    .map(|all| (all.asc, all.nulls_first)),
                limit: unpivot
                    .limit
                    .as_ref()
                    .map(|limit| normalize_limit(limit, resolver)),
                spelling: unpivot.spelling,
                alias: alias
                    .as_ref()
                    .map(|alias| normalize_table_alias(alias, resolver)),
            }))
        }
        // DuckDB's DESCRIBE/SHOW/SUMMARIZE table source is a dialect-gated form the ANSI
        // generators never build (see the PIVOT/UNPIVOT note in `normalize_set_expr`).
        TableFactor::ShowRef { .. } => {
            unreachable!(
                "DESCRIBE/SHOW/SUMMARIZE table sources are a DuckDB-only form the ANSI generators never build"
            )
        }
        // JSON_TABLE / XMLTABLE are PostgreSQL-only column-defining table factors the ANSI
        // property generators never build (the ShowRef precedent above).
        TableFactor::JsonTable { .. } => {
            unreachable!("JSON_TABLE is a PostgreSQL-only form the ANSI generators never build")
        }
        TableFactor::XmlTable { .. } => {
            unreachable!("XMLTABLE is a PostgreSQL-only form the ANSI generators never build")
        }
        // `TABLE(<expr>)` is a Lenient-only form (no oracle-backed preset ships it) the
        // ANSI property generators never build (the ShowRef precedent above).
        TableFactor::TableExpr { .. } => {
            unreachable!("TABLE(<expr>) is a Lenient-only form the ANSI generators never build")
        }
        // MATCH_RECOGNIZE is a Snowflake/Lenient-only form the ANSI property generators
        // never build (the TableExpr precedent above).
        TableFactor::MatchRecognize { .. } => {
            unreachable!(
                "MATCH_RECOGNIZE is a Snowflake/Lenient-only form the ANSI generators never build"
            )
        }
        // OPENJSON is an MSSQL/Lenient-only form the ANSI property generators never build
        // (the TableExpr precedent above).
        TableFactor::OpenJson { .. } => {
            unreachable!("OPENJSON is an MSSQL/Lenient-only form the ANSI generators never build")
        }
        TableFactor::Other { ext, .. } => match *ext {},
    }
}

fn normalize_pivot_expr(expr: &PivotExpr<NoExt>, resolver: &dyn Resolver) -> NormalizedPivotExpr {
    NormalizedPivotExpr {
        expr: normalize_expr(&expr.expr, resolver),
        alias: expr
            .alias
            .as_ref()
            .map(|alias| resolve_ident(alias, resolver)),
    }
}

fn normalize_table_alias(alias: &TableAlias, resolver: &dyn Resolver) -> NormalizedTableAlias {
    NormalizedTableAlias {
        name: resolver.resolve(alias.name.sym).into(),
        columns: alias
            .columns
            .iter()
            .map(|ident| resolver.resolve(ident.sym).into())
            .collect(),
    }
}

fn normalize_table_sample(
    sample: &TableSample<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedTableSample {
    NormalizedTableSample {
        method: resolve_name(&sample.method, resolver),
        args: sample
            .args
            .iter()
            .map(|expr| normalize_expr(expr, resolver))
            .collect(),
        repeatable: sample
            .repeatable
            .as_ref()
            .map(|expr| normalize_expr(expr, resolver)),
    }
}

fn normalize_function_call(
    call: &FunctionCall<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedFunctionCall {
    NormalizedFunctionCall {
        name: resolve_name(&call.name, resolver),
        quantifier: call.quantifier,
        // The generator emits only positional arguments, so the argument name and
        // surface tag are constant here; normalize the values themselves.
        args: call
            .args
            .iter()
            .map(|arg| normalize_expr(&arg.value, resolver))
            .collect(),
        wildcard: call.wildcard,
        order_by: call
            .order_by
            .iter()
            .map(|item| normalize_order_by_item(item, resolver))
            .collect(),
        // The same sortby normalization the plain ORDER BY items use: a WITHIN
        // GROUP list is PG's `sort_clause`, so its items admit `USING <op>` too.
        within_group: call.within_group.as_ref().map(|items| {
            items
                .iter()
                .map(|item| normalize_order_by_item(item, resolver))
                .collect()
        }),
        filter: call
            .filter
            .as_ref()
            .map(|expr| Box::new(normalize_expr(expr, resolver))),
        over: call
            .over
            .as_ref()
            .map(|spec| normalize_window_spec(spec, resolver)),
    }
}

fn normalize_table_function_column(
    column: &TableFunctionColumn,
    resolver: &dyn Resolver,
) -> NormalizedTableFunctionColumn {
    NormalizedTableFunctionColumn {
        name: resolver.resolve(column.name.sym).into(),
        data_type: normalize_data_type(&column.data_type, resolver),
    }
}

fn normalize_join(join: &Join<NoExt>, resolver: &dyn Resolver) -> NormalizedJoin {
    NormalizedJoin {
        relation: normalize_table_factor(&join.relation, resolver),
        operator: match &join.operator {
            JoinOperator::Inner { constraint, .. } => {
                NormalizedJoinOperator::Inner(normalize_join_constraint(constraint, resolver))
            }
            JoinOperator::LeftOuter { constraint, .. } => {
                NormalizedJoinOperator::LeftOuter(normalize_join_constraint(constraint, resolver))
            }
            JoinOperator::RightOuter { constraint, .. } => {
                NormalizedJoinOperator::RightOuter(normalize_join_constraint(constraint, resolver))
            }
            JoinOperator::FullOuter { constraint, .. } => {
                NormalizedJoinOperator::FullOuter(normalize_join_constraint(constraint, resolver))
            }
            JoinOperator::AsOf {
                kind, constraint, ..
            } => {
                NormalizedJoinOperator::AsOf(*kind, normalize_join_constraint(constraint, resolver))
            }
            JoinOperator::Cross { .. } => NormalizedJoinOperator::Cross,
            JoinOperator::Positional { .. } => NormalizedJoinOperator::Positional,
            JoinOperator::Semi {
                asof,
                side,
                constraint,
                ..
            } => NormalizedJoinOperator::Semi(
                *asof,
                *side,
                normalize_join_constraint(constraint, resolver),
            ),
            JoinOperator::Anti {
                asof,
                side,
                constraint,
                ..
            } => NormalizedJoinOperator::Anti(
                *asof,
                *side,
                normalize_join_constraint(constraint, resolver),
            ),
            JoinOperator::Apply { kind, .. } => NormalizedJoinOperator::Apply(*kind),
        },
    }
}

fn normalize_join_constraint(
    constraint: &squonk_ast::JoinConstraint<NoExt>,
    resolver: &dyn Resolver,
) -> NormalizedJoinConstraint {
    match constraint {
        squonk_ast::JoinConstraint::On { expr, .. } => {
            NormalizedJoinConstraint::On(normalize_expr(expr, resolver))
        }
        squonk_ast::JoinConstraint::Using { columns, alias, .. } => {
            NormalizedJoinConstraint::Using {
                columns: columns
                    .iter()
                    .map(|ident| resolver.resolve(ident.sym).into())
                    .collect(),
                alias: alias
                    .as_ref()
                    .map(|ident| resolver.resolve(ident.sym).into()),
            }
        }
        squonk_ast::JoinConstraint::Natural { .. } => NormalizedJoinConstraint::Natural,
        squonk_ast::JoinConstraint::None { .. } => NormalizedJoinConstraint::None,
    }
}

fn normalize_expr(expr: &Expr<NoExt>, resolver: &dyn Resolver) -> NormalizedExpr {
    match expr {
        Expr::Column { name, .. } => NormalizedExpr::Column(resolve_name(name, resolver)),
        Expr::Literal { literal, .. } => NormalizedExpr::Literal(literal.kind.clone()),
        Expr::BinaryOp {
            left, op, right, ..
        } => NormalizedExpr::Binary {
            left: Box::new(normalize_expr(left, resolver)),
            op: op.clone(),
            right: Box::new(normalize_expr(right, resolver)),
        },
        Expr::UnaryOp { op, expr, .. } => NormalizedExpr::Unary {
            op: op.clone(),
            expr: Box::new(normalize_expr(expr, resolver)),
        },
        Expr::Function { call, .. } => {
            NormalizedExpr::Function(Box::new(normalize_function_call(call, resolver)))
        }
        Expr::Cast {
            expr, data_type, ..
        } => NormalizedExpr::Cast {
            expr: Box::new(normalize_expr(expr, resolver)),
            data_type: normalize_data_type(data_type, resolver),
        },
        Expr::InSubquery {
            expr,
            subquery,
            negated,
            ..
        } => NormalizedExpr::InSubquery {
            expr: Box::new(normalize_expr(expr, resolver)),
            subquery: Box::new(normalize_query(subquery, resolver)),
            negated: *negated,
        },
        Expr::Exists { query, .. } => {
            NormalizedExpr::Exists(Box::new(normalize_query(query, resolver)))
        }
        Expr::QuantifiedComparison {
            left,
            op,
            quantifier,
            subquery,
            ..
        } => NormalizedExpr::QuantifiedComparison {
            left: Box::new(normalize_expr(left, resolver)),
            op: op.clone(),
            quantifier: *quantifier,
            subquery: Box::new(normalize_query(subquery, resolver)),
        },
        Expr::Subquery { query, .. } => {
            NormalizedExpr::Subquery(Box::new(normalize_query(query, resolver)))
        }
        Expr::IsNull { .. }
        | Expr::IsTruth { .. }
        | Expr::IsNormalized { .. }
        | Expr::Between { .. }
        | Expr::Like { .. }
        | Expr::InList { .. }
        | Expr::InExpr { .. }
        | Expr::Case { .. }
        | Expr::Extract { .. }
        | Expr::Parameter { .. }
        | Expr::PositionalColumn { .. }
        | Expr::SessionVariable { .. }
        | Expr::Subscript { .. }
        | Expr::SemiStructuredAccess { .. }
        | Expr::Collate { .. }
        | Expr::AtTimeZone { .. }
        | Expr::Interval { .. }
        | Expr::Array { .. }
        | Expr::Struct { .. }
        | Expr::StructConstructor { .. }
        | Expr::Map { .. }
        | Expr::Row { .. }
        | Expr::FieldSelection { .. }
        | Expr::NamedOperator { .. }
        | Expr::PrefixOperator { .. }
        | Expr::PostfixOperator { .. }
        | Expr::Lambda { .. }
        | Expr::Columns { .. }
        | Expr::QuantifiedList { .. }
        | Expr::QuantifiedLike { .. }
        | Expr::SpecialFunction { .. }
        | Expr::JsonFunc { .. }
        | Expr::JsonObject { .. }
        | Expr::JsonArray { .. }
        | Expr::JsonAggregate { .. }
        | Expr::JsonConstructor { .. }
        | Expr::IsJson { .. }
        | Expr::XmlFunc { .. }
        | Expr::IsDocument { .. }
        | Expr::StringFunc { .. }
        | Expr::Other { .. } => {
            unreachable!("generated M1 proptest subset does not emit this expression variant")
        }
    }
}

fn normalize_data_type(data_type: &DataType, resolver: &dyn Resolver) -> NormalizedDataType {
    match data_type {
        DataType::Boolean { spelling, .. } => NormalizedDataType::Boolean(*spelling),
        DataType::SmallInt { .. } => NormalizedDataType::SmallInt,
        DataType::Integer { spelling, .. } => NormalizedDataType::Integer(*spelling),
        DataType::BigInt { .. } => NormalizedDataType::BigInt,
        DataType::Decimal {
            spelling,
            precision,
            scale,
            ..
        } => NormalizedDataType::Decimal {
            spelling: *spelling,
            precision: *precision,
            scale: *scale,
        },
        DataType::Float { precision, .. } => NormalizedDataType::Float(*precision),
        DataType::Real { .. } => NormalizedDataType::Real,
        DataType::Double { spelling, .. } => NormalizedDataType::Double(*spelling),
        DataType::Text { .. } => NormalizedDataType::Text,
        DataType::Character { spelling, size, .. } => NormalizedDataType::Character {
            spelling: *spelling,
            size: *size,
        },
        DataType::Binary { spelling, size, .. } => NormalizedDataType::Binary {
            spelling: *spelling,
            size: *size,
        },
        DataType::Date { .. } => NormalizedDataType::Date,
        DataType::Time {
            spelling,
            precision,
            time_zone,
            ..
        } => NormalizedDataType::Time {
            spelling: *spelling,
            precision: *precision,
            time_zone: *time_zone,
        },
        DataType::Timestamp {
            spelling,
            precision,
            time_zone,
            ..
        } => NormalizedDataType::Timestamp {
            spelling: *spelling,
            precision: *precision,
            time_zone: *time_zone,
        },
        DataType::Interval {
            fields, precision, ..
        } => NormalizedDataType::Interval {
            fields: *fields,
            precision: *precision,
        },
        DataType::Array { element, .. } => {
            NormalizedDataType::Array(Box::new(normalize_data_type(element, resolver)))
        }
        DataType::UserDefined {
            name, modifiers, ..
        } => NormalizedDataType::UserDefined {
            name: resolve_name(name, resolver),
            modifiers: modifiers.iter().map(|m| m.kind.clone()).collect(),
        },
        DataType::Bit { .. }
        | DataType::Json { .. }
        | DataType::Uuid { .. }
        | DataType::TinyInt { .. }
        | DataType::MediumInt { .. }
        | DataType::Blob { .. }
        | DataType::Enum { .. }
        | DataType::Set { .. }
        | DataType::NumericModifier { .. }
        | DataType::Struct { .. }
        | DataType::Union { .. }
        | DataType::Map { .. }
        | DataType::Wrapped { .. }
        | DataType::FixedString { .. }
        | DataType::DateTime64 { .. }
        | DataType::Nested { .. }
        | DataType::FixedWidthInt { .. }
        | DataType::Liberal { .. } => {
            unreachable!("generated M1 proptest subset does not emit this data type")
        }
        // Uninhabited under the builtin `NoExt`, like the other seams above.
        DataType::Other { ext, .. } => match *ext {},
    }
}

fn resolve_name(name: &ObjectName, resolver: &dyn Resolver) -> Vec<String> {
    name.0
        .iter()
        .map(|ident| resolve_ident(ident, resolver))
        .collect()
}

fn normalize_idents(idents: &[Ident], resolver: &dyn Resolver) -> Vec<String> {
    idents
        .iter()
        .map(|ident| resolve_ident(ident, resolver))
        .collect()
}

fn normalize_index_columns(
    columns: &[IndexColumn<NoExt>],
    resolver: &dyn Resolver,
) -> Vec<NormalizedIndexColumn> {
    columns
        .iter()
        .map(|column| NormalizedIndexColumn {
            expr: normalize_expr(&column.expr, resolver),
            asc: column.asc,
        })
        .collect()
}

fn resolve_ident(ident: &Ident, resolver: &dyn Resolver) -> String {
    resolver.resolve(ident.sym).into()
}
