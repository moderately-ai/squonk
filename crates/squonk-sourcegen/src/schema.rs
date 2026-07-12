// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Parse the hand-written AST node types into a small schema the emitters walk.
//!
//! The node types are read as *text* (never by depending on the compiled
//! `squonk-ast` crate) so the generator can run before the AST compiles and
//! cannot be perturbed by the very traits it emits.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use syn::{
    Fields, GenericArgument, GenericParam, Generics, Item, ItemEnum, ItemStruct, PathArguments,
    PathSegment, Type, TypeParamBound, TypePath,
};

/// How a single spanned field contributes to a node's span.
#[derive(Clone, Copy)]
pub(crate) enum FieldKind {
    /// One spanned value (a node, `Box<node>`, `Ident`, or `Literal`): `x.span()`.
    Direct,
    /// A `Vec`/`Option` of spanned values, folded together with `Span::union`.
    Fold,
}

/// The generated visitors/render skeleton can walk either a known AST item or
/// the open extension parameter (`X`).
#[derive(Clone)]
pub(crate) enum WalkTarget {
    Node(String),
    Extension,
}

/// How a field containing a walkable target is shaped.
///
/// `Option`/`Iter` nest, so a multi-level container (e.g. the `Vec<Vec<Expr>>`
/// of `Values::rows`) walks every element instead of being silently skipped.
#[derive(Clone)]
pub(crate) enum WalkKind {
    Direct(WalkTarget),
    Option(Box<WalkKind>),
    Iter(Box<WalkKind>),
}

/// A struct or enum definition lifted from the AST source.
pub(crate) enum NodeItem {
    Struct(ItemStruct),
    Enum(ItemEnum),
}

impl NodeItem {
    fn ident(&self) -> &syn::Ident {
        match self {
            NodeItem::Struct(s) => &s.ident,
            NodeItem::Enum(e) => &e.ident,
        }
    }

    pub(crate) fn name(&self) -> String {
        self.ident().to_string()
    }

    pub(crate) fn generics(&self) -> &Generics {
        match self {
            NodeItem::Struct(s) => &s.generics,
            NodeItem::Enum(e) => &e.generics,
        }
    }
}

/// Every node definition plus the set of type names that carry a span.
pub(crate) struct Schema {
    /// Definitions in a deterministic order (sorted files, source order within).
    pub(crate) items: Vec<NodeItem>,
    /// The AST source-file stem (`expr`, `query`, `ddl`, …) each item was parsed
    /// from, keyed by item name. This is the render-shape fingerprint partition:
    /// sourcegen emits one fingerprint const per file so a shape change in
    /// `ast/<family>.rs` moves only that file's fingerprint, and two agents
    /// editing disjoint files touch disjoint pin lines (see `render_skeleton`).
    /// Type names are unique across the flattened `ast` module, so name is a key.
    pub(crate) families: BTreeMap<String, String>,
    /// Names of every struct/enum parsed from the hand-written AST source.
    pub(crate) item_names: BTreeSet<String>,
    /// Names of every type for which a `Spanned` impl is generated.
    pub(crate) spanned: BTreeSet<String>,
    /// Names of every type that is part of the AST node graph: a spanned node,
    /// a tag/leaf type referenced by one's field, or an extension default. Types
    /// outside this set (e.g. the `LiteralValueError` accessor-error types) are
    /// not AST nodes and get no generated walk.
    pub(crate) reachable: BTreeSet<String>,
}

impl Schema {
    /// Read and parse the AST source, then classify which types are spanned.
    pub(crate) fn load() -> Self {
        let mut items = Vec::new();
        let mut families = BTreeMap::new();
        for path in node_source_files() {
            let family = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("AST source file has a UTF-8 stem")
                .to_owned();
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
            let file = syn::parse_file(&text)
                .unwrap_or_else(|err| panic!("parse {}: {err}", path.display()));
            for item in file.items {
                let node = match item {
                    Item::Struct(s) => NodeItem::Struct(s),
                    Item::Enum(e) => NodeItem::Enum(e),
                    _ => continue,
                };
                families.insert(node.name(), family.clone());
                items.push(node);
            }
        }
        let item_names = items.iter().map(NodeItem::name).collect();
        let spanned = compute_spanned(&items);
        let reachable = compute_reachable(&items, &spanned, &item_names);
        Self {
            items,
            families,
            item_names,
            spanned,
            reachable,
        }
    }

    /// The AST source-file family (`expr`, `ddl`, …) an item belongs to — the
    /// unit sourcegen fingerprints as one render-shape const.
    pub(crate) fn family_of(&self, name: &str) -> &str {
        self.families
            .get(name)
            .map(String::as_str)
            .unwrap_or_else(|| panic!("item `{name}` has no recorded source family"))
    }

