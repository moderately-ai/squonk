// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Emit the drift-prone `FeatureSet` delta/registry boilerplate.
//!
//! ADR-0011 models a dialect as the const `FeatureSet` data value, but adding one
//! typed dimension to that struct used to mean hand-syncing ~6 sites: the
//! `FeatureDelta` mirror (field, `EMPTY`, `with` arm, setter) and the
//! `Feature` / `FeatureMetadata` registry
//! (variant, `id`, `ALL` count, `FEATURE_METADATA`). That per-field, cross-type
//! duplication is exactly the class of walk ADR-0013 codegens elsewhere, so this
//! module generates it from one source of truth.
//!
//! ## Source of truth
//!
//! The hand-written `FeatureSet` struct, read as *text* from `dialect/mod.rs`
//! (never by compiling the crate, ADR-0013), anchors *which* fields exist, their
//! types, and their order. The [`FEATURE_FIELDS`] table supplies only the
//! per-field registry metadata the struct cannot carry: the stable `Feature`
//! variant and `id` (note `identifier_quotes` keeps the historical singular
//! `identifier_quote` id) and the 1:1 ISO feature anchors (ADR-0011's
//! self-describing metadata). The two are kept in exact bijection — generation
//! fails loudly if a struct field has no annotation row or a row has no field —
//! so the registry, `ALL` count, and metadata can never drift from the struct.

use std::collections::BTreeSet;
use std::fs;

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use syn::{Fields, Item, Type};

/// Per-field registry metadata that the `FeatureSet` struct text cannot express.
///
/// Everything else the generator needs (field name, type, order) is read from the
/// struct; this table is the "source-of-truth annotation" the ticket calls for.
struct FieldMeta {
    /// `FeatureSet` struct field name; the join key against the struct text.
    field: &'static str,
    /// `Feature` enum variant name. Usually PascalCase of `field`, but
    /// `identifier_quotes` -> `IdentifierQuote` keeps the established singular.
    variant: &'static str,
    /// Stable machine-readable `Feature::id` (usually `field`, but the historical
    /// `identifier_quote` singular is preserved rather than silently renamed).
    id: &'static str,
    /// SQL:2016 feature id this knob anchors to 1:1, when it does; `None` for
    /// parser mechanisms and aggregate knobs whose sub-features anchor in
    /// `STANDARD_FEATURE_CATALOG`.
    iso_id: Option<&'static str>,
    /// Maturity of this metadata entry.
    maturity: &'static str,
    /// Whether an "enable everything" preset should turn this feature on.
    ideally_enabled: bool,
}

