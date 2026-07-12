// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The dynamic extension escape hatch: [`DynExt`].
//!
//! The stock extension seam is the *generic, typed* one: a node parameterized by
//! `X: Extension` ([`Statement<X>`](crate::ast::Statement), [`Expr<X>`](crate::ast::Expr),
//! …) monomorphizes to one concrete extension type chosen at compile time. That is
//! the zero-cost default — `NoExt` makes the `Other` variant statically dead, and a
//! concrete `X` is inlined with no indirection.
//!
//! This module adds the *opt-in* alternative for callers who must compose an
//! extension set at **run time** — a plugin host, say, that loads several unrelated
//! custom-node kinds and cannot name a single `X` enum that closes over all of
//! them. Such a caller uses [`DynExt`] as their `X`: one type-erased node that any
//! number of concrete extension kinds can inhabit, selected dynamically. Nothing
//! about the static paths changes — `Statement<NoExt>` and `Statement<MyEnum>` keep
//! their exact layout and codegen; `DynExt` is a distinct instantiation you pay for
//! only where you write it.
//!
//! # Why a facet, not `Box<dyn Extension>`
//!
//! [`Extension`] is *not* object-safe, so `Box<dyn Extension>`
//! cannot exist. Its supertraits each break dyn-compatibility for a different
//! reason: `Clone` returns `Self` (and requires `Sized`), `PartialEq`/`Eq` take
//! `Self` by reference in argument position, and `Hash::hash` is generic over the
//! `Hasher`. A trait object erases the concrete type, so none of those signatures
//! can be dispatched through a vtable.
//!
//! [`DynAstExt`] is the standard dyn-compatible *facet* of that obligation set
//! (mirroring the `dyn`-wrapper idiom std uses for `Error`/`Any`): every
//! non-object-safe method is re-expressed as an object-safe shim — `dyn_clone`
//! returns a fresh box, `dyn_eq` takes an erased `&dyn DynAstExt` and downcasts,
//! `dyn_hash` drives a `&mut dyn Hasher`. [`Render`] and [`Spanned`] are *already*
//! object-safe, so they ride along as supertraits and need no shim. A blanket impl
//! lifts every `T: Extension + Render + 'static` into a `DynAstExt`.
//!
//! # Why the newtype, not a bare `Box<dyn DynAstExt>`
//!
//! It is tempting to make `Box<dyn DynAstExt>` *itself* the `X` by implementing
//! `Clone`/`Eq`/`Hash` on it. That compiles, but it does **not** compose with the
//! `#[derive(PartialEq)]`/`Clone`/`Hash` the node types use: a derived
//! `self.ext == other.ext` over a `Box<dyn Trait>` field *moves* the box out of the
//! shared `&other` instead of borrowing it (the `==` operator lowers through `Box`'s
//! `Deref`, and a user `PartialEq` impl on the box does not get the borrow the std
//! blanket gets), so `Statement<Box<dyn DynAstExt>>` fails to derive `PartialEq` —
//! the box is then not a drop-in `X` at all. Wrapping the box in the `Sized` newtype
//! [`DynExt`], whose own `PartialEq`/`Hash`/`Clone` are hand-written to borrow the
//! inner box explicitly, restores composition: a derived `self.ext == other.ext`
//! over a `DynExt` field borrows like any ordinary field. So `DynExt` — not the bare
//! box — is the public hatch, and it slots into every node and render/visit site.
//!
//! # Equality and hashing of erased nodes
//!
//! `dyn_eq` compares by *concrete type then value*: two nodes are equal exactly when
//! they hold the same underlying type and that type's `PartialEq` deems the payloads
//! equal; differently-typed nodes are never equal. This is the only rule consistent
//! with the structural `Eq`/`Hash` the rest of the AST derives, and it is not
//! optional — `Eq` and `Hash` are *load-bearing* for the [`Extension`] bound, so
//! without them `DynExt` could not be an `X` at all. `dyn_hash` mixes the [`TypeId`]
//! before the payload hash so the hash stays consistent with that type-discriminating
//! equality.
//!
//! # Trade-off: thread-safety
//!
//! `DynExt` wraps `Box<dyn DynAstExt + 'static>`; it is deliberately *not*
//! `Send`/`Sync`, so a [`Parsed`](crate::ast) carrying it forgoes the stock root's
//! `Send + Sync`. That is inherent to an unbounded trait object and is
//! the price of run-time composition; a `Send + Sync` variant would need a second,
//! separately-named bound and is out of scope until a caller needs it.
//!
//! [`Extension`]: crate::ast::Extension

