// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Dialect feature coverage matrix.
//!
//! ADR-0015 requires every self-described [`FeatureSet`] field to have explicit
//! positive and negative behavior coverage. Metadata assertions are still useful
//! diagnostics, but they do not satisfy the behavior gate: every feature needs
//! objective accept/reject or structural evidence.

use squonk::ast::dialect::lex_class::{
    CLASS_IDENTIFIER_CONTINUE, CLASS_IDENTIFIER_START, CLASS_OPERATOR,
};
use squonk::ast::dialect::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, Conformance, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FEATURES, Feature, FeatureDelta,
    FeatureSet, GroupingSyntax, IdentifierQuote, IdentifierSyntax, IndexAlterSyntax, JoinSyntax,
    KeywordOperators, KeywordSet, MaintenanceSyntax, Maturity, MutationSyntax, NullOrdering,
    NumericLiteralSyntax, OperatorSyntax, ParameterSyntax, PipeOperator, PredicateSyntax,
    QueryTailSyntax, STANDARD_FEATURE_CATALOG, SelectSyntax, SessionVariableSyntax, ShowSyntax,
    StandardVersion, StatementDdlGates, StringFuncForms, StringLiteralSyntax,
    TableExpressionSyntax, TableFactorSyntax, TypeNameSyntax, UtilitySyntax, max_feature_metadata,
    standard_feature, standard_features_as_of, unsupported_standard_features,
};
use squonk::ast::precedence::{Assoc, BindingPower};
use squonk::ast::{
    BinaryOperator, CastSyntax, DataType, EqualsSpelling, Expr, GroupByItem, IntegerDivideSpelling,
    NoExt, SelectItem, SetExpr, SetOperator, Statement, TableFactor,
};
use squonk::dialect::{Ansi, MySql, Postgres};
use squonk::tokenizer::{Operator, Token, TokenKind, tokenize_with};
use squonk::{Dialect, Parsed, parse_with};
use squonk_ast::render::{RenderConfig, RenderCtx, RenderExt as _, RenderSpelling};

mod cases;
pub(crate) mod harness;
mod labeled;
mod lattice;
mod matrix;

pub(crate) use harness::accepts_under;
pub(crate) use labeled::{
    feature_flip_changes_parse, feature_set_with, required_features_satisfied,
};
// The M2 oracle matrix rows are consumed only by the `oracle-engines`-gated `m2` module
// (`crate::coverage::M2_ORACLE_ROWS`); gate the re-export to match its sole consumer.
#[cfg(feature = "oracle-engines")]
pub(crate) use matrix::M2_ORACLE_ROWS;
