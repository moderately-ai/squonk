// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! PostgreSQL protobuf -> neutral structural shape mapper.
//!
//! The `pg_*` functions that map a `pg_query` protobuf parse tree into the
//! engine-neutral `*Shape` vocabulary ([`crate::shape`]). [`pg_shape`] is the entry
//! point the module root re-exports (`crate::pg::pg_shape`); every other function here
//! is an internal per-node mapper it drives. Split out of `pg.rs` (the module root)
//! under the file+dir idiom so the root keeps only the accept/reject and structural
//! oracles, the divergence allowlist, and the parity helpers.

use crate::shape::*;
use pg_query::NodeEnum;
use pg_query::protobuf as pgpb;
use pg_query::protobuf::a_const;
use squonk_ast::{InsertOverriding, Quantifier};

/// Extract the neutral shape from a PostgreSQL protobuf parse result.
pub fn pg_shape(result: &pgpb::ParseResult) -> Result<Vec<StatementShape>, String> {
    result
        .stmts
        .iter()
        .map(|stmt| {
            let node = stmt
                .stmt
                .as_deref()
                .ok_or_else(|| "RawStmt has no stmt".to_string())?;
            pg_node_shape(node)
        })
        .collect()
}

/// Map one PostgreSQL protobuf statement node to its neutral shape. Split out from
/// [`pg_shape`] so [`EXPLAIN`](StatementShape::Explain) can recurse into its inner
/// statement node.
fn pg_node_shape(node: &pgpb::Node) -> Result<StatementShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::SelectStmt(select)) => pg_query_shape(select).map(StatementShape::Query),
        Some(NodeEnum::CreateStmt(create)) => {
            pg_create_stmt_shape(create).map(StatementShape::CreateTable)
        }
        // `CREATE TABLE ... AS` and `CREATE MATERIALIZED VIEW` share the
        // `CreateTableAsStmt` node, distinguished by `objtype`.
        Some(NodeEnum::CreateTableAsStmt(create)) => pg_create_table_as_node_shape(create),
        Some(NodeEnum::AlterTableStmt(alter)) => {
            pg_alter_table_shape(alter).map(StatementShape::AlterTable)
        }
        Some(NodeEnum::DropStmt(drop)) => pg_drop_shape(drop).map(StatementShape::Drop),
        Some(NodeEnum::CreateSchemaStmt(schema)) => {
            pg_create_schema_shape(schema).map(StatementShape::CreateSchema)
        }
        Some(NodeEnum::ViewStmt(view)) => pg_view_stmt_shape(view).map(StatementShape::CreateView),
        Some(NodeEnum::IndexStmt(index)) => {
            pg_index_stmt_shape(index).map(StatementShape::CreateIndex)
        }
        Some(NodeEnum::InsertStmt(insert)) => pg_insert_shape(insert).map(StatementShape::Insert),
        Some(NodeEnum::UpdateStmt(update)) => pg_update_shape(update).map(StatementShape::Update),
        Some(NodeEnum::DeleteStmt(delete)) => pg_delete_shape(delete).map(StatementShape::Delete),
        Some(NodeEnum::TransactionStmt(transaction)) => pg_transaction_shape(transaction),
        // `SET TRANSACTION` also lowers to a `VariableSetStmt`, so the router decides
        // between a transaction and a session shape.
        Some(NodeEnum::VariableSetStmt(set)) => pg_variable_set_shape(set),
        Some(NodeEnum::VariableShowStmt(show)) => Ok(StatementShape::Session(SessionShape::Show {
            name: show.name.clone(),
        })),
        Some(NodeEnum::GrantStmt(grant)) => {
            pg_grant_shape(grant).map(StatementShape::AccessControl)
        }
        Some(NodeEnum::GrantRoleStmt(grant)) => {
            pg_grant_role_shape(grant).map(StatementShape::AccessControl)
        }
        Some(NodeEnum::RenameStmt(rename)) => {
            pg_role_rename_shape(rename).map(StatementShape::AccessControl)
        }
        Some(NodeEnum::ExplainStmt(explain)) => {
            pg_explain_shape(explain).map(StatementShape::Explain)
        }
        Some(NodeEnum::TruncateStmt(truncate)) => {
            pg_truncate_shape(truncate).map(StatementShape::Truncate)
        }
        Some(NodeEnum::CommentStmt(comment)) => {
            pg_comment_shape(comment).map(StatementShape::CommentOn)
        }
        other => Err(format!("unmapped top-level PostgreSQL node: {other:?}")),
    }
}

fn pg_role_rename_shape(rename: &pgpb::RenameStmt) -> Result<AccessControlShape, String> {
    if pgpb::ObjectType::try_from(rename.rename_type).unwrap_or(pgpb::ObjectType::Undefined)
        != pgpb::ObjectType::ObjectRole
    {
        return Err("unsupported PostgreSQL RENAME object".to_string());
    }
    let object = rename
        .object
        .as_deref()
        .ok_or_else(|| "PostgreSQL ALTER ROLE RENAME has no role".to_string())?;
    let name = match object.node.as_ref() {
        Some(NodeEnum::RoleSpec(role)) => role.rolename.clone(),
        Some(NodeEnum::String(value)) => value.sval.clone(),
        Some(NodeEnum::List(list)) => pg_string_list(&list.items)?.join("."),
        other => return Err(format!("unsupported PostgreSQL role name node: {other:?}")),
    };
    Ok(AccessControlShape::RoleRename {
        name,
        new_name: rename.newname.clone(),
    })
}

// ---- PostgreSQL protobuf -> statement shape --------------------------------

fn pg_create_table_as_node_shape(stmt: &pgpb::CreateTableAsStmt) -> Result<StatementShape, String> {
    match pgpb::ObjectType::try_from(stmt.objtype).unwrap_or(pgpb::ObjectType::Undefined) {
        pgpb::ObjectType::ObjectTable => {
            pg_create_table_as_shape(stmt).map(StatementShape::CreateTable)
        }
        pgpb::ObjectType::ObjectMatview => pg_matview_shape(stmt).map(StatementShape::CreateView),
        other => Err(format!(
            "unsupported PostgreSQL CREATE TABLE AS object type: {other:?}"
        )),
    }
}

fn pg_create_stmt_shape(stmt: &pgpb::CreateStmt) -> Result<CreateTableShape, String> {
    if !stmt.inh_relations.is_empty() {
        return Err("unsupported PostgreSQL table inheritance".to_string());
    }
    if stmt.partbound.is_some() || stmt.partspec.is_some() {
        return Err("unsupported PostgreSQL table partitioning".to_string());
    }
    if stmt.of_typename.is_some() {
        return Err("unsupported PostgreSQL typed (OF) table".to_string());
    }
    let relation = stmt
        .relation
        .as_ref()
        .ok_or_else(|| "PostgreSQL CREATE TABLE has no relation".to_string())?;
    Ok(CreateTableShape {
        temporary: pg_is_temp(&relation.relpersistence),
        unlogged: pg_is_unlogged(&relation.relpersistence),
        if_not_exists: stmt.if_not_exists,
        name: pg_range_var_name(relation),
        body: CreateTableBodyShape::Definition(
            stmt.table_elts
                .iter()
                .map(pg_table_element_shape)
                .collect::<Result<_, _>>()?,
        ),
        access_method: pg_nonempty_string(&stmt.access_method),
        options: pg_create_stmt_options(stmt)?,
    })
}

fn pg_create_table_as_shape(stmt: &pgpb::CreateTableAsStmt) -> Result<CreateTableShape, String> {
    let into = stmt
        .into
        .as_ref()
        .ok_or_else(|| "PostgreSQL CREATE TABLE AS has no INTO clause".to_string())?;
    let rel = into
        .rel
        .as_ref()
        .ok_or_else(|| "PostgreSQL CREATE TABLE AS has no target relation".to_string())?;
    Ok(CreateTableShape {
        temporary: pg_is_temp(&rel.relpersistence),
        unlogged: pg_is_unlogged(&rel.relpersistence),
        if_not_exists: stmt.if_not_exists,
        name: pg_range_var_name(rel),
        body: CreateTableBodyShape::AsQuery {
            columns: pg_string_list(&into.col_names)?,
            query: Box::new(pg_query_shape_from_node(stmt.query.as_deref())?),
            no_data: into.skip_data,
        },
        // The CTAS `USING <method>` slot (before `AS`) reports through the `IntoClause`.
        access_method: pg_nonempty_string(&into.access_method),
        options: pg_into_clause_options(into)?,
    })
}

fn pg_table_element_shape(node: &pgpb::Node) -> Result<TableElementShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::ColumnDef(column)) => {
            Ok(TableElementShape::Column(pg_table_column_shape(column)?))
        }
        Some(NodeEnum::Constraint(constraint)) => Ok(TableElementShape::Constraint(
            pg_table_constraint_shape(constraint)?,
        )),
        other => Err(format!("unsupported PostgreSQL table element: {other:?}")),
    }
}

fn pg_table_column_shape(column: &pgpb::ColumnDef) -> Result<TableColumnShape, String> {
    let type_name = column
        .type_name
        .as_ref()
        .ok_or_else(|| "PostgreSQL column definition has no type".to_string())?;
    Ok(TableColumnShape {
        name: column.colname.clone(),
        data_type: pg_type_name_shape(type_name)?,
        // The column collation hangs on the `ColumnDef` (`collClause`), where our side lifts
        // its first parsed collate constraint to meet it.
        collation: match &column.coll_clause {
            Some(collate) => Some(pg_string_list(&collate.collname)?),
            None => None,
        },
        // `STORAGE <name>` fills `storage_name` (PostgreSQL 16+; the legacy single-char
        // `storage` field stays empty at raw parse — engine-measured on libpg_query 6/PG17);
        // `COMPRESSION <name>` fills `compression`. Both are empty strings when unwritten.
        storage: pg_nonempty_string(&column.storage_name),
        compression: pg_nonempty_string(&column.compression),
        constraints: column
            .constraints
            .iter()
            .map(pg_column_constraint_shape)
            .collect::<Result<_, _>>()?,
    })
}

fn pg_column_constraint_shape(node: &pgpb::Node) -> Result<ColumnConstraintShape, String> {
    let Some(NodeEnum::Constraint(constraint)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL column constraint node: {:?}",
            node.node
        ));
    };
    Ok(ColumnConstraintShape {
        name: (!constraint.conname.is_empty()).then(|| constraint.conname.clone()),
        option: pg_column_option_shape(constraint)?,
    })
}

fn pg_column_option_shape(constraint: &pgpb::Constraint) -> Result<ColumnOptionShape, String> {
    match pgpb::ConstrType::try_from(constraint.contype).unwrap_or(pgpb::ConstrType::Undefined) {
        pgpb::ConstrType::ConstrNull => Ok(ColumnOptionShape::Null),
        pgpb::ConstrType::ConstrNotnull => Ok(ColumnOptionShape::NotNull),
        pgpb::ConstrType::ConstrDefault => {
            Ok(ColumnOptionShape::Default(pg_constraint_expr(constraint)?))
        }
        pgpb::ConstrType::ConstrGenerated => Ok(ColumnOptionShape::Generated {
            expr: pg_constraint_expr(constraint)?,
            stored: true,
        }),
        pgpb::ConstrType::ConstrIdentity => Ok(ColumnOptionShape::Identity {
            generation: pg_identity_generation(&constraint.generated_when)?,
            options: constraint
                .options
                .iter()
                .map(pg_identity_option_shape)
                .collect::<Result<_, _>>()?,
        }),
        pgpb::ConstrType::ConstrPrimary => Ok(ColumnOptionShape::PrimaryKey),
        pgpb::ConstrType::ConstrUnique => Ok(ColumnOptionShape::Unique),
        pgpb::ConstrType::ConstrCheck => {
            Ok(ColumnOptionShape::Check(pg_constraint_expr(constraint)?))
        }
        pgpb::ConstrType::ConstrForeign => {
            let table = constraint
                .pktable
                .as_ref()
                .map(pg_range_var_name)
                .ok_or_else(|| "PostgreSQL foreign key has no referenced table".to_string())?;
            Ok(ColumnOptionShape::References {
                table,
                columns: pg_string_list(&constraint.pk_attrs)?,
                actions: pg_foreign_key_actions_shape(constraint)?,
            })
        }
        other => Err(format!(
            "unsupported PostgreSQL column constraint type: {other:?}"
        )),
    }
}

/// Map PostgreSQL's `MATCH` / `ON UPDATE` / `ON DELETE` foreign-key action codes to
/// the neutral shape. PostgreSQL always materializes these (empty string or the
/// default code meaning `MATCH SIMPLE` / `NO ACTION`), so the omitted-clause form
/// and the explicit default both fold to the default shape (ADR-0015). The `SET
/// NULL`/`SET DEFAULT` column list (`fk_del_set_cols`) exists only for `ON DELETE`.
fn pg_foreign_key_actions_shape(
    constraint: &pgpb::Constraint,
) -> Result<ForeignKeyActionsShape, String> {
    Ok(ForeignKeyActionsShape {
        match_type: pg_foreign_key_match_shape(&constraint.fk_matchtype)?,
        on_delete: pg_referential_action_shape(
            &constraint.fk_del_action,
            &constraint.fk_del_set_cols,
        )?,
        on_update: pg_referential_action_shape(&constraint.fk_upd_action, &[])?,
    })
}

fn pg_foreign_key_match_shape(match_type: &str) -> Result<ForeignKeyMatchShape, String> {
    match match_type {
        "" | "s" => Ok(ForeignKeyMatchShape::Simple),
        "f" => Ok(ForeignKeyMatchShape::Full),
        "p" => Ok(ForeignKeyMatchShape::Partial),
        other => Err(format!(
            "unsupported PostgreSQL foreign key MATCH type {other:?}"
        )),
    }
}

fn pg_referential_action_shape(
    action: &str,
    set_cols: &[pgpb::Node],
) -> Result<ReferentialActionShape, String> {
    let columns = || pg_string_list(set_cols);
    match action {
        "" | "a" => Ok(ReferentialActionShape::NoAction),
        "r" => Ok(ReferentialActionShape::Restrict),
        "c" => Ok(ReferentialActionShape::Cascade),
        "n" => Ok(ReferentialActionShape::SetNull {
            columns: columns()?,
        }),
        "d" => Ok(ReferentialActionShape::SetDefault {
            columns: columns()?,
        }),
        other => Err(format!(
            "unsupported PostgreSQL foreign key referential action {other:?}"
        )),
    }
}

fn pg_constraint_expr(constraint: &pgpb::Constraint) -> Result<ExprShape, String> {
    let expr = constraint
        .raw_expr
        .as_deref()
        .ok_or_else(|| "PostgreSQL constraint has no expression".to_string())?;
    pg_expr_shape(expr)
}