    pub(crate) fn is_spanned(&self, name: &str) -> bool {
        self.spanned.contains(name)
    }

    pub(crate) fn is_ast_item(&self, name: &str) -> bool {
        self.item_names.contains(name)
    }

    pub(crate) fn is_reachable(&self, name: &str) -> bool {
        self.reachable.contains(name)
    }
}

/// The `*.rs` files that define real nodes, in a stable order.
///
/// `mod.rs` only re-exports and asserts enum sizes, and `tests.rs` holds decoy
/// mirror enums (`ExprWithoutOther`, …) used for size checks — neither defines a
/// node, so both are skipped to keep generation correct and deterministic.
fn node_source_files() -> Vec<PathBuf> {
    let dir = crate::ast_src_dir();
    let mut files = Vec::new();
    for entry in
        fs::read_dir(&dir).unwrap_or_else(|err| panic!("read dir {}: {err}", dir.display()))
    {
        let path = entry.expect("read AST source directory entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if stem == "mod" || stem == "tests" {
            continue;
        }
        files.push(path);
    }
    files.sort();
    files
}

/// Compute the set of spanned type names by fixpoint over the node graph.
///
/// Seeds: every struct with a `meta: Meta` field, every enum whose variants all
/// carry `meta: Meta`, plus `ObjectName` (the one documented node that wraps
/// `Ident`s instead of carrying `meta`). A spanned enum without variant metadata
/// is rejected: enum-level AST nodes must be addressable directly instead of
/// reconstructing their span from children.
fn compute_spanned(items: &[NodeItem]) -> BTreeSet<String> {
    let mut spanned = BTreeSet::new();
    for item in items {
        match item {
            NodeItem::Struct(s) if struct_has_meta(s) => {
                spanned.insert(s.ident.to_string());
            }
            NodeItem::Enum(e) if enum_has_meta(e) => {
                spanned.insert(e.ident.to_string());
            }
            NodeItem::Struct(_) | NodeItem::Enum(_) => {}
        }
    }
    // ObjectName is a thin qualified-name wrapper (`ObjectName(pub ThinVec<Ident>)`),
    // not a `meta` node; its span is the union of its identifier parts.
    spanned.insert("ObjectName".to_owned());

    for item in items {
        let NodeItem::Enum(e) = item else { continue };
        if spanned.contains(&e.ident.to_string()) {
            continue;
        }
        let generics = spanned_generics(&e.generics);
        let is_node = e.variants.iter().any(|variant| {
            variant
                .fields
                .iter()
                .any(|field| classify(&field.ty, &spanned, &generics).is_some())
        });
        if is_node {
            panic!(
                "enum `{}` is a spanned AST node because one of its variants \
                 references a spanned child, but not every variant has a \
                 named `meta: Meta` field",
                e.ident,
            );
        }
    }
    spanned
}

