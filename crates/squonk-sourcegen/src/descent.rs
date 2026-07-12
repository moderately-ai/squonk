// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The shared field-descent engine behind the generated visitors and the
//! render-shape skeleton.
//!
//! `visit.rs` and `render_skeleton.rs` both walk every schema item's fields and
//! emit per-field traversal code. The emission differs only at the seams this
//! engine parameterizes: what a classified leaf call looks like, what an
//! unclassifiable field falls back to, how a unit struct's body reads, and —
//! for the mutable visitor — how containers unwrap. Everything else (struct
//! destructuring, enum arms, `Option`/iterator nesting, binding names) must
//! stay emission-identical between the two generators, so it lives here once:
//! a future [`WalkKind`] variant is taught to one engine, not two.
//! (`node_id_walk.rs` avoids the problem differently, by delegating to the
//! generated walks themselves.)

use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Fields, ItemEnum, ItemStruct, Type, Variant};

use crate::schema::{
    NodeItem, Schema, WalkKind, WalkTarget, classify_walk, extension_generics, type_references_any,
};

/// Whether generated container traversal borrows shared or mutable.
#[derive(Clone, Copy)]
pub(crate) enum Mutability {
    Shared,
    Mutable,
}

/// One generator's parameterization of the shared descent.
pub(crate) struct DescentEmitter<'a> {
    /// How containers unwrap and iterate: `as_ref`/`iter` under
    /// [`Mutability::Shared`], `as_mut`/`iter_mut` under
    /// [`Mutability::Mutable`].
    pub mutability: Mutability,
    /// Emission for a classified [`WalkKind::Direct`] leaf (a visitor method
    /// call, or a `render_shape_*` call).
    pub leaf: &'a dyn Fn(WalkTarget, TokenStream) -> TokenStream,
    /// Emission for a field that references no walkable node.
    pub fallback: &'a dyn Fn(TokenStream) -> TokenStream,
    /// The generator's name in the cannot-walk panic. Generation-time only —
    /// it never reaches the generated output, so it cannot affect the
    /// byte-identity contract.
    pub context: &'a str,
    /// Body emitted for a unit struct (the visitor rebinds the name; the
    /// skeleton touches the node).
    pub unit_struct: &'a dyn Fn(&syn::Ident) -> TokenStream,
}