fn pg_identity_generation(generated_when: &str) -> Result<IdentityGenerationShape, String> {
    match generated_when {
        "a" => Ok(IdentityGenerationShape::Always),
        "d" => Ok(IdentityGenerationShape::ByDefault),
        other => Err(format!(
            "unsupported PostgreSQL identity generation marker {other:?}"
        )),
    }
}

fn pg_identity_option_shape(node: &pgpb::Node) -> Result<IdentityOptionShape, String> {
    let Some(NodeEnum::DefElem(elem)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL identity option node: {:?}",
            node.node
        ));
    };
    match elem.defname.as_str() {
        "start" => Ok(IdentityOptionShape::StartWith(pg_def_elem_required_value(
            elem,
        )?)),
        "increment" => Ok(IdentityOptionShape::IncrementBy(
            pg_def_elem_required_value(elem)?,
        )),
        "minvalue" => Ok(IdentityOptionShape::MinValue(pg_def_elem_optional_value(
            elem,
        )?)),
        "maxvalue" => Ok(IdentityOptionShape::MaxValue(pg_def_elem_optional_value(
            elem,
        )?)),
        "cache" => Ok(IdentityOptionShape::Cache(pg_def_elem_required_value(
            elem,
        )?)),
        "cycle" => Ok(IdentityOptionShape::Cycle(pg_def_elem_bool(elem)?)),
        other => Err(format!("unsupported PostgreSQL identity option {other:?}")),
    }
}

fn pg_def_elem_required_value(elem: &pgpb::DefElem) -> Result<ExprShape, String> {
    let arg = elem
        .arg
        .as_deref()
        .ok_or_else(|| format!("PostgreSQL option {:?} has no value", elem.defname))?;
    pg_def_elem_value_expr(arg)
}

fn pg_def_elem_optional_value(elem: &pgpb::DefElem) -> Result<Option<ExprShape>, String> {
    elem.arg.as_deref().map(pg_def_elem_value_expr).transpose()
}

fn pg_def_elem_bool(elem: &pgpb::DefElem) -> Result<bool, String> {
    match elem.arg.as_deref().and_then(|node| node.node.as_ref()) {
        Some(NodeEnum::Boolean(value)) => Ok(value.boolval),
        other => Err(format!(
            "expected PostgreSQL boolean option, found {other:?}"
        )),
    }
}

/// Map a `DefElem` value node — `SeqOptElem`/storage-parameter values are bare
/// `Integer`/`Float`/`String` nodes, not the `A_Const`s expressions carry, so this
/// folds them into the literal shape our parsed `Expr` produces.
fn pg_def_elem_value_expr(node: &pgpb::Node) -> Result<ExprShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::Integer(value)) => Ok(ExprShape::Literal(LiteralShape::Integer(
            value.ival.to_string(),
        ))),
        Some(NodeEnum::Float(value)) => {
            Ok(ExprShape::Literal(LiteralShape::Float(value.fval.clone())))
        }
        Some(NodeEnum::String(value)) => {
            Ok(ExprShape::Literal(LiteralShape::String(value.sval.clone())))
        }
        other => Err(format!(
            "unsupported PostgreSQL option value node: {other:?}"
        )),
    }
}

fn pg_table_constraint_shape(
    constraint: &pgpb::Constraint,
) -> Result<TableConstraintShape, String> {
    let name = (!constraint.conname.is_empty()).then(|| constraint.conname.clone());
    let kind = match pgpb::ConstrType::try_from(constraint.contype)
        .unwrap_or(pgpb::ConstrType::Undefined)
    {
        pgpb::ConstrType::ConstrPrimary => {
            TableConstraintKindShape::PrimaryKey(pg_constraint_keys(constraint)?)
        }
        pgpb::ConstrType::ConstrUnique => {
            TableConstraintKindShape::Unique(pg_constraint_keys(constraint)?)
        }
        pgpb::ConstrType::ConstrCheck => {
            TableConstraintKindShape::Check(pg_constraint_expr(constraint)?)
        }
        pgpb::ConstrType::ConstrForeign => TableConstraintKindShape::ForeignKey {
            columns: pg_string_list(&constraint.fk_attrs)?,
            table: constraint
                .pktable
                .as_ref()
                .map(pg_range_var_name)
                .ok_or_else(|| "PostgreSQL foreign key has no referenced table".to_string())?,
            ref_columns: pg_string_list(&constraint.pk_attrs)?,
            actions: pg_foreign_key_actions_shape(constraint)?,
        },
        other => {
            return Err(format!(
                "unsupported PostgreSQL table constraint type: {other:?}"
            ));
        }
    };
    Ok(TableConstraintShape { name, kind })
}

/// Resolve a list of protobuf `String`/`A_Star` nodes to their text — the shared
/// shape of the many `&[pgpb::Node]` ident/name fields the PostgreSQL side carries
/// (column lists, index keys, GROUP BY items, ...). Propagates the first
/// [`pg_string_node`] error.
fn pg_string_list(nodes: &[pgpb::Node]) -> Result<Vec<String>, String> {
    nodes.iter().map(pg_string_node).collect()
}

fn pg_constraint_keys(constraint: &pgpb::Constraint) -> Result<Vec<String>, String> {
    pg_string_list(&constraint.keys)
}

fn pg_create_stmt_options(stmt: &pgpb::CreateStmt) -> Result<TableOptionsShape, String> {
    Ok(TableOptionsShape {
        with_params: stmt
            .options
            .iter()
            .map(pg_storage_param_shape)
            .collect::<Result<_, _>>()?,
        on_commit: pg_on_commit_shape(stmt.oncommit)?,
        tablespace: (!stmt.tablespacename.is_empty()).then(|| stmt.tablespacename.clone()),
    })
}

fn pg_into_clause_options(into: &pgpb::IntoClause) -> Result<TableOptionsShape, String> {
    Ok(TableOptionsShape {
        with_params: into
            .options
            .iter()
            .map(pg_storage_param_shape)
            .collect::<Result<_, _>>()?,
        on_commit: pg_on_commit_shape(into.on_commit)?,
        tablespace: (!into.table_space_name.is_empty()).then(|| into.table_space_name.clone()),
    })
}

fn pg_on_commit_shape(value: i32) -> Result<Option<OnCommitShape>, String> {
    match pgpb::OnCommitAction::try_from(value).unwrap_or(pgpb::OnCommitAction::Undefined) {
        pgpb::OnCommitAction::OncommitNoop | pgpb::OnCommitAction::Undefined => Ok(None),
        pgpb::OnCommitAction::OncommitPreserveRows => Ok(Some(OnCommitShape::PreserveRows)),
        pgpb::OnCommitAction::OncommitDeleteRows => Ok(Some(OnCommitShape::DeleteRows)),
        pgpb::OnCommitAction::OncommitDrop => Ok(Some(OnCommitShape::Drop)),
    }
}

fn pg_storage_param_shape(node: &pgpb::Node) -> Result<StorageParamShape, String> {
    let Some(NodeEnum::DefElem(elem)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL storage parameter node: {:?}",
            node.node
        ));
    };
    let mut name = Vec::with_capacity(2);
    if !elem.defnamespace.is_empty() {
        name.push(elem.defnamespace.clone());
    }
    name.push(elem.defname.clone());
    Ok(StorageParamShape {
        name,
        value: elem
            .arg
            .as_deref()
            .map(pg_def_elem_value_expr)
            .transpose()?,
    })
}

fn pg_alter_table_shape(stmt: &pgpb::AlterTableStmt) -> Result<AlterTableShape, String> {
    if pgpb::ObjectType::try_from(stmt.objtype).unwrap_or(pgpb::ObjectType::Undefined)
        != pgpb::ObjectType::ObjectTable
    {
        return Err("unsupported PostgreSQL ALTER on a non-table object".to_string());
    }
    let relation = stmt
        .relation
        .as_ref()
        .ok_or_else(|| "PostgreSQL ALTER TABLE has no relation".to_string())?;
    Ok(AlterTableShape {
        if_exists: stmt.missing_ok,
        name: pg_range_var_name(relation),
        actions: stmt
            .cmds
            .iter()
            .map(pg_alter_table_action_shape)
            .collect::<Result<_, _>>()?,
    })
}

fn pg_alter_table_action_shape(node: &pgpb::Node) -> Result<AlterTableActionShape, String> {
    let Some(NodeEnum::AlterTableCmd(cmd)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL ALTER TABLE command node: {:?}",
            node.node
        ));
    };
    match pgpb::AlterTableType::try_from(cmd.subtype).unwrap_or(pgpb::AlterTableType::Undefined) {
        pgpb::AlterTableType::AtAddColumn => Ok(AlterTableActionShape::AddColumn {
            if_not_exists: cmd.missing_ok,
            column: pg_alter_column_def(cmd.def.as_deref())?,
        }),
        pgpb::AlterTableType::AtDropColumn => Ok(AlterTableActionShape::DropColumn {
            if_exists: cmd.missing_ok,
            name: cmd.name.clone(),
            cascade: pg_drop_cascade(cmd.behavior),
        }),
        pgpb::AlterTableType::AtAddConstraint => {
            let Some(NodeEnum::Constraint(constraint)) =
                cmd.def.as_deref().and_then(|node| node.node.as_ref())
            else {
                return Err("PostgreSQL ADD CONSTRAINT has no constraint".to_string());
            };
            Ok(AlterTableActionShape::AddConstraint(
                pg_table_constraint_shape(constraint)?,
            ))
        }
        pgpb::AlterTableType::AtDropConstraint => Ok(AlterTableActionShape::DropConstraint {
            if_exists: cmd.missing_ok,
            name: cmd.name.clone(),
            cascade: pg_drop_cascade(cmd.behavior),
        }),
        pgpb::AlterTableType::AtColumnDefault => Ok(AlterTableActionShape::AlterColumn {
            name: cmd.name.clone(),
            change: match cmd.def.as_deref() {
                Some(expr) => AlterColumnActionShape::SetDefault(pg_expr_shape(expr)?),
                None => AlterColumnActionShape::DropDefault,
            },
        }),
        pgpb::AlterTableType::AtSetNotNull => Ok(AlterTableActionShape::AlterColumn {
            name: cmd.name.clone(),
            change: AlterColumnActionShape::SetNotNull,
        }),
        pgpb::AlterTableType::AtDropNotNull => Ok(AlterTableActionShape::AlterColumn {
            name: cmd.name.clone(),
            change: AlterColumnActionShape::DropNotNull,
        }),
        pgpb::AlterTableType::AtAlterColumnType => {
            let (data_type, using) = pg_alter_column_type(cmd.def.as_deref())?;
            Ok(AlterTableActionShape::AlterColumn {
                name: cmd.name.clone(),
                change: AlterColumnActionShape::SetDataType { data_type, using },
            })
        }
        pgpb::AlterTableType::AtSetRelOptions => {
            let Some(NodeEnum::List(options)) =
                cmd.def.as_deref().and_then(|node| node.node.as_ref())
            else {
                return Err("PostgreSQL ALTER TABLE SET has no option list".to_string());
            };
            Ok(AlterTableActionShape::SetOptions(
                options
                    .items
                    .iter()
                    .map(pg_storage_param_shape)
                    .collect::<Result<_, _>>()?,
            ))
        }
        other => Err(format!(
            "unsupported PostgreSQL ALTER TABLE command: {other:?}"
        )),
    }
}

fn pg_alter_column_def(def: Option<&pgpb::Node>) -> Result<TableColumnShape, String> {
    let Some(NodeEnum::ColumnDef(column)) = def.and_then(|node| node.node.as_ref()) else {
        return Err("PostgreSQL ADD COLUMN has no column definition".to_string());
    };
    pg_table_column_shape(column)
}

fn pg_alter_column_type(
    def: Option<&pgpb::Node>,
) -> Result<(DataTypeShape, Option<ExprShape>), String> {
    let Some(NodeEnum::ColumnDef(column)) = def.and_then(|node| node.node.as_ref()) else {
        return Err("PostgreSQL ALTER COLUMN TYPE has no column definition".to_string());
    };
    let type_name = column
        .type_name
        .as_ref()
        .ok_or_else(|| "PostgreSQL ALTER COLUMN TYPE has no type".to_string())?;
    // The `USING` conversion expression rides in `raw_default` on the synthetic
    // ColumnDef PostgreSQL builds for the type change.
    let using = column
        .raw_default
        .as_deref()
        .map(pg_expr_shape)
        .transpose()?;
    Ok((pg_type_name_shape(type_name)?, using))
}

fn pg_drop_shape(stmt: &pgpb::DropStmt) -> Result<DropShape, String> {
    let object_kind =
        match pgpb::ObjectType::try_from(stmt.remove_type).unwrap_or(pgpb::ObjectType::Undefined) {
            pgpb::ObjectType::ObjectTable => DropObjectKindShape::Table,
            pgpb::ObjectType::ObjectView => DropObjectKindShape::View,
            pgpb::ObjectType::ObjectIndex => DropObjectKindShape::Index,
            pgpb::ObjectType::ObjectSchema => DropObjectKindShape::Schema,
            other => {
                return Err(format!(
                    "unsupported PostgreSQL DROP object type: {other:?}"
                ));
            }
        };
    Ok(DropShape {
        object_kind,
        if_exists: stmt.missing_ok,
        names: stmt
            .objects
            .iter()
            .map(pg_drop_name_shape)
            .collect::<Result<_, _>>()?,
        cascade: pg_drop_cascade(stmt.behavior),
    })
}

/// One dropped object name. `DROP TABLE/VIEW/INDEX` wrap the (possibly qualified)
/// name in a `List` of `String`s; `DROP SCHEMA` uses a bare `String` node.
fn pg_drop_name_shape(node: &pgpb::Node) -> Result<Vec<String>, String> {
    match node.node.as_ref() {
        Some(NodeEnum::List(list)) => pg_string_list(&list.items),
        Some(NodeEnum::String(value)) => Ok(vec![value.sval.clone()]),
        other => Err(format!("unsupported PostgreSQL DROP name node: {other:?}")),
    }
}

fn pg_drop_cascade(behavior: i32) -> bool {
    matches!(
        pgpb::DropBehavior::try_from(behavior).unwrap_or(pgpb::DropBehavior::Undefined),
        pgpb::DropBehavior::DropCascade
    )
}

fn pg_truncate_shape(stmt: &pgpb::TruncateStmt) -> Result<TruncateShape, String> {
    Ok(TruncateShape {
        names: stmt
            .relations
            .iter()
            .map(pg_relation_name_shape)
            .collect::<Result<_, _>>()?,
        restart_identity: stmt.restart_seqs,
        cascade: pg_drop_cascade(stmt.behavior),
    })
}