use std::any::{Any, TypeId};
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::ast::{Extension, Spanned};
use crate::precedence::BindingPower;
use crate::vocab::Span;

use super::{Render, RenderCtx};

/// Object-safe facet of [`Extension`] `+` [`Render`], so a
/// runtime-composed extension set can be erased behind [`DynExt`].
///
/// This trait is rarely named directly and almost never implemented by hand: the
/// blanket impl covers every `T: Extension + Render + 'static`, and callers
/// interact with the erased node through [`DynExt`] and its ordinary `Clone`/
/// `PartialEq`/`Hash`/`Spanned`/`Render` impls. The one method worth calling
/// directly is [`as_any`](DynAstExt::as_any), to downcast an erased node back to a
/// concrete type (or use [`DynExt::downcast_ref`]).
///
/// `Render` and `Spanned` are supertraits (both are already object-safe), so a
/// `&dyn DynAstExt` can be rendered and span-queried straight through the vtable.
pub trait DynAstExt: Render + Spanned + fmt::Debug {
    /// Erase to `&dyn Any` for downcasting a node back to its concrete type.
    fn as_any(&self) -> &dyn Any;

    /// Clone into a fresh box — the object-safe stand-in for `Clone` (whose
    /// `Self`-returning signature cannot go through a vtable).
    fn dyn_clone(&self) -> Box<dyn DynAstExt>;

    /// Structural equality against another erased node — the object-safe stand-in
    /// for `PartialEq` (whose `&Self` argument cannot go through a vtable). Equal
    /// iff `other` holds the same concrete type and that type deems the values
    /// equal; differently-typed nodes are never equal.
    fn dyn_eq(&self, other: &dyn DynAstExt) -> bool;

    /// Feed this node's hash into an erased hasher — the object-safe stand-in for
    /// `Hash::hash` (whose generic `H: Hasher` cannot go through a vtable).
    fn dyn_hash(&self, state: &mut dyn Hasher);
}

impl<T: Extension + Render + 'static> DynAstExt for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dyn_clone(&self) -> Box<dyn DynAstExt> {
        Box::new(self.clone())
    }

    fn dyn_eq(&self, other: &dyn DynAstExt) -> bool {
        // Downcast to `Self`; a different concrete type can never be equal. This is
        // what makes erased equality match the typed path: `Other(a) == Other(b)`
        // holds exactly when `a` and `b` are the same node, same as the derived
        // `PartialEq` on a static `X`.
        other
            .as_any()
            .downcast_ref::<T>()
            .is_some_and(|other| self == other)
    }

    fn dyn_hash(&self, mut state: &mut dyn Hasher) {
        // `&mut dyn Hasher: Hasher` (std's `impl Hasher for &mut H`), so the concrete
        // `Hash` impl drives the type-erased hasher directly. Mixing the `TypeId`
        // first keeps `Hash` consistent with the type-discriminating `dyn_eq`: two
        // different extension types that happen to hash their payloads identically
        // still (almost surely) land on different hashes, never colliding as equal.
        TypeId::of::<T>().hash(&mut state);
        self.hash(&mut state);
    }
}

/// A type-erased extension node — the opt-in `X` for runtime-composed extension
/// sets.
///
/// `DynExt` reconstructs the full `Extension + Render` surface from the object-safe
/// [`DynAstExt`] shims, so it drops into any node (`Statement<DynExt>`,
/// `Expr<DynExt>`, …) and every render/visit site exactly where `NoExt` or a
/// concrete `X` would go — while the static paths keep their zero-cost layout. The
/// newtype (rather than a bare `Box<dyn DynAstExt>`) is what lets the node types'
/// derived `PartialEq`/`Hash`/`Clone` compose; see the module docs.
///
/// Build one with [`new`](DynExt::new) from any concrete extension node, and recover
/// the concrete type with [`downcast_ref`](DynExt::downcast_ref):
///
/// ```
/// use squonk_ast::render::{DynExt, Render, RenderCtx};
/// use squonk_ast::{Span, Spanned};
/// use std::fmt;
///
/// #[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// struct MyNode(u32);
/// impl Spanned for MyNode {
///     fn span(&self) -> Span { Span::SYNTHETIC }
/// }
/// impl Render for MyNode {
///     fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "my({})", self.0)
///     }
/// }
///
/// let ext = DynExt::new(MyNode(7));
/// assert_eq!(ext.downcast_ref::<MyNode>(), Some(&MyNode(7)));
/// assert_eq!(ext.clone(), ext);
/// ```
pub struct DynExt(Box<dyn DynAstExt>);