/// The single, ordered registry annotation, one row per `FeatureSet` field. Kept
/// in struct-field order for readability; the generator re-derives order from the
/// struct, so a row that drifts out of place is corrected, never silently honoured.
const FEATURE_FIELDS: &[FieldMeta] = &[
    meta(
        "identifier_casing",
        "IdentifierCasing",
        "identifier_casing",
        None,
    ),
    // E031 "Identifiers" -> E031-01 "Delimited identifiers" (Core).
    meta(
        "identifier_quotes",
        "IdentifierQuote",
        "identifier_quote",
        Some("E031-01"),
    ),
    meta(
        "default_null_ordering",
        "DefaultNullOrdering",
        "default_null_ordering",
        None,
    ),
    meta(
        "reserved_column_name",
        "ReservedColumnName",
        "reserved_column_name",
        None,
    ),
    meta(
        "reserved_function_name",
        "ReservedFunctionName",
        "reserved_function_name",
        None,
    ),
    meta(
        "reserved_type_name",
        "ReservedTypeName",
        "reserved_type_name",
        None,
    ),
    meta(
        "reserved_bare_alias",
        "ReservedBareAlias",
        "reserved_bare_alias",
        None,
    ),
    meta(
        "reserved_as_label",
        "ReservedAsLabel",
        "reserved_as_label",
        None,
    ),
    meta(
        "catalog_qualified_names",
        "CatalogQualifiedNames",
        "catalog_qualified_names",
        None,
    ),
    meta("byte_classes", "ByteClasses", "byte_classes", None),
    meta("binding_powers", "BindingPowers", "binding_powers", None),
    meta(
        "set_operation_powers",
        "SetOperationPowers",
        "set_operation_powers",
        None,
    ),
    meta("string_literals", "StringLiterals", "string_literals", None),
    meta(
        "numeric_literals",
        "NumericLiterals",
        "numeric_literals",
        None,
    ),
    meta("parameters", "Parameters", "parameters", None),
    meta(
        "session_variables",
        "SessionVariables",
        "session_variables",
        None,
    ),
    meta(
        "identifier_syntax",
        "IdentifierSyntax",
        "identifier_syntax",
        None,
    ),
    meta(
        "table_expressions",
        "TableExpressions",
        "table_expressions",
        None,
    ),
    meta("join_syntax", "JoinSyntax", "join_syntax", None),
    meta(
        "table_factor_syntax",
        "TableFactorSyntax",
        "table_factor_syntax",
        None,
    ),
    meta(
        "expression_syntax",
        "ExpressionSyntax",
        "expression_syntax",
        None,
    ),
    meta("operator_syntax", "OperatorSyntax", "operator_syntax", None),
    meta("call_syntax", "CallSyntax", "call_syntax", None),
    meta(
        "string_func_forms",
        "StringFuncForms",
        "string_func_forms",
        None,
    ),
    meta(
        "aggregate_call_syntax",
        "AggregateCallSyntax",
        "aggregate_call_syntax",
        None,
    ),
    // E021 "Character string types" -> E021-08 "LIKE predicate". The knob's headline
    // standard feature is the core `LIKE` predicate (mirroring `pipe_operator` ->
    // E021-07); the `ILIKE`/`SIMILAR TO` members are dialect extensions on top.
    meta(
        "predicate_syntax",
        "PredicateSyntax",
        "predicate_syntax",
        Some("E021-08"),
    ),
    // E021 "Character string types" -> E021-07 "Character concatenation".
    meta(
        "pipe_operator",
        "PipeOperator",
        "pipe_operator",
        Some("E021-07"),
    ),
    meta(
        "double_ampersand",
        "DoubleAmpersand",
        "double_ampersand",
        None,
    ),
    meta(
        "keyword_operators",
        "KeywordOperators",
        "keyword_operators",
        None,
    ),
    meta("caret_operator", "CaretOperator", "caret_operator", None),
    meta(
        "hash_bitwise_xor",
        "HashBitwiseXor",
        "hash_bitwise_xor",
        None,
    ),
    meta("comment_syntax", "CommentSyntax", "comment_syntax", None),
    meta("mutation_syntax", "MutationSyntax", "mutation_syntax", None),
    meta(
        "statement_ddl_gates",
        "StatementDdlGates",
        "statement_ddl_gates",
        None,
    ),
    meta(
        "create_table_clause_syntax",
        "CreateTableClauseSyntax",
        "create_table_clause_syntax",
        None,
    ),
    meta(
        "column_definition_syntax",
        "ColumnDefinitionSyntax",
        "column_definition_syntax",
        None,
    ),
    meta(
        "constraint_syntax",
        "ConstraintSyntax",
        "constraint_syntax",
        None,
    ),
    meta(
        "index_alter_syntax",
        "IndexAlterSyntax",
        "index_alter_syntax",
        None,
    ),
    meta(
        "existence_guards",
        "ExistenceGuards",
        "existence_guards",
        None,
    ),
    meta("select_syntax", "SelectSyntax", "select_syntax", None),
    meta(
        "query_tail_syntax",
        "QueryTailSyntax",
        "query_tail_syntax",
        None,
    ),
    meta("grouping_syntax", "GroupingSyntax", "grouping_syntax", None),
    meta("utility_syntax", "UtilitySyntax", "utility_syntax", None),
    meta("show_syntax", "ShowSyntax", "show_syntax", None),
    meta(
        "maintenance_syntax",
        "MaintenanceSyntax",
        "maintenance_syntax",
        None,
    ),
    meta(
        "access_control_syntax",
        "AccessControlSyntax",
        "access_control_syntax",
        None,
    ),
    meta(
        "type_name_syntax",
        "TypeNameSyntax",
        "type_name_syntax",
        None,
    ),
    meta("target_spelling", "TargetSpelling", "target_spelling", None),
];