/// One `TRUNCATE` relation: PostgreSQL wraps each table in a `RangeVar` node.
fn pg_relation_name_shape(node: &pgpb::Node) -> Result<Vec<String>, String> {
    match node.node.as_ref() {
        Some(NodeEnum::RangeVar(range)) => Ok(pg_range_var_name(range)),
        other => Err(format!(
            "unsupported PostgreSQL TRUNCATE relation: {other:?}"
        )),
    }
}

fn pg_comment_shape(stmt: &pgpb::CommentStmt) -> Result<CommentOnShape, String> {
    let target =
        match pgpb::ObjectType::try_from(stmt.objtype).unwrap_or(pgpb::ObjectType::Undefined) {
            pgpb::ObjectType::ObjectTable => CommentTargetShape::Table,
            pgpb::ObjectType::ObjectColumn => CommentTargetShape::Column,
            pgpb::ObjectType::ObjectDatabase => CommentTargetShape::Database,
            other => {
                return Err(format!(
                    "unsupported PostgreSQL COMMENT ON object type: {other:?}"
                ));
            }
        };
    let object = stmt
        .object
        .as_deref()
        .ok_or_else(|| "PostgreSQL COMMENT ON has no object".to_string())?;
    Ok(CommentOnShape {
        target,
        // Table/column names lower to a `List`, a bare database name to a `String`;
        // `pg_drop_name_shape` already handles both.
        name: pg_drop_name_shape(object)?,
        // PostgreSQL lowers `IS NULL` to an empty comment string, indistinguishable from
        // `IS ''`; both normalize to `None` (see [`CommentOnShape`]).
        comment: (!stmt.comment.is_empty()).then(|| stmt.comment.clone()),
    })
}

fn pg_create_schema_shape(stmt: &pgpb::CreateSchemaStmt) -> Result<CreateSchemaShape, String> {
    Ok(CreateSchemaShape {
        if_not_exists: stmt.if_not_exists,
        name: (!stmt.schemaname.is_empty()).then(|| vec![stmt.schemaname.clone()]),
        authorization: stmt.authrole.as_ref().map(pg_role_name).transpose()?,
    })
}

fn pg_role_name(role: &pgpb::RoleSpec) -> Result<String, String> {
    match pgpb::RoleSpecType::try_from(role.roletype).unwrap_or(pgpb::RoleSpecType::Undefined) {
        pgpb::RoleSpecType::RolespecCstring => Ok(role.rolename.clone()),
        other => Err(format!(
            "unsupported PostgreSQL role specification: {other:?}"
        )),
    }
}

fn pg_view_stmt_shape(stmt: &pgpb::ViewStmt) -> Result<CreateViewShape, String> {
    if !stmt.options.is_empty() {
        return Err("unsupported PostgreSQL view WITH options".to_string());
    }
    let view = stmt
        .view
        .as_ref()
        .ok_or_else(|| "PostgreSQL CREATE VIEW has no relation".to_string())?;
    Ok(CreateViewShape {
        or_replace: stmt.replace,
        materialized: false,
        temporary: pg_is_temp(&view.relpersistence),
        // Regular `CREATE VIEW` has no `IF NOT EXISTS` spelling in PostgreSQL.
        if_not_exists: false,
        name: pg_range_var_name(view),
        columns: pg_string_list(&stmt.aliases)?,
        query: Box::new(pg_query_shape_from_node(stmt.query.as_deref())?),
        check_option: pg_view_check_option(stmt.with_check_option)?,
        no_data: false,
    })
}

fn pg_view_check_option(value: i32) -> Result<Option<ViewCheckOptionShape>, String> {
    match pgpb::ViewCheckOption::try_from(value).unwrap_or(pgpb::ViewCheckOption::Undefined) {
        pgpb::ViewCheckOption::NoCheckOption | pgpb::ViewCheckOption::Undefined => Ok(None),
        pgpb::ViewCheckOption::LocalCheckOption => Ok(Some(ViewCheckOptionShape::Local)),
        pgpb::ViewCheckOption::CascadedCheckOption => Ok(Some(ViewCheckOptionShape::Cascaded)),
    }
}

fn pg_matview_shape(stmt: &pgpb::CreateTableAsStmt) -> Result<CreateViewShape, String> {
    let into = stmt
        .into
        .as_ref()
        .ok_or_else(|| "PostgreSQL materialized view has no INTO clause".to_string())?;
    let rel = into
        .rel
        .as_ref()
        .ok_or_else(|| "PostgreSQL materialized view has no relation".to_string())?;
    if !into.options.is_empty() || !into.table_space_name.is_empty() {
        return Err("unsupported PostgreSQL materialized view storage options".to_string());
    }
    Ok(CreateViewShape {
        or_replace: false,
        materialized: true,
        temporary: pg_is_temp(&rel.relpersistence),
        if_not_exists: stmt.if_not_exists,
        name: pg_range_var_name(rel),
        columns: pg_string_list(&into.col_names)?,
        query: Box::new(pg_query_shape_from_node(stmt.query.as_deref())?),
        check_option: None,
        no_data: into.skip_data,
    })
}

fn pg_index_stmt_shape(stmt: &pgpb::IndexStmt) -> Result<CreateIndexShape, String> {
    if !stmt.index_including_params.is_empty() {
        return Err("unsupported PostgreSQL index INCLUDE columns".to_string());
    }
    if !stmt.options.is_empty() {
        return Err("unsupported PostgreSQL index WITH options".to_string());
    }
    if stmt.primary || stmt.isconstraint {
        return Err("unsupported PostgreSQL constraint-backing index".to_string());
    }
    if stmt.nulls_not_distinct {
        return Err("unsupported PostgreSQL index NULLS NOT DISTINCT".to_string());
    }
    let relation = stmt
        .relation
        .as_ref()
        .ok_or_else(|| "PostgreSQL CREATE INDEX has no relation".to_string())?;
    Ok(CreateIndexShape {
        unique: stmt.unique,
        concurrently: stmt.concurrent,
        if_not_exists: stmt.if_not_exists,
        name: (!stmt.idxname.is_empty()).then(|| stmt.idxname.clone()),
        table: pg_range_var_name(relation),
        method: pg_normalize_index_method(&stmt.access_method),
        columns: stmt
            .index_params
            .iter()
            .map(pg_index_column_shape)
            .collect::<Result<_, _>>()?,
        predicate: stmt
            .where_clause
            .as_deref()
            .map(pg_expr_shape)
            .transpose()?,
    })
}

fn pg_normalize_index_method(method: &str) -> String {
    if method.is_empty() {
        "btree".to_string()
    } else {
        method.to_ascii_lowercase()
    }
}

fn pg_index_column_shape(node: &pgpb::Node) -> Result<IndexColumnShape, String> {
    let Some(NodeEnum::IndexElem(elem)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL index element node: {:?}",
            node.node
        ));
    };
    if !elem.collation.is_empty() {
        return Err("unsupported PostgreSQL index COLLATE clause".to_string());
    }
    if !elem.opclass.is_empty() {
        return Err("unsupported PostgreSQL index operator class".to_string());
    }
    Ok(IndexColumnShape {
        expr: pg_index_elem_expr(elem)?,
        asc: match pgpb::SortByDir::try_from(elem.ordering).unwrap_or(pgpb::SortByDir::Undefined) {
            pgpb::SortByDir::SortbyDefault | pgpb::SortByDir::Undefined => None,
            pgpb::SortByDir::SortbyAsc => Some(true),
            pgpb::SortByDir::SortbyDesc => Some(false),
            pgpb::SortByDir::SortbyUsing => {
                return Err("unsupported PostgreSQL index USING ordering".to_string());
            }
        },
        nulls_first: match pgpb::SortByNulls::try_from(elem.nulls_ordering)
            .unwrap_or(pgpb::SortByNulls::Undefined)
        {
            pgpb::SortByNulls::SortbyNullsDefault | pgpb::SortByNulls::Undefined => None,
            pgpb::SortByNulls::SortbyNullsFirst => Some(true),
            pgpb::SortByNulls::SortbyNullsLast => Some(false),
        },
    })
}

/// The keyed expression of an index element / conflict-inference element: a bare
/// column name folds to a [`ExprShape::Column`], otherwise the parenthesized index
/// expression is mapped directly.
fn pg_index_elem_expr(elem: &pgpb::IndexElem) -> Result<ExprShape, String> {
    if !elem.name.is_empty() {
        Ok(ExprShape::Column(vec![elem.name.clone()]))
    } else if let Some(expr) = elem.expr.as_deref() {
        pg_expr_shape(expr)
    } else {
        Err("PostgreSQL index element has neither a name nor an expression".to_string())
    }
}

fn pg_insert_shape(stmt: &pgpb::InsertStmt) -> Result<InsertShape, String> {
    let relation = stmt
        .relation
        .as_ref()
        .ok_or_else(|| "PostgreSQL INSERT has no relation".to_string())?;
    Ok(InsertShape {
        with: stmt.with_clause.as_ref().map(pg_with_shape).transpose()?,
        target: InsertTargetShape {
            name: pg_range_var_name(relation),
            alias: relation.alias.as_ref().map(|alias| alias.aliasname.clone()),
            columns: stmt
                .cols
                .iter()
                .map(pg_insert_column_name)
                .collect::<Result<_, _>>()?,
        },
        overriding: pg_overriding_shape(stmt.r#override),
        source: pg_insert_source_shape(stmt.select_stmt.as_deref())?,
        on_conflict: stmt
            .on_conflict_clause
            .as_deref()
            .map(pg_on_conflict_shape)
            .transpose()?,
        returning: pg_select_items(&stmt.returning_list)?,
    })
}

fn pg_insert_column_name(node: &pgpb::Node) -> Result<String, String> {
    let res = pg_res_target(node)?;
    if !res.indirection.is_empty() {
        return Err("unsupported PostgreSQL INSERT column indirection".to_string());
    }
    Ok(res.name.clone())
}

fn pg_overriding_shape(value: i32) -> Option<InsertOverriding> {
    match pgpb::OverridingKind::try_from(value).unwrap_or(pgpb::OverridingKind::Undefined) {
        pgpb::OverridingKind::OverridingSystemValue => Some(InsertOverriding::SystemValue),
        pgpb::OverridingKind::OverridingUserValue => Some(InsertOverriding::UserValue),
        pgpb::OverridingKind::OverridingNotSet | pgpb::OverridingKind::Undefined => None,
    }
}

/// Map an `INSERT` data source. A bare `VALUES` list parses to a `SelectStmt` with
/// `values_lists` and nothing else; anything carrying a target list, set operation,
/// `WITH`/`ORDER BY`/`LIMIT`, or `FROM` is a query source.
fn pg_insert_source_shape(select_stmt: Option<&pgpb::Node>) -> Result<InsertSourceShape, String> {
    let Some(node) = select_stmt else {
        return Ok(InsertSourceShape::DefaultValues);
    };
    let select = match node.node.as_ref() {
        Some(NodeEnum::SelectStmt(select)) => select,
        other => {
            return Err(format!(
                "unsupported PostgreSQL INSERT source node: {other:?}"
            ));
        }
    };
    let is_plain_values = !select.values_lists.is_empty()
        && matches!(
            pgpb::SetOperation::try_from(select.op).unwrap_or(pgpb::SetOperation::Undefined),
            pgpb::SetOperation::SetopNone
        )
        && select.target_list.is_empty()
        && select.from_clause.is_empty()
        && select.sort_clause.is_empty()
        && select.limit_count.is_none()
        && select.limit_offset.is_none()
        && select.with_clause.is_none();
    if is_plain_values {
        Ok(InsertSourceShape::Values(
            select
                .values_lists
                .iter()
                .map(pg_insert_values_row_shape)
                .collect::<Result<_, _>>()?,
        ))
    } else {
        Ok(InsertSourceShape::Query(Box::new(pg_query_shape(select)?)))
    }
}

fn pg_insert_values_row_shape(node: &pgpb::Node) -> Result<Vec<InsertItemShape>, String> {
    match node.node.as_ref() {
        Some(NodeEnum::List(row)) => row.items.iter().map(pg_insert_item_shape).collect(),
        other => Err(format!(
            "unsupported PostgreSQL INSERT VALUES row: {other:?}"
        )),
    }
}

fn pg_insert_item_shape(node: &pgpb::Node) -> Result<InsertItemShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::SetToDefault(_)) => Ok(InsertItemShape::Default),
        _ => Ok(InsertItemShape::Expr(pg_expr_shape(node)?)),
    }
}

fn pg_on_conflict_shape(clause: &pgpb::OnConflictClause) -> Result<OnConflictShape, String> {
    let target = clause
        .infer
        .as_deref()
        .map(pg_conflict_target_shape)
        .transpose()?;
    let action = match pgpb::OnConflictAction::try_from(clause.action)
        .unwrap_or(pgpb::OnConflictAction::Undefined)
    {
        pgpb::OnConflictAction::OnconflictNothing => {
            if !clause.target_list.is_empty() || clause.where_clause.is_some() {
                return Err("PostgreSQL ON CONFLICT DO NOTHING carries an update body".to_string());
            }
            ConflictActionShape::Nothing
        }
        pgpb::OnConflictAction::OnconflictUpdate => ConflictActionShape::Update {
            assignments: pg_update_assignments(&clause.target_list)?,
            selection: clause
                .where_clause
                .as_deref()
                .map(pg_expr_shape)
                .transpose()?,
        },
        other => {
            return Err(format!(
                "unsupported PostgreSQL ON CONFLICT action: {other:?}"
            ));
        }
    };
    Ok(OnConflictShape { target, action })
}

fn pg_conflict_target_shape(infer: &pgpb::InferClause) -> Result<ConflictTargetShape, String> {
    if !infer.conname.is_empty() {
        return Ok(ConflictTargetShape::Constraint(infer.conname.clone()));
    }
    Ok(ConflictTargetShape::Index {
        columns: infer
            .index_elems
            .iter()
            .map(pg_conflict_index_elem)
            .collect::<Result<_, _>>()?,
        predicate: infer
            .where_clause
            .as_deref()
            .map(pg_expr_shape)
            .transpose()?,
    })
}

fn pg_conflict_index_elem(node: &pgpb::Node) -> Result<ExprShape, String> {
    let Some(NodeEnum::IndexElem(elem)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL conflict-inference element node: {:?}",
            node.node
        ));
    };
    pg_index_elem_expr(elem)
}

fn pg_update_shape(stmt: &pgpb::UpdateStmt) -> Result<UpdateShape, String> {
    Ok(UpdateShape {
        with: stmt.with_clause.as_ref().map(pg_with_shape).transpose()?,
        target: pg_dml_target_shape(stmt.relation.as_ref())?,
        assignments: pg_update_assignments(&stmt.target_list)?,
        from: pg_from_shapes(&stmt.from_clause)?,
        selection: pg_dml_selection_shape(stmt.where_clause.as_deref())?,
        returning: pg_select_items(&stmt.returning_list)?,
    })
}