impl DescentEmitter<'_> {
    /// The walk body for one schema item (the content inside the generated fn).
    pub(crate) fn item_body(&self, item: &NodeItem, schema: &Schema) -> TokenStream {
        match item {
            NodeItem::Struct(s) => self.struct_body(s, schema),
            NodeItem::Enum(e) => self.enum_body(e, schema),
        }
    }

    fn struct_body(&self, s: &ItemStruct, schema: &Schema) -> TokenStream {
        let name = &s.ident;
        let generics = extension_generics(&s.generics);
        match &s.fields {
            Fields::Named(fields) => {
                let bindings = fields
                    .named
                    .iter()
                    .map(|field| field.ident.as_ref().expect("named field has an identifier"))
                    .collect::<Vec<_>>();
                let stmts = fields.named.iter().map(|field| {
                    let ident = field.ident.as_ref().expect("named field has an identifier");
                    self.field_stmt(quote! { #ident }, &field.ty, schema, &generics)
                });
                quote! {
                    let #name { #(#bindings),* } = node;
                    #(#stmts)*
                }
            }
            Fields::Unnamed(fields) => {
                let bindings = (0..fields.unnamed.len())
                    .map(|index| format_ident!("field{index}"))
                    .collect::<Vec<_>>();
                let stmts = fields.unnamed.iter().enumerate().map(|(index, field)| {
                    let binding = &bindings[index];
                    self.field_stmt(quote! { #binding }, &field.ty, schema, &generics)
                });
                quote! {
                    let #name(#(#bindings),*) = node;
                    #(#stmts)*
                }
            }
            Fields::Unit => (self.unit_struct)(name),
        }
    }

    fn enum_body(&self, e: &ItemEnum, schema: &Schema) -> TokenStream {
        if e.variants.is_empty() {
            return quote! {
                match *node {}
            };
        }

        let name = &e.ident;
        let generics = extension_generics(&e.generics);
        let arms = e
            .variants
            .iter()
            .map(|variant| self.enum_arm(name, variant, schema, &generics));
        quote! {
            match node {
                #(#arms)*
            }
        }
    }

    fn enum_arm(
        &self,
        enum_name: &syn::Ident,
        variant: &Variant,
        schema: &Schema,
        generics: &BTreeSet<String>,
    ) -> TokenStream {
        let vname = &variant.ident;
        match &variant.fields {
            Fields::Unit => quote! { #enum_name::#vname => {} },
            Fields::Unnamed(fields) => {
                let bindings = (0..fields.unnamed.len())
                    .map(|index| format_ident!("field{index}"))
                    .collect::<Vec<_>>();
                let stmts = fields.unnamed.iter().enumerate().map(|(index, field)| {
                    let binding = &bindings[index];
                    self.field_stmt(quote! { #binding }, &field.ty, schema, generics)
                });
                quote! {
                    #enum_name::#vname(#(#bindings),*) => {
                        #(#stmts)*
                    }
                }
            }
            Fields::Named(fields) => {
                let bindings = fields
                    .named
                    .iter()
                    .map(|field| field.ident.as_ref().expect("named field has an identifier"))
                    .collect::<Vec<_>>();
                let stmts = fields.named.iter().map(|field| {
                    let ident = field.ident.as_ref().expect("named field has an identifier");
                    self.field_stmt(quote! { #ident }, &field.ty, schema, generics)
                });
                quote! {
                    #enum_name::#vname { #(#bindings),* } => {
                        #(#stmts)*
                    }
                }
            }
        }
    }

    fn field_stmt(
        &self,
        accessor: TokenStream,
        ty: &Type,
        schema: &Schema,
        generics: &BTreeSet<String>,
    ) -> TokenStream {
        match classify_walk(ty, schema, generics) {
            Some(kind) => self.emit_walk_kind(&kind, accessor, 0),
            None => {
                assert!(
                    !type_references_any(ty, &schema.item_names, generics),
                    "{}: a field of type `{}` references an AST node but \
                     cannot be walked; extend `classify_walk` in sourcegen/schema.rs",
                    self.context,
                    ty.to_token_stream(),
                );
                (self.fallback)(accessor)
            }
        }
    }

    /// Emit the (possibly nested) traversal for one classified field.
    ///
    /// `depth` keeps each container level's binding distinct (`item`, `item1`, …)
    /// so nested containers such as `Vec<Vec<Expr>>` walk every element.
    fn emit_walk_kind(&self, kind: &WalkKind, accessor: TokenStream, depth: usize) -> TokenStream {
        match kind {
            WalkKind::Direct(target) => (self.leaf)(target.clone(), accessor),
            WalkKind::Option(inner) => {
                let binding = level_binding(depth);
                let call = self.emit_walk_kind(inner, quote! { #binding }, depth + 1);
                let unwrap = match self.mutability {
                    Mutability::Shared => quote! { as_ref },
                    Mutability::Mutable => quote! { as_mut },
                };
                quote! {
                    if let Some(#binding) = #accessor.#unwrap() {
                        #call
                    }
                }
            }
            WalkKind::Iter(inner) => {
                let binding = level_binding(depth);
                let call = self.emit_walk_kind(inner, quote! { #binding }, depth + 1);
                let iter = match self.mutability {
                    Mutability::Shared => quote! { iter },
                    Mutability::Mutable => quote! { iter_mut },
                };
                quote! {
                    for #binding in #accessor.#iter() {
                        #call
                    }
                }
            }
        }
    }
}

/// The loop / `Option` binding name at container nesting `depth`.
fn level_binding(depth: usize) -> syn::Ident {
    if depth == 0 {
        format_ident!("item")
    } else {
        format_ident!("item{depth}")
    }
}

/// The `Name<T…>` type expression for a schema item, generics included.
pub(crate) fn item_ty(item: &NodeItem) -> TokenStream {
    match item {
        NodeItem::Struct(s) => {
            let name = &s.ident;
            let (_, ty_generics, _) = s.generics.split_for_impl();
            quote! { #name #ty_generics }
        }
        NodeItem::Enum(e) => {
            let name = &e.ident;
            let (_, ty_generics, _) = e.generics.split_for_impl();
            quote! { #name #ty_generics }
        }
    }
}