impl DynExt {
    /// Erase a concrete extension node into the dynamic hatch.
    pub fn new<T: Extension + Render + 'static>(ext: T) -> Self {
        DynExt(Box::new(ext))
    }

    /// Recover the concrete extension type, or `None` if this node is some other type.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref::<T>()
    }
}

// The impls below rebuild the full `Extension + Render` surface on the `Sized`
// newtype from the object-safe shims. Each borrows the inner box explicitly (never a
// bare `==`/`Clone::clone` on `Box<dyn DynAstExt>`), so they compose with the node
// types' `#[derive(..)]` (see module docs) and impose no cost on the static paths.

impl Clone for DynExt {
    fn clone(&self) -> Self {
        DynExt(self.0.dyn_clone())
    }
}

impl fmt::Debug for DynExt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Transparent, mirroring the typed path where `Other { ext }` debugs as the
        // inner node rather than as a wrapper.
        fmt::Debug::fmt(&self.0, f)
    }
}

impl PartialEq for DynExt {
    fn eq(&self, other: &Self) -> bool {
        self.0.dyn_eq(&*other.0)
    }
}

impl Eq for DynExt {}

impl Hash for DynExt {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.dyn_hash(state);
    }
}

impl Spanned for DynExt {
    fn span(&self) -> Span {
        self.0.span()
    }
}