fn pg_delete_shape(stmt: &pgpb::DeleteStmt) -> Result<DeleteShape, String> {
    Ok(DeleteShape {
        with: stmt.with_clause.as_ref().map(pg_with_shape).transpose()?,
        target: pg_dml_target_shape(stmt.relation.as_ref())?,
        using: pg_from_shapes(&stmt.using_clause)?,
        selection: pg_dml_selection_shape(stmt.where_clause.as_deref())?,
        returning: pg_select_items(&stmt.returning_list)?,
    })
}

fn pg_dml_target_shape(relation: Option<&pgpb::RangeVar>) -> Result<DmlTargetShape, String> {
    let relation =
        relation.ok_or_else(|| "PostgreSQL DML statement has no target relation".to_string())?;
    Ok(DmlTargetShape {
        name: pg_range_var_name(relation),
        only: !relation.inh,
        alias: relation.alias.as_ref().map(|alias| alias.aliasname.clone()),
    })
}

/// Re-group PostgreSQL's flattened `SET` target list. A single-column assignment is
/// one `ResTarget` with a plain value; a multiple-column `(a, b) = source` is `n`
/// consecutive `ResTarget`s whose values are `MultiAssignRef`s sharing one source
/// (`colno` 1..=`ncolumns`), which this collapses back into one [`Tuple`](UpdateAssignmentShape::Tuple).
fn pg_update_assignments(target_list: &[pgpb::Node]) -> Result<Vec<UpdateAssignmentShape>, String> {
    let mut assignments = Vec::new();
    let mut index = 0;
    while index < target_list.len() {
        let res = pg_res_target(&target_list[index])?;
        if !res.indirection.is_empty() {
            return Err("unsupported PostgreSQL UPDATE target indirection".to_string());
        }
        let value = res
            .val
            .as_deref()
            .ok_or_else(|| "PostgreSQL UPDATE target has no value".to_string())?;
        match value.node.as_ref() {
            Some(NodeEnum::MultiAssignRef(first)) => {
                let ncolumns = usize::try_from(first.ncolumns)
                    .map_err(|_| "PostgreSQL multi-assign column count is negative".to_string())?;
                if ncolumns == 0 {
                    return Err("PostgreSQL multi-assign has no columns".to_string());
                }
                let mut targets = Vec::with_capacity(ncolumns);
                for offset in 0..ncolumns {
                    let res = pg_res_target(target_list.get(index + offset).ok_or_else(|| {
                        "PostgreSQL multi-assign group is truncated".to_string()
                    })?)?;
                    if !res.indirection.is_empty() {
                        return Err("unsupported PostgreSQL UPDATE target indirection".to_string());
                    }
                    let Some(NodeEnum::MultiAssignRef(mar)) =
                        res.val.as_deref().and_then(|node| node.node.as_ref())
                    else {
                        return Err("PostgreSQL multi-assign group is not contiguous".to_string());
                    };
                    if mar.colno as usize != offset + 1 || mar.ncolumns as usize != ncolumns {
                        return Err("PostgreSQL multi-assign indices are inconsistent".to_string());
                    }
                    targets.push(vec![res.name.clone()]);
                }
                assignments.push(UpdateAssignmentShape::Tuple {
                    targets,
                    source: pg_update_tuple_source_shape(first.source.as_deref())?,
                });
                index += ncolumns;
            }
            _ => {
                assignments.push(UpdateAssignmentShape::Single {
                    target: vec![res.name.clone()],
                    value: pg_update_value_shape(value)?,
                });
                index += 1;
            }
        }
    }
    Ok(assignments)
}

fn pg_update_tuple_source_shape(
    source: Option<&pgpb::Node>,
) -> Result<UpdateTupleSourceShape, String> {
    let source =
        source.ok_or_else(|| "PostgreSQL multi-assign reference has no source".to_string())?;
    match source.node.as_ref() {
        Some(NodeEnum::RowExpr(row)) => Ok(UpdateTupleSourceShape::Row {
            explicit: matches!(
                pgpb::CoercionForm::try_from(row.row_format)
                    .unwrap_or(pgpb::CoercionForm::Undefined),
                pgpb::CoercionForm::CoerceExplicitCall
            ),
            values: row
                .args
                .iter()
                .map(pg_update_value_shape)
                .collect::<Result<_, _>>()?,
        }),
        Some(NodeEnum::SubLink(sublink)) => {
            if !matches!(
                pgpb::SubLinkType::try_from(sublink.sub_link_type)
                    .unwrap_or(pgpb::SubLinkType::Undefined),
                pgpb::SubLinkType::ExprSublink
            ) {
                return Err("unsupported PostgreSQL multi-assign subquery kind".to_string());
            }
            let subquery = sublink
                .subselect
                .as_deref()
                .ok_or_else(|| "PostgreSQL multi-assign subquery has no select".to_string())?;
            Ok(UpdateTupleSourceShape::Subquery(Box::new(
                pg_query_shape_from_node(Some(subquery))?,
            )))
        }
        Some(NodeEnum::SetToDefault(_)) => Ok(UpdateTupleSourceShape::Default),
        other => Err(format!(
            "unsupported PostgreSQL multi-assign source node: {other:?}"
        )),
    }
}

fn pg_update_value_shape(node: &pgpb::Node) -> Result<UpdateValueShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::SetToDefault(_)) => Ok(UpdateValueShape::Default),
        _ => Ok(UpdateValueShape::Expr(pg_expr_shape(node)?)),
    }
}

fn pg_dml_selection_shape(
    where_clause: Option<&pgpb::Node>,
) -> Result<Option<DmlSelectionShape>, String> {
    let Some(node) = where_clause else {
        return Ok(None);
    };
    match node.node.as_ref() {
        Some(NodeEnum::CurrentOfExpr(current)) => Ok(Some(DmlSelectionShape::CurrentOf(
            current.cursor_name.clone(),
        ))),
        _ => Ok(Some(DmlSelectionShape::Where(pg_expr_shape(node)?))),
    }
}

fn pg_res_target(node: &pgpb::Node) -> Result<&pgpb::ResTarget, String> {
    match node.node.as_ref() {
        Some(NodeEnum::ResTarget(res)) => Ok(res),
        other => Err(format!(
            "expected PostgreSQL result target, found {other:?}"
        )),
    }
}

/// `'t'` (temporary) is one of the two modelled relpersistence marks; `'p'`
/// (permanent) and `'u'` (unlogged) are not temporary.
fn pg_is_temp(relpersistence: &str) -> bool {
    relpersistence == "t"
}

/// `'u'` marks a `CREATE UNLOGGED TABLE` relation (a peer of the temporary `'t'` —
/// PostgreSQL's `OptTemp` makes the two mutually exclusive).
fn pg_is_unlogged(relpersistence: &str) -> bool {
    relpersistence == "u"
}

/// An optional protobuf string field: PostgreSQL reports an unwritten clause as an empty
/// string, mapped to `None` so the shape compares presence, not emptiness.
fn pg_nonempty_string(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn pg_query_shape_from_node(node: Option<&pgpb::Node>) -> Result<QueryShape, String> {
    let node = node.ok_or_else(|| "PostgreSQL statement has no query".to_string())?;
    match node.node.as_ref() {
        Some(NodeEnum::SelectStmt(select)) => pg_query_shape(select),
        other => Err(format!("unsupported PostgreSQL query node: {other:?}")),
    }
}

fn pg_query_shape(select: &pgpb::SelectStmt) -> Result<QueryShape, String> {
    let limit_option =
        pgpb::LimitOption::try_from(select.limit_option).unwrap_or(pgpb::LimitOption::Undefined);

    Ok(QueryShape {
        with: select.with_clause.as_ref().map(pg_with_shape).transpose()?,
        body: pg_set_shape(select)?,
        order_by: pg_order_by_shapes(&select.sort_clause)?,
        // PostgreSQL's grammar has no `ORDER BY ALL` mode, so its shape never carries one.
        order_by_all: None,
        limit: LimitShape {
            count: select
                .limit_count
                .as_deref()
                .map(pg_expr_shape)
                .transpose()?,
            offset: select
                .limit_offset
                .as_deref()
                .map(pg_expr_shape)
                .transpose()?,
            with_ties: matches!(limit_option, pgpb::LimitOption::WithTies),
        },
        locking: pg_locking_clause_shapes(&select.locking_clause)?,
    })
}

/// Map PostgreSQL's `SelectStmt.locking_clause` list to the neutral shapes.
fn pg_locking_clause_shapes(nodes: &[pgpb::Node]) -> Result<Vec<LockingClauseShape>, String> {
    nodes.iter().map(pg_locking_clause_shape).collect()
}

fn pg_locking_clause_shape(node: &pgpb::Node) -> Result<LockingClauseShape, String> {
    let Some(NodeEnum::LockingClause(clause)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL locking clause node: {node:?}"
        ));
    };
    let strength = match pgpb::LockClauseStrength::try_from(clause.strength)
        .unwrap_or(pgpb::LockClauseStrength::Undefined)
    {
        pgpb::LockClauseStrength::LcsForupdate => LockStrengthShape::Update,
        pgpb::LockClauseStrength::LcsFornokeyupdate => LockStrengthShape::NoKeyUpdate,
        pgpb::LockClauseStrength::LcsForshare => LockStrengthShape::Share,
        pgpb::LockClauseStrength::LcsForkeyshare => LockStrengthShape::KeyShare,
        other => return Err(format!("unsupported PostgreSQL lock strength: {other:?}")),
    };
    let wait = match pgpb::LockWaitPolicy::try_from(clause.wait_policy)
        .unwrap_or(pgpb::LockWaitPolicy::Undefined)
    {
        // `LockWaitBlock` is the default (no `NOWAIT`/`SKIP LOCKED`), so it maps to
        // `None` — the same "wrote no tail" state our parser records.
        pgpb::LockWaitPolicy::LockWaitBlock | pgpb::LockWaitPolicy::Undefined => None,
        pgpb::LockWaitPolicy::LockWaitSkip => Some(LockWaitShape::SkipLocked),
        pgpb::LockWaitPolicy::LockWaitError => Some(LockWaitShape::NoWait),
    };
    // Each `OF` target is a `RangeVar` relation reference; map it to its name parts.
    let of = clause
        .locked_rels
        .iter()
        .map(pg_locked_rel_name)
        .collect::<Result<_, _>>()?;
    Ok(LockingClauseShape { strength, of, wait })
}

/// Extract the object-name parts of one `OF` target (a `RangeVar`).
fn pg_locked_rel_name(node: &pgpb::Node) -> Result<Vec<String>, String> {
    match node.node.as_ref() {
        Some(NodeEnum::RangeVar(range)) => Ok(pg_range_var_name(range)),
        other => Err(format!("unsupported PostgreSQL OF target: {other:?}")),
    }
}

fn pg_set_shape(select: &pgpb::SelectStmt) -> Result<SetShape, String> {
    match pgpb::SetOperation::try_from(select.op).unwrap_or(pgpb::SetOperation::Undefined) {
        pgpb::SetOperation::SetopNone | pgpb::SetOperation::Undefined => {
            if !select.values_lists.is_empty() {
                Ok(SetShape::Values(pg_values_shape(&select.values_lists)?))
            } else {
                Ok(SetShape::Select(pg_select_shape(select)?))
            }
        }
        op => Ok(SetShape::SetOperation {
            op: match op {
                pgpb::SetOperation::SetopUnion => SetOpShape::Union,
                pgpb::SetOperation::SetopIntersect => SetOpShape::Intersect,
                pgpb::SetOperation::SetopExcept => SetOpShape::Except,
                pgpb::SetOperation::SetopNone | pgpb::SetOperation::Undefined => unreachable!(),
            },
            all: select.all,
            // PostgreSQL has no name-matched set operation; every PG set op is positional.
            by_name: false,
            left: Box::new(pg_set_operand_shape(
                select
                    .larg
                    .as_deref()
                    .expect("PostgreSQL set operation has a left operand"),
            )?),
            right: Box::new(pg_set_operand_shape(
                select
                    .rarg
                    .as_deref()
                    .expect("PostgreSQL set operation has a right operand"),
            )?),
        }),
    }
}

/// Shape of one operand (`larg`/`rarg`) of a PostgreSQL set operation.
///
/// PostgreSQL has no parenthesized-grouping node, so a parenthesized set operand that
/// carries its own `WITH`/`ORDER BY`/`LIMIT` lands those clauses directly on the
/// operand's `SelectStmt` (the top-level query captures its own via [`pg_query_shape`]
/// instead, so this is only reached below the top). Wrap such an operand in
/// [`SetShape::Query`] so the comparison keeps the clauses — mirroring `set_shape`'s
/// `SetExpr::Query` arm — while a clause-free operand stays the flat set shape. A
/// `locking_clause` is one of those load-bearing tails ([`QueryShape`] now models it),
/// so an operand carrying it is wrapped too.
fn pg_set_operand_shape(select: &pgpb::SelectStmt) -> Result<SetShape, String> {
    if select.with_clause.is_some()
        || !select.sort_clause.is_empty()
        || select.limit_count.is_some()
        || select.limit_offset.is_some()
        || !select.locking_clause.is_empty()
    {
        Ok(SetShape::Query(Box::new(pg_query_shape(select)?)))
    } else {
        pg_set_shape(select)
    }
}

fn pg_with_shape(with: &pgpb::WithClause) -> Result<WithShape, String> {
    Ok(WithShape {
        recursive: with.recursive,
        ctes: with
            .ctes
            .iter()
            .map(pg_cte_shape)
            .collect::<Result<_, _>>()?,
    })
}