/// Build a `FieldMeta` with the current uniform maturity/polarity defaults.
///
/// Every dialect-data knob today is `Stable` and additive; a future
/// experimental or restrictive dimension sets these explicitly on its row and the
/// generated `maturity` / `ideally_enabled` switch to per-field matches.
const fn meta(
    field: &'static str,
    variant: &'static str,
    id: &'static str,
    iso_id: Option<&'static str>,
) -> FieldMeta {
    FieldMeta {
        field,
        variant,
        id,
        iso_id,
        maturity: "Stable",
        ideally_enabled: true,
    }
}

/// One `FeatureSet` field resolved against its annotation row.
struct ResolvedField {
    ident: syn::Ident,
    ty: Type,
    meta: &'static FieldMeta,
}

/// Render the full contents of `generated/feature_set.rs`.
pub(crate) fn render() -> String {
    let fields = resolve_fields();

    let delta_fields = fields.iter().map(|f| {
        let (ident, ty) = (&f.ident, &f.ty);
        let doc = doc_attr(&format!(
            "Override for the `{}` dialect-data dimension; `None` preserves the base value.",
            f.meta.id
        ));
        quote! {
            #doc
            pub #ident: Option<#ty>,
        }
    });
    let empty_fields = fields.iter().map(|f| {
        let ident = &f.ident;
        quote! { #ident: None, }
    });
    let delta_setters = fields.iter().map(|f| {
        let (ident, ty) = (&f.ident, &f.ty);
        let doc = doc_attr(&format!("Override the `{ident}` dialect-data dimension."));
        quote! {
            #doc
            pub const fn #ident(mut self, value: #ty) -> Self {
                self.#ident = Some(value);
                self
            }
        }
    });
    let with_arms = fields.iter().map(|f| {
        let ident = &f.ident;
        quote! {
            #ident: match delta.#ident {
                Some(value) => value,
                None => self.#ident,
            },
        }
    });
    let variants = fields.iter().map(|f| {
        let variant = variant_ident(f);
        let doc = doc_attr(&format!("The `{}` dialect-data dimension.", f.meta.id));
        quote! {
            #doc
            #variant,
        }
    });
    let all_entries = fields.iter().map(|f| {
        let v = variant_ident(f);
        quote! { Self::#v, }
    });
    let id_arms = fields.iter().map(|f| {
        let v = variant_ident(f);
        let id = f.meta.id;
        quote! { Self::#v => #id, }
    });
    let metadata_entries = fields.iter().map(|f| {
        let v = variant_ident(f);
        quote! { Feature::#v.metadata(), }
    });

    let count = Literal::usize_unsuffixed(fields.len());
    let maturity_body = uniform_or_match(&fields, |f| {
        let m = format_ident!("{}", f.meta.maturity);
        quote! { Maturity::#m }
    });
    let ideally_body = uniform_or_match(&fields, |f| {
        let enabled = f.meta.ideally_enabled;
        quote! { #enabled }
    });
    let iso_body = iso_id_body(&fields);

    // Doc comments emitted with a leading space so prettyplease renders the
    // idiomatic `/// text` (not `///text`); blank lines render as a bare `///`.
    let doc_delta = doc_lines(&[
        "Explicit customizations applied to a base [`FeatureSet`].",
        "",
        "Applied by [`FeatureSet::with`] (unchecked) or [`FeatureSet::try_with`], which",
        "returns the first [`LexicalConflict`] a delta would introduce; see",
        "[`FeatureSet::lexical_conflict`] for the shared-trigger hazards a delta can create.",
    ]);
    let doc_empty = doc_attr("No changes from the base feature set.");
    let doc_with = doc_attr("Return a custom dialect data value by applying `delta` to this base.");
    let doc_feature = doc_attr("Enumerable dialect data fields for the coverage matrix.");
    let doc_all =
        doc_attr("Features in stable coverage-matrix order; `Feature::ALL[d] as usize == d`.");
    let doc_id = doc_attr("Stable machine-readable feature id.");
    let doc_maturity = doc_attr("Current maturity of this feature metadata entry.");
    let doc_ideally = doc_lines(&[
        "Whether an \"enable everything\" preset should include this feature; `false`",
        "marks a negative-polarity feature kept out of max-feature unions. See",
        "[`FeatureMetadata::ideally_enabled`].",
    ]);
    let doc_iso = doc_lines(&[
        "The SQL:2016 feature id this dialect-data field anchors to 1:1, when it does.",
        "",
        "Aggregate knobs whose sub-features map individually and pure parser mechanisms",
        "return `None`; their standard anchors live as rows in `STANDARD_FEATURE_CATALOG`.",
    ]);
    let doc_metadata = doc_attr("Full metadata entry for coverage enumeration.");
    let doc_features = doc_attr("All self-described dialect data fields.");
    let doc_feature_metadata =
        doc_attr("All self-described dialect data fields with stable metadata.");

    let tokens = quote! {
        use crate::dialect::*;
        use crate::precedence::{BindingPowerTable, SetOperationBindingPowerTable};

        #doc_delta
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct FeatureDelta {
            #(#delta_fields)*
        }

        impl FeatureDelta {
            #doc_empty
            pub const EMPTY: Self = Self {
                #(#empty_fields)*
            };

            #(#delta_setters)*
        }

        impl Default for FeatureDelta {
            fn default() -> Self {
                Self::EMPTY
            }
        }

        impl FeatureSet {
            #doc_with
            pub const fn with(&self, delta: FeatureDelta) -> Self {
                Self {
                    #(#with_arms)*
                }
            }
        }

        #doc_feature
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Feature {
            #(#variants)*
        }

        impl Feature {
            #doc_all
            pub const ALL: [Self; #count] = [
                #(#all_entries)*
            ];

            #doc_id
            pub const fn id(&self) -> &'static str {
                match self {
                    #(#id_arms)*
                }
            }

            #doc_maturity
            pub const fn maturity(&self) -> Maturity {
                #maturity_body
            }

            #doc_ideally
            pub const fn ideally_enabled(&self) -> bool {
                #ideally_body
            }

            #doc_iso
            pub const fn iso_id(&self) -> Option<&'static str> {
                #iso_body
            }

            #doc_metadata
            pub const fn metadata(&self) -> FeatureMetadata {
                FeatureMetadata {
                    feature: *self,
                    id: self.id(),
                    iso_id: self.iso_id(),
                    ideally_enabled: self.ideally_enabled(),
                    maturity: self.maturity(),
                }
            }
        }

        #doc_features
        pub const FEATURES: &[Feature] = &Feature::ALL;

        #doc_feature_metadata
        pub const FEATURE_METADATA: &[FeatureMetadata] = &[
            #(#metadata_entries)*
        ];
    };

    let file =
        syn::parse2::<syn::File>(tokens).expect("generated feature-set source parses as a file");
    format!(
        "{}{HEADER}{}",
        crate::license_header::block(crate::license_header::Comment::Slash),
        prettyplease::unparse(&file)
    )
}

/// Read the `FeatureSet` struct field idents/types from the AST source, then join
/// each with its annotation row, enforcing a strict bijection in both directions.
fn resolve_fields() -> Vec<ResolvedField> {
    let struct_fields = feature_set_struct_fields();
    let resolved: Vec<ResolvedField> = struct_fields
        .into_iter()
        .map(|(ident, ty)| {
            let name = ident.to_string();
            let meta = FEATURE_FIELDS
                .iter()
                .find(|row| row.field == name)
                .unwrap_or_else(|| {
                    panic!(
                        "FeatureSet field `{name}` has no row in FEATURE_FIELDS \
                         (crates/squonk-sourcegen/src/feature_set.rs); add its \
                         source-of-truth annotation so the delta/registry can generate",
                    )
                });
            ResolvedField { ident, ty, meta }
        })
        .collect();

    // No orphan annotation rows: a row whose field was renamed or removed from the
    // struct would otherwise generate a phantom registry entry.
    for row in FEATURE_FIELDS {
        assert!(
            resolved.iter().any(|f| f.meta.field == row.field),
            "FEATURE_FIELDS row `{}` has no matching FeatureSet struct field; \
             remove it or restore the field",
            row.field,
        );
    }
    resolved
}

/// Parse `dialect/mod.rs` and return the `FeatureSet` struct's named fields in
/// declaration order. Read as text so generation never depends on the crate it
/// feeds (ADR-0013), exactly like [`crate::schema::Schema::load`] reads nodes.
fn feature_set_struct_fields() -> Vec<(syn::Ident, Type)> {
    let path = crate::dialect_src_path();
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let file =
        syn::parse_file(&text).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()));
    for item in file.items {
        let Item::Struct(item) = item else { continue };
        if item.ident != "FeatureSet" {
            continue;
        }
        let Fields::Named(named) = item.fields else {
            panic!("`FeatureSet` must be a struct with named fields");
        };
        return named
            .named
            .into_iter()
            .map(|field| (field.ident.expect("FeatureSet field is named"), field.ty))
            .collect();
    }
    panic!("`struct FeatureSet` not found in {}", path.display());
}