impl Render for DynExt {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.render(ctx, f)
    }

    fn operand_binding_power(&self) -> Option<BindingPower> {
        // Forward so a *dynamic* extension operator parenthesizes by the same
        // binding-power rule as a typed one (ADR-0008/0009).
        self.0.operand_binding_power()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOperator, Expr, NoExt, Statement};
    use crate::generated::visit::{self, Visit};
    use crate::render::{RenderConfig, RenderCtx, RenderExt as _};
    use crate::vocab::{Meta, NodeId, Resolver, Span, Symbol};

    fn meta() -> Meta {
        Meta::new(Span::SYNTHETIC, NodeId::new(1).expect("non-zero node id"))
    }

    /// A trivial renderable extension node: renders as `#<n>`. Used erased to prove
    /// the dynamic hatch carries a real, type-erased extension.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct Tag(u32);

    impl Spanned for Tag {
        fn span(&self) -> Span {
            Span::SYNTHETIC
        }
    }

    impl Render for Tag {
        fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "#{}", self.0)
        }
    }

    /// A *second, unrelated* extension type, to show heterogeneous nodes coexisting
    /// behind one `DynExt` — the runtime-composition the hatch exists for.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct Marker;

    impl Spanned for Marker {
        fn span(&self) -> Span {
            Span::SYNTHETIC
        }
    }

    impl Render for Marker {
        fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("<marker>")
        }
    }

    fn other(ext: DynExt) -> Expr<DynExt> {
        Expr::Other { ext, meta: meta() }
    }

    fn rendered(node: &impl Render) -> String {
        // No identifiers/literals in these trees, so an empty resolver/source suffice.
        struct Empty;
        impl Resolver for Empty {
            fn try_resolve(&self, _sym: Symbol) -> Option<&str> {
                None
            }
        }
        let config = RenderConfig::default();
        let ctx = RenderCtx::new(&Empty, "", &config);
        node.displayed(&ctx).to_string()
    }

    #[test]
    fn erased_extension_renders_through_the_node_render_path() {
        // A `DynExt` drops straight into `Expr::Other` and renders via the ordinary
        // `impl<X: Extension + Render> Render for Expr<X>` seam.
        let expr = other(DynExt::new(Tag(7)));
        assert_eq!(rendered(&expr), "#7");
    }

    #[test]
    fn heterogeneous_erased_extensions_compose_in_one_tree() {
        // Two unrelated concrete types behind the same `X = DynExt`, in a single
        // built-in node — impossible on the typed path without a hand-written sum
        // type enumerating both. Each renders through its own vtable.
        let expr: Expr<DynExt> = Expr::BinaryOp {
            left: Box::new(other(DynExt::new(Tag(1)))),
            op: BinaryOperator::Plus,
            right: Box::new(other(DynExt::new(Marker))),
            meta: meta(),
        };
        assert_eq!(rendered(&expr), "#1 + <marker>");
    }

    #[test]
    fn node_holding_dynext_derives_eq_and_clone() {
        // The crux this design exists to make work: the node types' own
        // `#[derive(PartialEq, Eq, Clone)]` compose over a `DynExt` field. A bare
        // `Box<dyn DynAstExt>` field would *fail* to derive (see module docs).
        let expr = other(DynExt::new(Tag(7)));
        let same = other(DynExt::new(Tag(7)));
        let different = other(DynExt::new(Tag(8)));

        assert!(
            expr == same,
            "structurally equal erased nodes compare equal"
        );
        assert!(expr != different, "different payloads compare unequal");
        assert!(
            expr == expr.clone(),
            "a cloned subtree stays equal to its origin"
        );
    }

    #[test]
    fn erased_extension_equality_is_concrete_type_then_value() {
        let a = DynExt::new(Tag(1));
        let a2 = DynExt::new(Tag(1));
        let b = DynExt::new(Tag(2));
        let m = DynExt::new(Marker);

        assert!(a == a2, "same type, same value compares equal");
        assert!(a != b, "same type, different value compares unequal");
        assert!(a != m, "different concrete types are never equal");
    }

    #[test]
    fn erased_extension_clone_is_a_deep_typed_clone() {
        let original = DynExt::new(Tag(42));
        let clone = original.clone();
        assert!(original == clone);
        // The clone is a real `Tag`, recoverable by downcast — not some erased husk.
        assert_eq!(clone.downcast_ref::<Tag>(), Some(&Tag(42)));
    }

    #[test]
    fn erased_extension_hash_agrees_with_equality() {
        use std::collections::hash_map::DefaultHasher;

        fn hash_of(ext: &DynExt) -> u64 {
            let mut hasher = DefaultHasher::new();
            ext.hash(&mut hasher);
            hasher.finish()
        }

        // Equal values must hash equally (the `Hash`/`Eq` contract the AST relies on).
        assert_eq!(hash_of(&DynExt::new(Tag(1))), hash_of(&DynExt::new(Tag(1))));
    }

    #[test]
    fn visitor_threads_and_downcasts_erased_extensions() {
        // The generated `Visit` traversal needs no object-safety work: it hands the
        // visitor `&X`, and here `X = DynExt`, which a visitor can both count and
        // downcast — so existing tooling sees dynamic extensions for free.
        #[derive(Default)]
        struct Collect {
            tags: Vec<u32>,
            others: usize,
        }

        impl<'ast> Visit<'ast, DynExt> for Collect {
            fn visit_extension(&mut self, node: &'ast DynExt) {
                match node.downcast_ref::<Tag>() {
                    Some(Tag(n)) => self.tags.push(*n),
                    None => self.others += 1,
                }
            }
        }

        let expr: Expr<DynExt> = Expr::BinaryOp {
            left: Box::new(other(DynExt::new(Tag(1)))),
            op: BinaryOperator::Plus,
            right: Box::new(other(DynExt::new(Marker))),
            meta: meta(),
        };
        let mut collect = Collect::default();
        collect.visit_expr(&expr);

        assert_eq!(collect.tags, vec![1]);
        assert_eq!(collect.others, 1, "the non-Tag extension was still visited");
        let _ = visit::walk_expr::<Collect, DynExt>; // walk_* is generic over X
    }

    #[test]
    fn dynamic_hatch_does_not_change_the_static_noext_layout() {
        use std::mem::size_of;

        // The headline guarantee, pinned locally. The stock `NoExt` path is pinned
        // byte-for-byte by `crate::generated::size_asserts`; those budgets did not
        // move when this hatch was added (the generated file is unchanged). Here we
        // re-state the two invariants that make the hatch *opt-in*:
        //   1. the default type parameter is `NoExt`, so the hot path is `NoExt`;
        assert_eq!(size_of::<Statement<NoExt>>(), size_of::<Statement>());
        //   2. the erased extension is a strictly *wider*, distinct instantiation —
        //      its fat-pointer payload widens `Other`, so you pay only when you reach
        //      for it; the `NoExt` variant stays zero-width and dead.
        assert!(size_of::<Statement<NoExt>>() < size_of::<Statement<DynExt>>());
    }
}