fn pg_cte_shape(node: &pgpb::Node) -> Result<CteShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::CommonTableExpr(cte)) => {
            if cte.search_clause.is_some() {
                return Err("unsupported PostgreSQL CTE SEARCH clause".to_string());
            }
            if cte.cycle_clause.is_some() {
                return Err("unsupported PostgreSQL CTE CYCLE clause".to_string());
            }
            let query = cte
                .ctequery
                .as_deref()
                .ok_or_else(|| "PostgreSQL CommonTableExpr has no query".to_string())?;
            let body = match query.node.as_ref() {
                Some(NodeEnum::SelectStmt(select)) => {
                    CteBodyShape::Query(Box::new(pg_query_shape(select)?))
                }
                // PostgreSQL's data-modifying CTE bodies (`PreparableStmt`), mapped
                // through the same DML shape fns as the top-level statements.
                Some(NodeEnum::InsertStmt(insert)) => {
                    CteBodyShape::Insert(Box::new(pg_insert_shape(insert)?))
                }
                Some(NodeEnum::UpdateStmt(update)) => {
                    CteBodyShape::Update(Box::new(pg_update_shape(update)?))
                }
                Some(NodeEnum::DeleteStmt(delete)) => {
                    CteBodyShape::Delete(Box::new(pg_delete_shape(delete)?))
                }
                // `MergeStmt` stays outside the structural subset, exactly as the
                // top-level statement mapper treats it.
                other => return Err(format!("unsupported PostgreSQL CTE query node: {other:?}")),
            };
            Ok(CteShape {
                name: cte.ctename.clone(),
                columns: pg_string_list(&cte.aliascolnames)?,
                materialized: pg_cte_materialization(cte)?,
                body,
            })
        }
        other => Err(format!("unsupported PostgreSQL WITH item: {other:?}")),
    }
}

fn pg_cte_materialization(cte: &pgpb::CommonTableExpr) -> Result<Option<bool>, String> {
    match pgpb::CteMaterialize::try_from(cte.ctematerialized)
        .unwrap_or(pgpb::CteMaterialize::CtematerializeUndefined)
    {
        pgpb::CteMaterialize::CtematerializeUndefined | pgpb::CteMaterialize::Default => Ok(None),
        pgpb::CteMaterialize::Always => Ok(Some(true)),
        pgpb::CteMaterialize::Never => Ok(Some(false)),
    }
}

fn pg_values_shape(nodes: &[pgpb::Node]) -> Result<Vec<Vec<ValuesItemShape>>, String> {
    nodes.iter().map(pg_values_row_shape).collect()
}

fn pg_values_row_shape(node: &pgpb::Node) -> Result<Vec<ValuesItemShape>, String> {
    match node.node.as_ref() {
        Some(NodeEnum::List(row)) => row.items.iter().map(pg_values_item_shape).collect(),
        other => Err(format!("unsupported PostgreSQL VALUES row: {other:?}")),
    }
}

/// Map a `VALUES` row element. PostgreSQL parses a bare `DEFAULT` to `SetToDefault`
/// (mirroring [`pg_insert_item_shape`]); every other node is an expression.
fn pg_values_item_shape(node: &pgpb::Node) -> Result<ValuesItemShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::SetToDefault(_)) => Ok(ValuesItemShape::Default),
        _ => Ok(ValuesItemShape::Expr(pg_expr_shape(node)?)),
    }
}

fn pg_select_shape(select: &pgpb::SelectStmt) -> Result<SelectShape, String> {
    if select.into_clause.is_some() {
        return Err("unsupported PostgreSQL SELECT INTO clause".to_string());
    }
    if !select.window_clause.is_empty() {
        return Err("unsupported PostgreSQL window clause".to_string());
    }
    // A `locking_clause` is a query-level tail captured by [`pg_query_shape`] on the
    // `QueryShape`, so the SELECT body itself does not reject it (mirrors our side,
    // where `Query::locking` rides the query, not the `Select` body).

    Ok(SelectShape {
        distinct: !select.distinct_clause.is_empty(),
        projection: pg_select_items(&select.target_list)?,
        from: pg_from_shapes(&select.from_clause)?,
        selection: select
            .where_clause
            .as_deref()
            .map(pg_expr_shape)
            .transpose()?,
        group_by: select
            .group_clause
            .iter()
            .map(pg_group_by_item_shape)
            .collect::<Result<_, _>>()?,
        // PostgreSQL's `GROUP BY DISTINCT` grouping-set quantifier; the raw parse tree
        // carries it as a bool (explicit `ALL` and an unwritten quantifier both map false).
        group_by_distinct: select.group_distinct,
        // PostgreSQL's grammar has no `GROUP BY ALL` mode, so its shape never carries one.
        group_by_all: false,
        having: select
            .having_clause
            .as_deref()
            .map(pg_expr_shape)
            .transpose()?,
        // PostgreSQL's grammar has no QUALIFY clause, so its shape never carries one.
        qualify: None,
    })
}

/// Map one PostgreSQL `group_clause` node to a neutral GROUP BY item shape.
///
/// A plain grouping expression stays a node (not wrapped); only `ROLLUP`/`CUBE`/
/// `GROUPING SETS`/`()` are `GroupingSet` nodes — the exact distinction squonk
/// now makes, so a mis-parse of a construct as a function call diverges here.
fn pg_group_by_item_shape(node: &pgpb::Node) -> Result<GroupByItemShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::GroupingSet(grouping)) => pg_grouping_set_shape(grouping),
        _ => Ok(GroupByItemShape::Expr(pg_expr_shape(node)?)),
    }
}

fn pg_grouping_set_shape(grouping: &pgpb::GroupingSet) -> Result<GroupByItemShape, String> {
    // `ROLLUP`/`CUBE` carry an expression list, while `GROUPING SETS` nests a
    // `group_by_list` whose members are themselves GROUP BY items (recursively).
    let exprs =
        |content: &[pgpb::Node]| content.iter().map(pg_expr_shape).collect::<Result<_, _>>();
    match pgpb::GroupingSetKind::try_from(grouping.kind).unwrap_or(pgpb::GroupingSetKind::Undefined)
    {
        pgpb::GroupingSetKind::GroupingSetEmpty => Ok(GroupByItemShape::Empty),
        pgpb::GroupingSetKind::GroupingSetRollup => {
            Ok(GroupByItemShape::Rollup(exprs(&grouping.content)?))
        }
        pgpb::GroupingSetKind::GroupingSetCube => {
            Ok(GroupByItemShape::Cube(exprs(&grouping.content)?))
        }
        pgpb::GroupingSetKind::GroupingSetSets => Ok(GroupByItemShape::GroupingSets(
            grouping
                .content
                .iter()
                .map(pg_group_by_item_shape)
                .collect::<Result<_, _>>()?,
        )),
        // `GROUPING_SET_SIMPLE` / `UNDEFINED` are parse-analysis artifacts, not raw
        // grammar output, so the raw parse tree never carries them here.
        other => Err(format!(
            "unsupported PostgreSQL grouping set kind: {other:?}"
        )),
    }
}

fn pg_select_items(nodes: &[pgpb::Node]) -> Result<Vec<SelectItemShape>, String> {
    nodes.iter().map(pg_select_item_shape).collect()
}

fn pg_select_item_shape(node: &pgpb::Node) -> Result<SelectItemShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::ResTarget(target)) => {
            if !target.indirection.is_empty() {
                return Err("unsupported PostgreSQL projection indirection".to_string());
            }
            let val = target
                .val
                .as_deref()
                .ok_or_else(|| "PostgreSQL ResTarget has no value".to_string())?;
            if let Some(wildcard) = pg_projection_wildcard(val)? {
                return Ok(wildcard);
            }
            Ok(SelectItemShape::Expr {
                expr: pg_expr_shape(val)?,
                alias: (!target.name.is_empty()).then(|| target.name.clone()),
            })
        }
        other => Err(format!("unsupported PostgreSQL select target: {other:?}")),
    }
}

fn pg_projection_wildcard(node: &pgpb::Node) -> Result<Option<SelectItemShape>, String> {
    let Some(NodeEnum::ColumnRef(column)) = node.node.as_ref() else {
        return Ok(None);
    };
    let Some(last) = column.fields.last() else {
        return Ok(None);
    };
    if !matches!(last.node.as_ref(), Some(NodeEnum::AStar(_))) {
        return Ok(None);
    }
    if column.fields.len() == 1 {
        return Ok(Some(SelectItemShape::Wildcard));
    }
    let qualifier = pg_string_list(&column.fields[..column.fields.len() - 1])?;
    Ok(Some(SelectItemShape::QualifiedWildcard(qualifier)))
}

fn pg_from_shapes(nodes: &[pgpb::Node]) -> Result<Vec<TableWithJoinsShape>, String> {
    nodes.iter().map(pg_table_with_joins_shape).collect()
}

fn pg_table_with_joins_shape(node: &pgpb::Node) -> Result<TableWithJoinsShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::JoinExpr(join)) => {
            if join.alias.is_some() {
                Ok(TableWithJoinsShape {
                    relation: TableFactorShape::NestedJoin {
                        table: Box::new(pg_join_expr_table_with_joins_shape(join)?),
                        alias: join.alias.as_ref().map(pg_alias_shape).transpose()?,
                    },
                    joins: Vec::new(),
                })
            } else {
                pg_join_expr_table_with_joins_shape(join)
            }
        }
        _ => Ok(TableWithJoinsShape {
            relation: pg_table_factor_shape(node)?,
            joins: Vec::new(),
        }),
    }
}

fn pg_join_expr_table_with_joins_shape(
    join: &pgpb::JoinExpr,
) -> Result<TableWithJoinsShape, String> {
    let left = join
        .larg
        .as_deref()
        .ok_or_else(|| "PostgreSQL JoinExpr has no left relation".to_string())?;
    let right = join
        .rarg
        .as_deref()
        .ok_or_else(|| "PostgreSQL JoinExpr has no right relation".to_string())?;
    let mut table = pg_table_with_joins_shape(left)?;
    table.joins.push(JoinShape {
        relation: pg_table_factor_shape(right)?,
        operator: pg_join_operator_shape(join)?,
    });
    Ok(table)
}

fn pg_table_factor_shape(node: &pgpb::Node) -> Result<TableFactorShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::RangeVar(range)) => Ok(TableFactorShape::Table {
            name: pg_range_var_name(range),
            alias: range.alias.as_ref().map(pg_alias_shape).transpose()?,
            only: !range.inh,
            sample: None,
        }),
        Some(NodeEnum::RangeSubselect(subselect)) => {
            let query = subselect
                .subquery
                .as_deref()
                .ok_or_else(|| "PostgreSQL RangeSubselect has no subquery".to_string())?;
            match query.node.as_ref() {
                Some(NodeEnum::SelectStmt(select)) => Ok(TableFactorShape::Derived {
                    lateral: subselect.lateral,
                    subquery: Box::new(pg_query_shape(select)?),
                    alias: subselect.alias.as_ref().map(pg_alias_shape).transpose()?,
                }),
                other => Err(format!("unsupported PostgreSQL subquery node: {other:?}")),
            }
        }
        Some(NodeEnum::RangeFunction(function)) => pg_range_function_shape(function),
        Some(NodeEnum::RangeTableSample(sample)) => pg_range_table_sample_shape(sample),
        Some(NodeEnum::JoinExpr(join)) => Ok(TableFactorShape::NestedJoin {
            table: Box::new(pg_join_expr_table_with_joins_shape(join)?),
            alias: join.alias.as_ref().map(pg_alias_shape).transpose()?,
        }),
        other => Err(format!("unsupported PostgreSQL FROM item: {other:?}")),
    }
}

fn pg_range_function_shape(function: &pgpb::RangeFunction) -> Result<TableFactorShape, String> {
    let alias = function.alias.as_ref().map(pg_alias_shape).transpose()?;
    if function.is_rowsfrom {
        // A function-level column definition list on `ROWS FROM` is a degenerate
        // form PostgreSQL rejects at analysis; the per-item lists carry the types,
        // so a function-level list here would have no sound shape to compare.
        if !function.coldeflist.is_empty() {
            return Err(
                "unsupported PostgreSQL function-level column definition list on ROWS FROM"
                    .to_string(),
            );
        }
        let functions = function
            .functions
            .iter()
            .map(pg_rows_from_item_shape)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TableFactorShape::RowsFrom {
            lateral: function.lateral,
            functions,
            with_ordinality: function.ordinality,
            alias,
        })
    } else {
        let [item] = function.functions.as_slice() else {
            return Err("PostgreSQL non-ROWS FROM RangeFunction is not singular".to_string());
        };
        // A plain function never carries a per-item column definition list (only the
        // function-level `func_alias_clause` one), so an item-level list here is an
        // unexpected shape rather than something to silently fold into the factor.
        let item = pg_rows_from_item_shape(item)?;
        if !item.column_defs.is_empty() {
            return Err(
                "unsupported PostgreSQL per-item column definition list on a plain table function"
                    .to_string(),
            );
        }
        let column_defs = function
            .coldeflist
            .iter()
            .map(pg_column_def_shape)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TableFactorShape::Function {
            lateral: function.lateral,
            function: item.function,
            with_ordinality: function.ordinality,
            alias,
            column_defs,
        })
    }
}

fn pg_rows_from_item_shape(node: &pgpb::Node) -> Result<RowsFromItemShape, String> {
    let Some(NodeEnum::List(list)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL table function list node: {:?}",
            node.node
        ));
    };
    let [function, column_defs] = list.items.as_slice() else {
        return Err("PostgreSQL table function item is not [function, coldefs]".to_string());
    };
    let column_defs = pg_column_def_list_node_shape(column_defs)?;
    match function.node.as_ref() {
        Some(NodeEnum::FuncCall(call)) => Ok(RowsFromItemShape {
            function: pg_func_call_shape(call)?,
            column_defs,
        }),
        other => Err(format!(
            "unsupported PostgreSQL table function node: {other:?}"
        )),
    }
}

/// Map a PostgreSQL per-function column definition list node — the second element
/// of a `ROWS FROM` item, a `List` of `ColumnDef`s, or a NIL node when the item
/// has no typed columns.
fn pg_column_def_list_node_shape(node: &pgpb::Node) -> Result<Vec<ColumnDefShape>, String> {
    match node.node.as_ref() {
        None => Ok(Vec::new()),
        Some(NodeEnum::List(list)) => list.items.iter().map(pg_column_def_shape).collect(),
        other => Err(format!(
            "unsupported PostgreSQL per-function column definition list node: {other:?}"
        )),
    }
}

/// Map one PostgreSQL `ColumnDef` produced by a `TableFuncElement` (`name type`).
/// A collation clause is rejected rather than dropped: the neutral shape models
/// only the name and type, so silently ignoring `COLLATE` would be an unsound
/// structural match.
fn pg_column_def_shape(node: &pgpb::Node) -> Result<ColumnDefShape, String> {
    let Some(NodeEnum::ColumnDef(column)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL table function column definition node: {:?}",
            node.node
        ));
    };
    if column.coll_clause.is_some() {
        return Err(
            "unsupported PostgreSQL collation in a table function column definition".to_string(),
        );
    }
    let type_name = column
        .type_name
        .as_ref()
        .ok_or_else(|| "PostgreSQL table function column definition has no type".to_string())?;
    Ok(ColumnDefShape {
        name: column.colname.clone(),
        data_type: pg_type_name_shape(type_name)?,
    })
}