/// Compute the AST node graph: the set of types that should get generated walks.
///
/// Seeded with the spanned node set plus every extension default (`NoExt`, the
/// stock `X`) — a real part of the AST even though no *field* names it, since the
/// `Other(X)` seam walks it through the generic — then closed over field
/// references so tag/leaf enums (`BinaryOperator`, `DataType`, `LiteralKind`, …)
/// are pulled in. Types never reached this way (e.g. the `LiteralValueError`
/// accessor-error types) are not AST nodes and are deliberately excluded.
fn compute_reachable(
    items: &[NodeItem],
    spanned: &BTreeSet<String>,
    item_names: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut reachable = spanned.clone();
    for item in items {
        collect_generic_defaults(item.generics(), item_names, &mut reachable);
    }
    loop {
        let mut changed = false;
        for item in items {
            if !reachable.contains(&item.name()) {
                continue;
            }
            let mut refs = BTreeSet::new();
            collect_item_refs(item, item_names, &mut refs);
            for name in refs {
                if reachable.insert(name) {
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    reachable
}

/// Collect the AST-item names a generic parameter defaults to (e.g. `NoExt`).
fn collect_generic_defaults(
    generics: &Generics,
    item_names: &BTreeSet<String>,
    out: &mut BTreeSet<String>,
) {
    for param in &generics.params {
        if let GenericParam::Type(type_param) = param {
            if let Some(default) = &type_param.default {
                collect_type_items(default, item_names, out);
            }
        }
    }
}

/// Collect the AST-item names referenced by an item's fields/variant fields.
fn collect_item_refs(item: &NodeItem, item_names: &BTreeSet<String>, out: &mut BTreeSet<String>) {
    match item {
        NodeItem::Struct(s) => {
            for field in s.fields.iter() {
                collect_type_items(&field.ty, item_names, out);
            }
        }
        NodeItem::Enum(e) => {
            for variant in &e.variants {
                for field in variant.fields.iter() {
                    collect_type_items(&field.ty, item_names, out);
                }
            }
        }
    }
}

/// Insert every AST-item name appearing anywhere in `ty`'s type tree into `out`.
fn collect_type_items(ty: &Type, item_names: &BTreeSet<String>, out: &mut BTreeSet<String>) {
    match ty {
        Type::Path(TypePath { qself: None, path }) => {
            for segment in &path.segments {
                let name = segment.ident.to_string();
                if item_names.contains(&name) {
                    out.insert(name);
                }
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let GenericArgument::Type(inner) = arg {
                            collect_type_items(inner, item_names, out);
                        }
                    }
                }
            }
        }
        Type::Tuple(tuple) => {
            for elem in &tuple.elems {
                collect_type_items(elem, item_names, out);
            }
        }
        Type::Reference(reference) => collect_type_items(&reference.elem, item_names, out),
        Type::Array(array) => collect_type_items(&array.elem, item_names, out),
        Type::Slice(slice) => collect_type_items(&slice.elem, item_names, out),
        Type::Group(group) => collect_type_items(&group.elem, item_names, out),
        Type::Paren(paren) => collect_type_items(&paren.elem, item_names, out),
        _ => {}
    }
}

/// True when `s` has a `pub meta: Meta` field, marking it a struct node.
pub(crate) fn struct_has_meta(s: &ItemStruct) -> bool {
    let Fields::Named(named) = &s.fields else {
        return false;
    };
    named.named.iter().any(|field| {
        field.ident.as_ref().is_some_and(|ident| ident == "meta")
            && type_core_ident(&field.ty).as_deref() == Some("Meta")
    })
}

/// True when every variant has a named `meta: Meta` field, marking an enum node.
pub(crate) fn enum_has_meta(e: &ItemEnum) -> bool {
    !e.variants.is_empty()
        && e.variants.iter().all(|variant| {
            let Fields::Named(named) = &variant.fields else {
                return false;
            };
            named.named.iter().any(|field| {
                field.ident.as_ref().is_some_and(|ident| ident == "meta")
                    && type_core_ident(&field.ty).as_deref() == Some("Meta")
            })
        })
}

/// The type-parameter names bound by `Extension`/`Spanned`, hence span-bearing.
///
/// The node enums are generic over `<X: Extension = NoExt>`, and `Extension`
/// requires `Spanned`, so an `Other(X)` arm can delegate to `X::span()`.
pub(crate) fn spanned_generics(generics: &Generics) -> BTreeSet<String> {
    extension_generics(generics)
}

/// Type-parameter names that stand for open AST extension nodes.
pub(crate) fn extension_generics(generics: &Generics) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for param in &generics.params {
        let GenericParam::Type(type_param) = param else {
            continue;
        };
        let bounded_by_span = type_param.bounds.iter().any(|bound| {
            let TypeParamBound::Trait(trait_bound) = bound else {
                return false;
            };
            matches!(
                trait_bound
                    .path
                    .segments
                    .last()
                    .map(|seg| seg.ident.to_string())
                    .as_deref(),
                Some("Extension" | "Spanned"),
            )
        });
        if bounded_by_span {
            names.insert(type_param.ident.to_string());
        }
    }
    names
}

/// Classify a field type, or return `None` for a non-spanned field
/// (`bool`, operator/tag enums, `Symbol`, …).
pub(crate) fn classify(
    ty: &Type,
    spanned: &BTreeSet<String>,
    generics: &BTreeSet<String>,
) -> Option<FieldKind> {
    let segment = last_path_segment(ty)?;
    match segment.ident.to_string().as_str() {
        // `Box<node>` is transparent: `&Box<T>` auto-derefs for both `.span()`
        // and `.iter()`, so it classifies exactly as its inner type.
        "Box" => classify(single_type_arg(segment)?, spanned, generics),
        // `ThinVec` is our default child-sequence container (ADR-0007); it folds
        // into a span exactly like `Vec` — same `<T>` shape, same `.iter()`.
        "Vec" | "ThinVec" | "Option" => {
            single_type_arg(segment).filter(|inner| core_is_spanned(inner, spanned, generics))?;
            Some(FieldKind::Fold)
        }
        other => (spanned.contains(other) || generics.contains(other)).then_some(FieldKind::Direct),
    }
}

/// Classify a field for generated visitors/render skeletons.
///
/// This is broader than span classification: operator/data-type/tag enums are
/// included so generated output changes when any AST enum variant changes, and
/// non-walked fields are still destructured by the emitters.
pub(crate) fn classify_walk(
    ty: &Type,
    schema: &Schema,
    generics: &BTreeSet<String>,
) -> Option<WalkKind> {
    let segment = last_path_segment(ty)?;
    match segment.ident.to_string().as_str() {
        "Box" => classify_walk(single_type_arg(segment)?, schema, generics),
        "Vec" | "ThinVec" => classify_walk(single_type_arg(segment)?, schema, generics)
            .map(|inner| WalkKind::Iter(Box::new(inner))),
        "Option" => classify_walk(single_type_arg(segment)?, schema, generics)
            .map(|inner| WalkKind::Option(Box::new(inner))),
        other if schema.is_ast_item(other) => {
            Some(WalkKind::Direct(WalkTarget::Node(other.to_owned())))
        }
        other if generics.contains(other) => Some(WalkKind::Direct(WalkTarget::Extension)),
        _ => None,
    }
}

/// Whether a type's core (the last path segment, through `Box`) is spanned.
fn core_is_spanned(ty: &Type, spanned: &BTreeSet<String>, generics: &BTreeSet<String>) -> bool {
    let Some(segment) = last_path_segment(ty) else {
        return false;
    };
    let head = segment.ident.to_string();
    if head == "Box" {
        return single_type_arg(segment)
            .is_some_and(|inner| core_is_spanned(inner, spanned, generics));
    }
    spanned.contains(&head) || generics.contains(&head)
}

/// Whether `ty` mentions any name in `names` (AST items, or spanned types) or an
/// extension generic, anywhere in its type tree.
///
/// The walk/span classifiers only descend the container shapes they know how to
/// emit (`Box`/`Vec`/`Option`). This is the fail-loud backstop: if a field
/// references an AST node through a shape the classifier does not handle (a
/// tuple, array, map, …), generation panics instead of silently dropping the
/// child — the "empty-span / un-visited node hole" ADR-0013 calls impossible.
pub(crate) fn type_references_any(
    ty: &Type,
    names: &BTreeSet<String>,
    generics: &BTreeSet<String>,
) -> bool {
    match ty {
        Type::Path(TypePath { qself: None, path }) => path.segments.iter().any(|segment| {
            let name = segment.ident.to_string();
            names.contains(&name)
                || generics.contains(&name)
                || matches!(
                    &segment.arguments,
                    PathArguments::AngleBracketed(args)
                        if args.args.iter().any(|arg| matches!(
                            arg,
                            GenericArgument::Type(inner)
                                if type_references_any(inner, names, generics)
                        ))
                )
        }),
        Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .any(|elem| type_references_any(elem, names, generics)),
        Type::Reference(reference) => type_references_any(&reference.elem, names, generics),
        Type::Array(array) => type_references_any(&array.elem, names, generics),
        Type::Slice(slice) => type_references_any(&slice.elem, names, generics),
        Type::Group(group) => type_references_any(&group.elem, names, generics),
        Type::Paren(paren) => type_references_any(&paren.elem, names, generics),
        _ => false,
    }
}

/// The last segment of a plain (non-`Self`-qualified) path type.
fn last_path_segment(ty: &Type) -> Option<&PathSegment> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    path.segments.last()
}

/// The first `<T>` type argument of a path segment (e.g. the `T` in `Vec<T>`).
fn single_type_arg(segment: &PathSegment) -> Option<&Type> {
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

/// The identifier of a path type's last segment, if it is a plain path.
fn type_core_ident(ty: &Type) -> Option<String> {
    last_path_segment(ty).map(|segment| segment.ident.to_string())
}

/// Convert a Rust type/variant name to a snake-case method suffix.
pub(crate) fn snake_case(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower_or_digit = false;
    let chars = name.chars().collect::<Vec<_>>();
    for (i, ch) in chars.iter().copied().enumerate() {
        let next_lower = chars
            .get(i + 1)
            .is_some_and(|next| next.is_ascii_lowercase());
        if ch.is_ascii_uppercase() {
            if i > 0 && (prev_lower_or_digit || next_lower) {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower_or_digit = false;
        } else {
            out.push(ch);
            prev_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    out
}

/// Convert a Rust type name to a screaming-snake constant prefix.
pub(crate) fn screaming_snake_case(name: &str) -> String {
    snake_case(name).to_ascii_uppercase()
}