/// The `Feature` variant ident for a resolved field.
fn variant_ident(field: &ResolvedField) -> syn::Ident {
    format_ident!("{}", field.meta.variant)
}

/// One `#[doc]` attribute. A non-empty line gets a leading space so prettyplease
/// renders the idiomatic `/// text`; an empty line stays a bare `///`.
fn doc_attr(text: &str) -> TokenStream {
    let text = if text.is_empty() {
        String::new()
    } else {
        format!(" {text}")
    };
    quote! { #[doc = #text] }
}

/// A run of `#[doc]` attributes, one per line of a multi-line comment.
fn doc_lines(lines: &[&str]) -> TokenStream {
    let attrs = lines.iter().map(|line| doc_attr(line));
    quote! { #(#attrs)* }
}

/// Emit a const-fn body that is a bare expression when every field shares the same
/// value (the common case today), or a `match self` over variants when they
/// differ — so a future per-field maturity/polarity needs no generator change.
fn uniform_or_match<F>(fields: &[ResolvedField], body: F) -> TokenStream
where
    F: Fn(&ResolvedField) -> TokenStream,
{
    let arms: Vec<(syn::Ident, TokenStream)> = fields
        .iter()
        .map(|field| (variant_ident(field), body(field)))
        .collect();
    let distinct: BTreeSet<String> = arms.iter().map(|(_, expr)| expr.to_string()).collect();
    if distinct.len() <= 1 {
        arms.into_iter()
            .next()
            .map(|(_, expr)| expr)
            .expect("FeatureSet has at least one field")
    } else {
        let arms = arms
            .into_iter()
            .map(|(variant, expr)| quote! { Self::#variant => #expr, });
        quote! { match self { #(#arms)* } }
    }
}

/// Emit the `iso_id` body: only the anchored variants plus a `None` fallback, or a
/// bare `None` when nothing anchors (keeps the output minimal as the table grows).
fn iso_id_body(fields: &[ResolvedField]) -> TokenStream {
    let arms: Vec<TokenStream> = fields
        .iter()
        .filter_map(|field| {
            field.meta.iso_id.map(|iso| {
                let variant = variant_ident(field);
                quote! { Self::#variant => Some(#iso), }
            })
        })
        .collect();
    if arms.is_empty() {
        quote! { None }
    } else {
        quote! { match self { #(#arms)* _ => None, } }
    }
}

/// Prepended verbatim so the `//!` banner survives (token streams carry no line
/// comments) and the regeneration command stays one copy-paste away.
const HEADER: &str = "\
//! @generated by the `squonk-sourcegen` xtask — do not edit by hand.
//!
//! `FeatureDelta`, `FeatureSet::with`, and the
//! `Feature` / `FeatureMetadata` registry, generated from the hand-written
//! `FeatureSet` struct in `crates/squonk-ast/src/dialect/mod.rs` plus the
//! `FEATURE_FIELDS` annotation in `crates/squonk-sourcegen/src/feature_set.rs`
//! (drift gate; dialect-as-data). Regenerate after changing a
//! `FeatureSet` field: `cargo run -p squonk-sourcegen`.

#![allow(clippy::all)]

";