fn pg_func_call_shape(call: &pgpb::FuncCall) -> Result<FunctionShape, String> {
    if !call.agg_order.is_empty()
        || call.agg_filter.is_some()
        || call.over.is_some()
        || call.agg_within_group
        || call.agg_distinct
        || call.func_variadic
    {
        return Err("unsupported PostgreSQL table function modifier".to_string());
    }
    Ok(FunctionShape {
        name: pg_string_list(&call.funcname)?,
        args: call
            .args
            .iter()
            .map(pg_expr_shape)
            .collect::<Result<_, _>>()?,
        wildcard: call.agg_star,
    })
}

fn pg_range_table_sample_shape(
    sample: &pgpb::RangeTableSample,
) -> Result<TableFactorShape, String> {
    let relation = sample
        .relation
        .as_deref()
        .ok_or_else(|| "PostgreSQL RangeTableSample has no relation".to_string())?;
    let sample_shape = TableSampleShape {
        method: pg_string_list(&sample.method)?,
        args: sample
            .args
            .iter()
            .map(pg_expr_shape)
            .collect::<Result<_, _>>()?,
        repeatable: sample
            .repeatable
            .as_deref()
            .map(pg_expr_shape)
            .transpose()?,
    };
    match pg_table_factor_shape(relation)? {
        TableFactorShape::Table {
            name,
            alias,
            only,
            sample: None,
        } => Ok(TableFactorShape::Table {
            name,
            alias,
            only,
            sample: Some(sample_shape),
        }),
        other => Err(format!(
            "unsupported PostgreSQL TABLESAMPLE relation: {other:?}"
        )),
    }
}

fn pg_join_operator_shape(join: &pgpb::JoinExpr) -> Result<JoinOperatorShape, String> {
    let constraint = pg_join_constraint_shape(join)?;
    let jointype = pgpb::JoinType::try_from(join.jointype).unwrap_or(pgpb::JoinType::Undefined);
    match jointype {
        pgpb::JoinType::JoinInner => Ok(JoinOperatorShape::Inner(constraint)),
        pgpb::JoinType::JoinLeft => Ok(JoinOperatorShape::LeftOuter(constraint)),
        pgpb::JoinType::JoinRight => Ok(JoinOperatorShape::RightOuter(constraint)),
        pgpb::JoinType::JoinFull => Ok(JoinOperatorShape::FullOuter(constraint)),
        other => Err(format!("unsupported PostgreSQL join type: {other:?}")),
    }
}

fn pg_join_constraint_shape(join: &pgpb::JoinExpr) -> Result<JoinConstraintShape, String> {
    if join.is_natural {
        return Ok(JoinConstraintShape::Natural);
    }
    if !join.using_clause.is_empty() {
        return Ok(JoinConstraintShape::Using {
            columns: pg_string_list(&join.using_clause)?,
            alias: join
                .join_using_alias
                .as_ref()
                .map(|alias| Ok::<_, String>(alias.aliasname.clone()))
                .transpose()?,
        });
    }
    if let Some(quals) = join.quals.as_deref() {
        return Ok(JoinConstraintShape::On(pg_expr_shape(quals)?));
    }
    Ok(JoinConstraintShape::None)
}

fn pg_order_by_shapes(nodes: &[pgpb::Node]) -> Result<Vec<OrderByShape>, String> {
    nodes.iter().map(pg_order_by_shape).collect()
}

fn pg_order_by_shape(node: &pgpb::Node) -> Result<OrderByShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::SortBy(sort)) => {
            let expr = sort
                .node
                .as_deref()
                .ok_or_else(|| "PostgreSQL SortBy has no expression".to_string())?;
            let dir =
                pgpb::SortByDir::try_from(sort.sortby_dir).unwrap_or(pgpb::SortByDir::Undefined);
            let nulls = pgpb::SortByNulls::try_from(sort.sortby_nulls)
                .unwrap_or(pgpb::SortByNulls::Undefined);
            // `USING <operator>` carries the operator in `use_op` (schema parts then
            // the operator symbol last) and reports `SortbyUsing` as its direction —
            // so it maps to `using`, not to `asc`.
            Ok(OrderByShape {
                expr: pg_expr_shape(expr)?,
                asc: match dir {
                    pgpb::SortByDir::SortbyDefault
                    | pgpb::SortByDir::Undefined
                    | pgpb::SortByDir::SortbyUsing => None,
                    pgpb::SortByDir::SortbyAsc => Some(true),
                    pgpb::SortByDir::SortbyDesc => Some(false),
                },
                using: pg_sort_using_op(&sort.use_op)?,
                nulls_first: match nulls {
                    pgpb::SortByNulls::SortbyNullsDefault | pgpb::SortByNulls::Undefined => None,
                    pgpb::SortByNulls::SortbyNullsFirst => Some(true),
                    pgpb::SortByNulls::SortbyNullsLast => Some(false),
                },
            })
        }
        other => Err(format!("unsupported PostgreSQL ORDER BY node: {other:?}")),
    }
}

/// The `USING <operator>` name parts of a PostgreSQL `SortBy` (`use_op`): the
/// schema-qualified operator as string parts with the operator symbol last, or
/// `None` when empty (the ordinary `ASC`/`DESC` sort).
fn pg_sort_using_op(nodes: &[pgpb::Node]) -> Result<Option<Vec<String>>, String> {
    if nodes.is_empty() {
        return Ok(None);
    }
    let mut parts = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node.node.as_ref() {
            Some(NodeEnum::String(name)) => parts.push(name.sval.clone()),
            other => {
                return Err(format!(
                    "unsupported PostgreSQL USING operator name node: {other:?}"
                ));
            }
        }
    }
    Ok(Some(parts))
}

fn pg_expr_shape(node: &pgpb::Node) -> Result<ExprShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::ColumnRef(column)) => pg_column_ref_shape(column),
        // PostgreSQL has already folded any unary minus on a numeric literal into a
        // signed constant (gram.y `doNegate`), so an `A_Const` maps straight to a
        // literal; our side mirrors that fold in [`fold_unary`] so the two trees
        // compare equal (ADR-0015 representation-equivalence, owned by this ticket).
        Some(NodeEnum::AConst(value)) => pg_literal_shape(value).map(ExprShape::Literal),
        Some(NodeEnum::AExpr(expr)) => pg_a_expr_shape(expr),
        Some(NodeEnum::BoolExpr(expr)) => pg_bool_expr_shape(expr),
        Some(NodeEnum::SubLink(sublink)) => pg_sub_link_shape(sublink),
        Some(NodeEnum::TypeCast(cast)) => pg_type_cast_shape(cast),
        // A generic `name(args)` call. `pg_func_call_shape` is the strict gate: it
        // rejects aggregate modifiers (DISTINCT/ORDER BY/FILTER/OVER/VARIADIC) the
        // neutral `FunctionShape` cannot represent. PostgreSQL lowers the
        // SQL-syntax functions (COALESCE/NULLIF/GREATEST/LEAST) to dedicated nodes
        // (CoalesceExpr/MinMaxExpr/AEXPR_NULLIF) our parser instead spells as a
        // generic call; that representation difference is a documented gap, left to
        // surface as an explicit divergence (ADR-0015), not normalized here.
        Some(NodeEnum::FuncCall(call)) => pg_func_call_shape(call).map(ExprShape::Function),
        // A named function argument `name => value`: PostgreSQL wraps the value in a
        // `NamedArgExpr`. The neutral `FunctionShape` compares argument values, so
        // unwrap to the inner value's shape (our `function_shape` likewise maps each
        // argument's value), keeping a named and a positional argument comparable.
        Some(NodeEnum::NamedArgExpr(named)) => {
            let arg = named
                .arg
                .as_deref()
                .ok_or_else(|| "PostgreSQL NamedArgExpr has no argument".to_string())?;
            pg_expr_shape(arg)
        }
        Some(NodeEnum::NullTest(test)) => pg_null_test_shape(test),
        Some(NodeEnum::CaseExpr(case)) => pg_case_shape(case),
        // `ARRAY[a, b, …]` — the element-list constructor maps to the same neutral
        // `Array` shape our `ArrayExpr::Elements` maps to (duckdb-collection-literals
        // wired both sides). `ARRAY(<query>)` lowers to an ARRAY-typed SubLink, which
        // stays unsupported below, matching our side's `Unmapped` for that form.
        Some(NodeEnum::AArrayExpr(array)) => Ok(ExprShape::Array(
            array
                .elements
                .iter()
                .map(pg_expr_shape)
                .collect::<Result<_, _>>()?,
        )),
        other => Err(format!("unsupported PostgreSQL expression node: {other:?}")),
    }
}

fn pg_null_test_shape(test: &pgpb::NullTest) -> Result<ExprShape, String> {
    if test.argisrow {
        // A row-wise `(a, b) IS NULL` carries whole-row semantics our scalar
        // `Expr::IsNull` does not model; surface it as an explicit gap rather than
        // silently flattening it to a scalar test (ADR-0015).
        return Err("unsupported PostgreSQL row IS NULL test".to_string());
    }
    let arg = test
        .arg
        .as_deref()
        .ok_or_else(|| "PostgreSQL NullTest has no argument".to_string())?;
    let negated = match pgpb::NullTestType::try_from(test.nulltesttype)
        .unwrap_or(pgpb::NullTestType::Undefined)
    {
        pgpb::NullTestType::IsNull => false,
        pgpb::NullTestType::IsNotNull => true,
        pgpb::NullTestType::Undefined => {
            return Err("PostgreSQL NullTest has an undefined kind".to_string());
        }
    };
    Ok(ExprShape::IsNull {
        expr: Box::new(pg_expr_shape(arg)?),
        negated,
    })
}

fn pg_case_shape(case: &pgpb::CaseExpr) -> Result<ExprShape, String> {
    // The raw parse tree keeps the simple form's compared operand in `arg` and each
    // branch's *bare* comparison value in `CaseWhen.expr` (the `arg = value`
    // rewrite is a later analysis step pg_query does not run), so this maps
    // one-to-one onto our `CaseExpr { operand, when_clauses, else_result }`.
    Ok(ExprShape::Case {
        operand: case
            .arg
            .as_deref()
            .map(pg_expr_shape)
            .transpose()?
            .map(Box::new),
        when_clauses: case
            .args
            .iter()
            .map(pg_case_when_shape)
            .collect::<Result<_, _>>()?,
        else_result: case
            .defresult
            .as_deref()
            .map(pg_expr_shape)
            .transpose()?
            .map(Box::new),
    })
}

fn pg_case_when_shape(node: &pgpb::Node) -> Result<WhenClauseShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::CaseWhen(when)) => {
            let condition = when
                .expr
                .as_deref()
                .ok_or_else(|| "PostgreSQL CaseWhen has no condition".to_string())?;
            let result = when
                .result
                .as_deref()
                .ok_or_else(|| "PostgreSQL CaseWhen has no result".to_string())?;
            Ok(WhenClauseShape {
                condition: pg_expr_shape(condition)?,
                result: pg_expr_shape(result)?,
            })
        }
        other => Err(format!(
            "unsupported PostgreSQL CASE branch node: {other:?}"
        )),
    }
}

fn pg_sub_link_shape(sublink: &pgpb::SubLink) -> Result<ExprShape, String> {
    let subquery = sublink
        .subselect
        .as_deref()
        .ok_or_else(|| "PostgreSQL SubLink has no subselect".to_string())
        .and_then(pg_subquery_shape)?;
    let sub_link_type =
        pgpb::SubLinkType::try_from(sublink.sub_link_type).unwrap_or(pgpb::SubLinkType::Undefined);

    match sub_link_type {
        pgpb::SubLinkType::ExistsSublink => {
            if sublink.testexpr.is_some() || !sublink.oper_name.is_empty() {
                return Err("PostgreSQL EXISTS SubLink carries test expression data".to_string());
            }
            Ok(ExprShape::Exists(Box::new(subquery)))
        }
        pgpb::SubLinkType::ExprSublink => {
            if sublink.testexpr.is_some() || !sublink.oper_name.is_empty() {
                return Err("PostgreSQL scalar SubLink carries test expression data".to_string());
            }
            Ok(ExprShape::Subquery(Box::new(subquery)))
        }
        pgpb::SubLinkType::AnySublink if sublink.oper_name.is_empty() => {
            let testexpr = sublink
                .testexpr
                .as_deref()
                .ok_or_else(|| "PostgreSQL IN SubLink has no test expression".to_string())?;
            Ok(ExprShape::InSubquery {
                expr: Box::new(pg_expr_shape(testexpr)?),
                subquery: Box::new(subquery),
                negated: false,
            })
        }
        pgpb::SubLinkType::AnySublink | pgpb::SubLinkType::AllSublink => {
            let testexpr = sublink.testexpr.as_deref().ok_or_else(|| {
                "PostgreSQL quantified SubLink has no test expression".to_string()
            })?;
            let op = pg_operator_name(&sublink.oper_name)?;
            Ok(ExprShape::QuantifiedComparison {
                left: Box::new(pg_expr_shape(testexpr)?),
                op: pg_binary_operator_shape(&op)?,
                quantifier: match sub_link_type {
                    pgpb::SubLinkType::AnySublink => Quantifier::Any,
                    pgpb::SubLinkType::AllSublink => Quantifier::All,
                    _ => unreachable!("outer match restricted quantified sublink kinds"),
                },
                subquery: Box::new(subquery),
            })
        }
        other => Err(format!("unsupported PostgreSQL SubLink type: {other:?}")),
    }
}

fn pg_subquery_shape(node: &pgpb::Node) -> Result<QueryShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::SelectStmt(select)) => pg_query_shape(select),
        other => Err(format!("unsupported PostgreSQL subquery node: {other:?}")),
    }
}

fn pg_type_cast_shape(cast: &pgpb::TypeCast) -> Result<ExprShape, String> {
    let arg = cast
        .arg
        .as_deref()
        .ok_or_else(|| "PostgreSQL TypeCast has no expression".to_string())?;
    let type_name = cast
        .type_name
        .as_ref()
        .ok_or_else(|| "PostgreSQL TypeCast has no type name".to_string())?;
    Ok(ExprShape::Cast {
        expr: Box::new(pg_expr_shape(arg)?),
        data_type: pg_type_name_shape(type_name)?,
    })
}

fn pg_type_name_shape(type_name: &pgpb::TypeName) -> Result<DataTypeShape, String> {
    if type_name.setof {
        return Err("unsupported PostgreSQL SETOF type name".to_string());
    }
    if type_name.pct_type {
        return Err("unsupported PostgreSQL %TYPE type name".to_string());
    }
    let raw_name = pg_string_list(&type_name.names)?;
    let name = normalize_pg_type_name(raw_name);
    let modifiers = type_name
        .typmods
        .iter()
        .map(pg_type_modifier)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DataTypeShape::Named {
        name,
        modifiers,
        array_depth: type_name.array_bounds.len(),
    })
}

fn normalize_pg_type_name(name: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::with_capacity(name.len());
    for part in name {
        normalized.push(part.to_ascii_lowercase());
    }
    if let [catalog, rest @ ..] = normalized.as_slice() {
        if catalog == "pg_catalog" {
            return rest.to_vec();
        }
    }
    normalized
}

fn pg_type_modifier(node: &pgpb::Node) -> Result<i64, String> {
    match node.node.as_ref() {
        Some(NodeEnum::AConst(value)) => pg_const_integer_modifier(value),
        Some(NodeEnum::Integer(value)) => Ok(i64::from(value.ival)),
        other => Err(format!(
            "unsupported PostgreSQL type modifier node: {other:?}"
        )),
    }
}

fn pg_const_integer_modifier(value: &pgpb::AConst) -> Result<i64, String> {
    match value.val.as_ref() {
        Some(a_const::Val::Ival(value)) => Ok(i64::from(value.ival)),
        Some(a_const::Val::Fval(value)) => value
            .fval
            .parse::<i64>()
            .map_err(|_| format!("unsupported PostgreSQL float type modifier {}", value.fval)),
        Some(a_const::Val::Boolval(_)) => {
            Err("unsupported PostgreSQL boolean type modifier".to_string())
        }
        Some(a_const::Val::Sval(_)) => {
            Err("unsupported PostgreSQL string type modifier".to_string())
        }
        Some(a_const::Val::Bsval(_)) => {
            Err("unsupported PostgreSQL bit-string type modifier".to_string())
        }
        None => Err("PostgreSQL type modifier has no value".to_string()),
    }
}

fn pg_column_ref_shape(column: &pgpb::ColumnRef) -> Result<ExprShape, String> {
    if column.fields.is_empty() {
        return Err("PostgreSQL ColumnRef has no fields".to_string());
    }
    if column
        .fields
        .iter()
        .any(|field| matches!(field.node.as_ref(), Some(NodeEnum::AStar(_))))
    {
        return Err("unsupported PostgreSQL wildcard column expression".to_string());
    }
    Ok(ExprShape::Column(pg_string_list(&column.fields)?))
}

fn pg_literal_shape(value: &pgpb::AConst) -> Result<LiteralShape, String> {
    if value.isnull {
        return Ok(LiteralShape::Null);
    }
    match value.val.as_ref() {
        Some(a_const::Val::Ival(value)) => Ok(LiteralShape::Integer(value.ival.to_string())),
        Some(a_const::Val::Fval(value)) => Ok(LiteralShape::Float(value.fval.clone())),
        Some(a_const::Val::Boolval(value)) => Ok(LiteralShape::Boolean(value.boolval)),
        Some(a_const::Val::Sval(value)) => Ok(LiteralShape::String(value.sval.clone())),
        Some(a_const::Val::Bsval(_)) => {
            Err("unsupported PostgreSQL bit-string literal".to_string())
        }
        None => Ok(LiteralShape::Null),
    }
}

fn pg_a_expr_shape(expr: &pgpb::AExpr) -> Result<ExprShape, String> {
    let kind = pgpb::AExprKind::try_from(expr.kind).unwrap_or(pgpb::AExprKind::Undefined);
    // `IS [NOT] DISTINCT FROM` is an `AExpr` whose DISTINCT *kind* — not its `name`
    // (which is "=") — carries the meaning; it is always binary. Map it before the
    // plain-operator path so the shape matches our `BinaryOperator::Is[Not]DistinctFrom`.
    let distinct = match kind {
        pgpb::AExprKind::AexprDistinct => Some(BinaryOperatorShape::IsDistinctFrom),
        pgpb::AExprKind::AexprNotDistinct => Some(BinaryOperatorShape::IsNotDistinctFrom),
        _ => None,
    };
    if let Some(op) = distinct {
        let left = expr
            .lexpr
            .as_deref()
            .ok_or_else(|| "PostgreSQL DISTINCT AExpr has no left operand".to_string())?;
        let right = expr
            .rexpr
            .as_deref()
            .ok_or_else(|| "PostgreSQL DISTINCT AExpr has no right operand".to_string())?;
        return Ok(ExprShape::BinaryOp {
            left: Box::new(pg_expr_shape(left)?),
            op,
            right: Box::new(pg_expr_shape(right)?),
        });
    }
    if kind != pgpb::AExprKind::AexprOp {
        return Err(format!("unsupported PostgreSQL expression kind: {kind:?}"));
    }
    let op = pg_operator_name(&expr.name)?;
    let right = expr
        .rexpr
        .as_deref()
        .ok_or_else(|| "PostgreSQL AExpr has no right operand".to_string())?;
    match expr.lexpr.as_deref() {
        Some(left) => Ok(ExprShape::BinaryOp {
            left: Box::new(pg_expr_shape(left)?),
            op: pg_binary_operator_shape(&op)?,
            right: Box::new(pg_expr_shape(right)?),
        }),
        None => Ok(ExprShape::UnaryOp {
            op: pg_unary_operator_shape(&op)?,
            expr: Box::new(pg_expr_shape(right)?),
        }),
    }
}

fn pg_bool_expr_shape(expr: &pgpb::BoolExpr) -> Result<ExprShape, String> {
    let boolop = pgpb::BoolExprType::try_from(expr.boolop).unwrap_or(pgpb::BoolExprType::Undefined);
    match boolop {
        pgpb::BoolExprType::AndExpr => {
            pg_fold_bool_args(&expr.args, BinaryOperatorShape::And, "AND")
        }
        pgpb::BoolExprType::OrExpr => pg_fold_bool_args(&expr.args, BinaryOperatorShape::Or, "OR"),
        pgpb::BoolExprType::NotExpr => {
            if expr.args.len() != 1 {
                return Err("PostgreSQL NOT expression does not have one argument".to_string());
            }
            let inner = pg_expr_shape(&expr.args[0])?;
            match inner {
                ExprShape::InSubquery {
                    expr,
                    subquery,
                    negated: false,
                } => Ok(ExprShape::InSubquery {
                    expr,
                    subquery,
                    negated: true,
                }),
                inner => Ok(ExprShape::UnaryOp {
                    op: UnaryOperatorShape::Not,
                    expr: Box::new(inner),
                }),
            }
        }
        other => Err(format!("unsupported PostgreSQL bool expression: {other:?}")),
    }
}

fn pg_fold_bool_args(
    args: &[pgpb::Node],
    op: BinaryOperatorShape,
    name: &'static str,
) -> Result<ExprShape, String> {
    let mut iter = args.iter();
    let first = iter
        .next()
        .ok_or_else(|| format!("PostgreSQL {name} expression has no arguments"))?;
    let mut expr = pg_expr_shape(first)?;
    for next in iter {
        expr = ExprShape::BinaryOp {
            left: Box::new(expr),
            op,
            right: Box::new(pg_expr_shape(next)?),
        };
    }
    Ok(expr)
}

fn pg_operator_name(nodes: &[pgpb::Node]) -> Result<String, String> {
    let parts = pg_string_list(nodes)?;
    if parts.is_empty() {
        return Err("PostgreSQL operator has no name".to_string());
    }
    Ok(parts.join("."))
}

fn pg_binary_operator_shape(op: &str) -> Result<BinaryOperatorShape, String> {
    match op {
        "+" => Ok(BinaryOperatorShape::Plus),
        "-" => Ok(BinaryOperatorShape::Minus),
        "*" => Ok(BinaryOperatorShape::Multiply),
        "/" => Ok(BinaryOperatorShape::Divide),
        "%" => Ok(BinaryOperatorShape::Modulo),
        "||" => Ok(BinaryOperatorShape::StringConcat),
        "=" => Ok(BinaryOperatorShape::Eq),
        "<>" | "!=" => Ok(BinaryOperatorShape::NotEq),
        "<" => Ok(BinaryOperatorShape::Lt),
        "<=" => Ok(BinaryOperatorShape::LtEq),
        ">" => Ok(BinaryOperatorShape::Gt),
        ">=" => Ok(BinaryOperatorShape::GtEq),
        "@>" => Ok(BinaryOperatorShape::Contains),
        "<@" => Ok(BinaryOperatorShape::ContainedBy),
        "->" => Ok(BinaryOperatorShape::JsonGet),
        "->>" => Ok(BinaryOperatorShape::JsonGetText),
        "?" => Ok(BinaryOperatorShape::JsonExists),
        "?|" => Ok(BinaryOperatorShape::JsonExistsAny),
        "?&" => Ok(BinaryOperatorShape::JsonExistsAll),
        "@?" => Ok(BinaryOperatorShape::JsonPathExists),
        "@@" => Ok(BinaryOperatorShape::JsonPathMatch),
        "#>" => Ok(BinaryOperatorShape::JsonExtractPath),
        "#>>" => Ok(BinaryOperatorShape::JsonExtractPathText),
        "#-" => Ok(BinaryOperatorShape::JsonDeletePath),
        "|" => Ok(BinaryOperatorShape::BitwiseOr),
        "&" => Ok(BinaryOperatorShape::BitwiseAnd),
        "<<" => Ok(BinaryOperatorShape::BitwiseShiftLeft),
        ">>" => Ok(BinaryOperatorShape::BitwiseShiftRight),
        "#" => Ok(BinaryOperatorShape::BitwiseXor),
        "^" => Ok(BinaryOperatorShape::Exponent),
        other => Err(format!("unsupported PostgreSQL binary operator: {other}")),
    }
}

fn pg_unary_operator_shape(op: &str) -> Result<UnaryOperatorShape, String> {
    match op {
        "-" => Ok(UnaryOperatorShape::Minus),
        "+" => Ok(UnaryOperatorShape::Plus),
        "~" => Ok(UnaryOperatorShape::BitwiseNot),
        other => Err(format!("unsupported PostgreSQL unary operator: {other}")),
    }
}

fn pg_range_var_name(range: &pgpb::RangeVar) -> Vec<String> {
    [&range.catalogname, &range.schemaname, &range.relname]
        .into_iter()
        .filter(|part| !part.is_empty())
        .cloned()
        .collect()
}

fn pg_alias_shape(alias: &pgpb::Alias) -> Result<AliasShape, String> {
    Ok(AliasShape {
        name: alias.aliasname.clone(),
        columns: pg_string_list(&alias.colnames)?,
    })
}

fn pg_string_node(node: &pgpb::Node) -> Result<String, String> {
    match node.node.as_ref() {
        Some(NodeEnum::String(value)) => Ok(value.sval.clone()),
        other => Err(format!("expected PostgreSQL string node, found {other:?}")),
    }
}

// ---- PostgreSQL protobuf -> transaction / session / DCL / EXPLAIN shape -----

fn pg_transaction_shape(transaction: &pgpb::TransactionStmt) -> Result<StatementShape, String> {
    let kind = pgpb::TransactionStmtKind::try_from(transaction.kind)
        .unwrap_or(pgpb::TransactionStmtKind::Undefined);
    let shape = match kind {
        pgpb::TransactionStmtKind::TransStmtBegin => TransactionShape::Begin {
            start: false,
            modes: pg_transaction_modes(&transaction.options)?,
        },
        pgpb::TransactionStmtKind::TransStmtStart => TransactionShape::Begin {
            start: true,
            modes: pg_transaction_modes(&transaction.options)?,
        },
        pgpb::TransactionStmtKind::TransStmtCommit => TransactionShape::Commit,
        pgpb::TransactionStmtKind::TransStmtRollback => {
            TransactionShape::Rollback { to_savepoint: None }
        }
        pgpb::TransactionStmtKind::TransStmtRollbackTo => TransactionShape::Rollback {
            to_savepoint: Some(transaction.savepoint_name.clone()),
        },
        pgpb::TransactionStmtKind::TransStmtSavepoint => {
            TransactionShape::Savepoint(transaction.savepoint_name.clone())
        }
        pgpb::TransactionStmtKind::TransStmtRelease => {
            TransactionShape::Release(transaction.savepoint_name.clone())
        }
        // Two-phase-commit forms (`PREPARE TRANSACTION`, ...) are outside the M1 AST.
        other => {
            return Err(format!(
                "unsupported PostgreSQL transaction statement kind: {other:?}"
            ));
        }
    };
    Ok(StatementShape::Transaction(shape))
}

fn pg_transaction_modes(options: &[pgpb::Node]) -> Result<Vec<TransactionModeShape>, String> {
    options.iter().map(pg_transaction_mode).collect()
}

fn pg_transaction_mode(node: &pgpb::Node) -> Result<TransactionModeShape, String> {
    let Some(NodeEnum::DefElem(elem)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL transaction mode node: {:?}",
            node.node
        ));
    };
    match elem.defname.as_str() {
        "transaction_isolation" => Ok(TransactionModeShape::IsolationLevel(
            pg_isolation_level_arg(elem)?,
        )),
        "transaction_read_only" => Ok(TransactionModeShape::ReadOnly(pg_transaction_flag(elem)?)),
        "transaction_deferrable" => {
            Ok(TransactionModeShape::Deferrable(pg_transaction_flag(elem)?))
        }
        other => Err(format!("unsupported PostgreSQL transaction mode: {other}")),
    }
}

fn pg_isolation_level_arg(elem: &pgpb::DefElem) -> Result<IsolationLevelShape, String> {
    let arg = elem
        .arg
        .as_deref()
        .ok_or_else(|| "PostgreSQL transaction_isolation has no argument".to_string())?;
    let text = match arg.node.as_ref() {
        Some(NodeEnum::AConst(value)) => match value.val.as_ref() {
            Some(a_const::Val::Sval(value)) => value.sval.as_str(),
            other => {
                return Err(format!(
                    "unsupported PostgreSQL isolation level constant: {other:?}"
                ));
            }
        },
        other => {
            return Err(format!(
                "unsupported PostgreSQL isolation level node: {other:?}"
            ));
        }
    };
    match text {
        "read uncommitted" => Ok(IsolationLevelShape::ReadUncommitted),
        "read committed" => Ok(IsolationLevelShape::ReadCommitted),
        "repeatable read" => Ok(IsolationLevelShape::RepeatableRead),
        "serializable" => Ok(IsolationLevelShape::Serializable),
        other => Err(format!("unsupported PostgreSQL isolation level: {other}")),
    }
}

/// The boolean a `transaction_read_only` / `transaction_deferrable` `DefElem`
/// carries — PostgreSQL encodes it as an integer `A_Const` (`0`/`1`), older trees as
/// a `Boolean` node. (The DDL `pg_def_elem_bool` only handles the `Boolean` form.)
fn pg_transaction_flag(elem: &pgpb::DefElem) -> Result<bool, String> {
    let arg = elem
        .arg
        .as_deref()
        .ok_or_else(|| format!("PostgreSQL {} has no argument", elem.defname))?;
    match arg.node.as_ref() {
        Some(NodeEnum::AConst(value)) => match value.val.as_ref() {
            Some(a_const::Val::Ival(value)) => Ok(value.ival != 0),
            other => Err(format!(
                "unsupported PostgreSQL boolean constant: {other:?}"
            )),
        },
        Some(NodeEnum::Boolean(value)) => Ok(value.boolval),
        other => Err(format!("unsupported PostgreSQL boolean node: {other:?}")),
    }
}

fn pg_variable_set_shape(set: &pgpb::VariableSetStmt) -> Result<StatementShape, String> {
    let kind =
        pgpb::VariableSetKind::try_from(set.kind).unwrap_or(pgpb::VariableSetKind::Undefined);
    match kind {
        pgpb::VariableSetKind::VarSetValue => Ok(StatementShape::Session(SessionShape::Set {
            local: set.is_local,
            name: set.name.to_ascii_lowercase(),
            value: SetValueShape::Values(pg_set_values(&set.args)?),
        })),
        pgpb::VariableSetKind::VarSetDefault => Ok(StatementShape::Session(SessionShape::Set {
            local: set.is_local,
            name: set.name.to_ascii_lowercase(),
            value: SetValueShape::Default,
        })),
        pgpb::VariableSetKind::VarReset => Ok(StatementShape::Session(SessionShape::Reset {
            name: Some(set.name.to_ascii_lowercase()),
        })),
        pgpb::VariableSetKind::VarResetAll => {
            Ok(StatementShape::Session(SessionShape::Reset { name: None }))
        }
        // `SET TRANSACTION` lowers to `VarSetMulti` with the pseudo-name `TRANSACTION`;
        // the other `VarSetMulti` names (`SESSION CHARACTERISTICS`, ...) are the special
        // SET subforms kept accept-only (see [`StatementShape`]).
        pgpb::VariableSetKind::VarSetMulti if set.name == "TRANSACTION" => Ok(
            StatementShape::Transaction(TransactionShape::SetCharacteristics {
                modes: pg_transaction_modes(&set.args)?,
            }),
        ),
        other => Err(format!(
            "unsupported PostgreSQL VariableSetStmt kind/name: {other:?}/{}",
            set.name
        )),
    }
}

fn pg_set_values(args: &[pgpb::Node]) -> Result<Vec<SetParameterValueShape>, String> {
    args.iter().map(pg_set_value).collect()
}

fn pg_set_value(node: &pgpb::Node) -> Result<SetParameterValueShape, String> {
    match node.node.as_ref() {
        Some(NodeEnum::AConst(value)) => {
            pg_literal_shape(value).map(SetParameterValueShape::Literal)
        }
        Some(NodeEnum::ParamRef(parameter)) => u32::try_from(parameter.number)
            .map(SetParameterValueShape::PositionalParameter)
            .map_err(|_| {
                format!(
                    "PostgreSQL SET positional parameter is negative: {}",
                    parameter.number
                )
            }),
        other => Err(format!("unsupported PostgreSQL SET value node: {other:?}")),
    }
}

fn pg_grant_shape(grant: &pgpb::GrantStmt) -> Result<AccessControlShape, String> {
    Ok(AccessControlShape::Privilege {
        is_grant: grant.is_grant,
        grant_option: grant.grant_option,
        privileges: pg_privileges_shape(&grant.privileges)?,
        object: pg_grant_object_shape(grant)?,
        grantees: pg_role_specs(&grant.grantees)?,
        granted_by: grant.grantor.as_ref().map(pg_role_spec_value).transpose()?,
        cascade: pg_drop_cascade(grant.behavior),
    })
}

fn pg_grant_role_shape(grant: &pgpb::GrantRoleStmt) -> Result<AccessControlShape, String> {
    Ok(AccessControlShape::Role {
        is_grant: grant.is_grant,
        // `WITH ADMIN OPTION` / `ADMIN OPTION FOR` both surface as an `admin` DefElem
        // (its boolean reflects the grant/revoke direction, already carried by
        // `is_grant`); its mere presence records that the clause was written.
        admin_option: grant.opt.iter().any(pg_is_admin_option),
        roles: pg_granted_role_names(&grant.granted_roles)?,
        grantees: pg_role_specs(&grant.grantee_roles)?,
        granted_by: grant.grantor.as_ref().map(pg_role_spec_value).transpose()?,
        cascade: pg_drop_cascade(grant.behavior),
    })
}

fn pg_is_admin_option(node: &pgpb::Node) -> bool {
    matches!(node.node.as_ref(), Some(NodeEnum::DefElem(elem)) if elem.defname == "admin")
}

fn pg_granted_role_names(nodes: &[pgpb::Node]) -> Result<Vec<String>, String> {
    nodes
        .iter()
        .map(|node| match node.node.as_ref() {
            // PostgreSQL records a granted role as an `AccessPriv` whose `priv_name`
            // is the already case-folded role name.
            Some(NodeEnum::AccessPriv(access)) => Ok(access.priv_name.clone()),
            other => Err(format!(
                "unsupported PostgreSQL granted role node: {other:?}"
            )),
        })
        .collect()
}

fn pg_privileges_shape(privileges: &[pgpb::Node]) -> Result<PrivilegesShape, String> {
    if privileges.is_empty() {
        return Ok(PrivilegesShape::All);
    }
    Ok(PrivilegesShape::List(
        privileges
            .iter()
            .map(pg_privilege_shape)
            .collect::<Result<_, _>>()?,
    ))
}

fn pg_privilege_shape(node: &pgpb::Node) -> Result<PrivilegeShape, String> {
    let Some(NodeEnum::AccessPriv(access)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL privilege node: {:?}",
            node.node
        ));
    };
    Ok(PrivilegeShape {
        name: access.priv_name.to_ascii_lowercase(),
        columns: pg_string_list(&access.cols)?,
    })
}

fn pg_grant_object_shape(grant: &pgpb::GrantStmt) -> Result<GrantObjectShape, String> {
    let objtype = pgpb::ObjectType::try_from(grant.objtype).unwrap_or(pgpb::ObjectType::Undefined);
    let targtype =
        pgpb::GrantTargetType::try_from(grant.targtype).unwrap_or(pgpb::GrantTargetType::Undefined);
    match targtype {
        pgpb::GrantTargetType::AclTargetObject => {
            if objtype == pgpb::ObjectType::ObjectTable {
                return Ok(GrantObjectShape::Objects {
                    kind: GrantObjectKindShape::Table,
                    names: pg_grant_object_names(&grant.objects)?,
                });
            }
            if let Some(kind) = pg_routine_kind(objtype) {
                return Ok(GrantObjectShape::Routines {
                    kind,
                    routines: pg_routine_signatures(&grant.objects)?,
                });
            }
            let kind = pg_named_object_kind(objtype)
                .ok_or_else(|| format!("unsupported PostgreSQL GRANT object type: {objtype:?}"))?;
            Ok(GrantObjectShape::Objects {
                kind,
                names: pg_grant_object_names(&grant.objects)?,
            })
        }
        pgpb::GrantTargetType::AclTargetAllInSchema => Ok(GrantObjectShape::AllInSchema {
            class: pg_schema_class(objtype)?,
            schemas: pg_grant_object_names(&grant.objects)?,
        }),
        other => Err(format!(
            "unsupported PostgreSQL GRANT target type: {other:?}"
        )),
    }
}

fn pg_named_object_kind(objtype: pgpb::ObjectType) -> Option<GrantObjectKindShape> {
    Some(match objtype {
        pgpb::ObjectType::ObjectSequence => GrantObjectKindShape::Sequence,
        pgpb::ObjectType::ObjectDatabase => GrantObjectKindShape::Database,
        pgpb::ObjectType::ObjectSchema => GrantObjectKindShape::Schema,
        pgpb::ObjectType::ObjectDomain => GrantObjectKindShape::Domain,
        pgpb::ObjectType::ObjectType => GrantObjectKindShape::Type,
        pgpb::ObjectType::ObjectLanguage => GrantObjectKindShape::Language,
        pgpb::ObjectType::ObjectTablespace => GrantObjectKindShape::Tablespace,
        pgpb::ObjectType::ObjectFdw => GrantObjectKindShape::ForeignDataWrapper,
        pgpb::ObjectType::ObjectForeignServer => GrantObjectKindShape::ForeignServer,
        _ => return None,
    })
}

fn pg_routine_kind(objtype: pgpb::ObjectType) -> Option<RoutineKindShape> {
    Some(match objtype {
        pgpb::ObjectType::ObjectFunction => RoutineKindShape::Function,
        pgpb::ObjectType::ObjectProcedure => RoutineKindShape::Procedure,
        pgpb::ObjectType::ObjectRoutine => RoutineKindShape::Routine,
        _ => return None,
    })
}

fn pg_schema_class(objtype: pgpb::ObjectType) -> Result<SchemaClassShape, String> {
    match objtype {
        pgpb::ObjectType::ObjectTable => Ok(SchemaClassShape::Tables),
        pgpb::ObjectType::ObjectSequence => Ok(SchemaClassShape::Sequences),
        pgpb::ObjectType::ObjectFunction => Ok(SchemaClassShape::Functions),
        pgpb::ObjectType::ObjectProcedure => Ok(SchemaClassShape::Procedures),
        pgpb::ObjectType::ObjectRoutine => Ok(SchemaClassShape::Routines),
        other => Err(format!(
            "unsupported PostgreSQL ALL IN SCHEMA object type: {other:?}"
        )),
    }
}

fn pg_grant_object_names(objects: &[pgpb::Node]) -> Result<Vec<Vec<String>>, String> {
    objects.iter().map(pg_grant_object_name).collect()
}

fn pg_grant_object_name(node: &pgpb::Node) -> Result<Vec<String>, String> {
    match node.node.as_ref() {
        // Tables/sequences ride a `RangeVar`; schemas/databases/languages/tablespaces
        // and the foreign objects ride a bare `String`; domains/types ride a `List`.
        Some(NodeEnum::RangeVar(range)) => Ok(pg_range_var_name(range)),
        Some(NodeEnum::String(value)) => Ok(vec![value.sval.clone()]),
        Some(NodeEnum::List(list)) => pg_string_list(&list.items),
        other => Err(format!(
            "unsupported PostgreSQL GRANT object name node: {other:?}"
        )),
    }
}

fn pg_routine_signatures(objects: &[pgpb::Node]) -> Result<Vec<RoutineSignatureShape>, String> {
    objects.iter().map(pg_routine_signature_shape).collect()
}

fn pg_routine_signature_shape(node: &pgpb::Node) -> Result<RoutineSignatureShape, String> {
    let Some(NodeEnum::ObjectWithArgs(object)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL routine object node: {:?}",
            node.node
        ));
    };
    Ok(RoutineSignatureShape {
        name: pg_string_list(&object.objname)?,
        // `args_unspecified` is the no-parentheses form (`FUNCTION foo`); otherwise the
        // (possibly empty) `objargs` type list is the written signature.
        arg_types: if object.args_unspecified {
            None
        } else {
            Some(
                object
                    .objargs
                    .iter()
                    .map(pg_routine_arg_type)
                    .collect::<Result<_, _>>()?,
            )
        },
    })
}

fn pg_routine_arg_type(node: &pgpb::Node) -> Result<DataTypeShape, String> {
    let Some(NodeEnum::TypeName(type_name)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL routine argument node: {:?}",
            node.node
        ));
    };
    pg_type_name_shape(type_name)
}

fn pg_role_specs(nodes: &[pgpb::Node]) -> Result<Vec<RoleSpecShape>, String> {
    nodes.iter().map(pg_role_spec_node).collect()
}

fn pg_role_spec_node(node: &pgpb::Node) -> Result<RoleSpecShape, String> {
    let Some(NodeEnum::RoleSpec(spec)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL role spec node: {:?}",
            node.node
        ));
    };
    pg_role_spec_value(spec)
}

fn pg_role_spec_value(spec: &pgpb::RoleSpec) -> Result<RoleSpecShape, String> {
    match pgpb::RoleSpecType::try_from(spec.roletype).unwrap_or(pgpb::RoleSpecType::Undefined) {
        pgpb::RoleSpecType::RolespecCstring => Ok(RoleSpecShape::Name(spec.rolename.clone())),
        pgpb::RoleSpecType::RolespecCurrentRole => Ok(RoleSpecShape::CurrentRole),
        pgpb::RoleSpecType::RolespecCurrentUser => Ok(RoleSpecShape::CurrentUser),
        pgpb::RoleSpecType::RolespecSessionUser => Ok(RoleSpecShape::SessionUser),
        pgpb::RoleSpecType::RolespecPublic => Ok(RoleSpecShape::Public),
        other => Err(format!("unsupported PostgreSQL role spec type: {other:?}")),
    }
}

fn pg_explain_shape(explain: &pgpb::ExplainStmt) -> Result<ExplainShape, String> {
    let statement = explain
        .query
        .as_deref()
        .ok_or_else(|| "PostgreSQL EXPLAIN has no inner statement".to_string())?;
    Ok(ExplainShape {
        options: explain
            .options
            .iter()
            .map(pg_explain_option_shape)
            .collect::<Result<_, _>>()?,
        statement: Box::new(pg_node_shape(statement)?),
    })
}

fn pg_explain_option_shape(node: &pgpb::Node) -> Result<ExplainOptionShape, String> {
    let Some(NodeEnum::DefElem(elem)) = node.node.as_ref() else {
        return Err(format!(
            "unsupported PostgreSQL EXPLAIN option node: {:?}",
            node.node
        ));
    };
    Ok(ExplainOptionShape {
        name: elem.defname.to_ascii_lowercase(),
        value: pg_explain_option_value(elem)?,
    })
}

fn pg_explain_option_value(elem: &pgpb::DefElem) -> Result<Option<String>, String> {
    let Some(arg) = elem.arg.as_deref() else {
        return Ok(None);
    };
    let text = match arg.node.as_ref() {
        Some(NodeEnum::String(value)) => value.sval.clone(),
        Some(NodeEnum::AConst(value)) => match pg_literal_shape(value)? {
            LiteralShape::String(text) => text,
            LiteralShape::Integer(text) | LiteralShape::Float(text) => text,
            LiteralShape::Boolean(value) => value.to_string(),
            LiteralShape::Null => "null".to_owned(),
        },
        Some(NodeEnum::Boolean(value)) => value.boolval.to_string(),
        Some(NodeEnum::Integer(value)) => value.ival.to_string(),
        other => {
            return Err(format!(
                "unsupported PostgreSQL EXPLAIN option value node: {other:?}"
            ));
        }
    };
    Ok(Some(text.to_ascii_lowercase()))
}
