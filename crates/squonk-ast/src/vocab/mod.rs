// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Foundational AST vocabulary shared by parser, renderer, and consumers.
//!
//! These types keep the AST dependency-free and lifetime-free: nodes carry byte
//! ranges, interned symbols, and side-table identity, while the parsed root owns
//! source text and resolver state.

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU32;
use std::ops::Deref;

/// A half-open byte range in the original source, `[start, end)`.
///
/// Spans store byte offsets, not line/column pairs. Line and byte-column
/// coordinates are recovered lazily through [`LineIndex`] so the common AST path
/// pays only for compact ranges.
// Spans serialize as their two byte offsets (ADR-0001 byte-range model). The
// derive builds the struct field-by-field on deserialize, bypassing `new`'s
// `start <= end` assert, so the `SYNTHETIC` sentinel (`start = u32::MAX, end = 0`)
// round-trips exactly rather than panicking.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Span {
    start: u32,
    end: u32,
}

impl Span {
    /// Sentinel for nodes synthesized by rewrites rather than parsed from text.
    ///
    /// The value is intentionally not a valid half-open range, which makes it
    /// hard to confuse with a real empty span from the source.
    pub const SYNTHETIC: Self = Self {
        start: u32::MAX,
        end: 0,
    };

    /// Create a source-backed span.
    ///
    /// # Panics
    ///
    /// Panics if `start > end`; synthetic spans must use [`Span::SYNTHETIC`].
    pub const fn new(start: u32, end: u32) -> Self {
        assert!(start <= end, "span start must be <= span end");
        Self { start, end }
    }

    /// Return the inclusive start byte offset.
    pub const fn start(&self) -> u32 {
        self.start
    }

    /// Return the exclusive end byte offset.
    pub const fn end(&self) -> u32 {
        self.end
    }

    /// Return the byte length of this source span.
    ///
    /// Synthetic spans have no source text, so they report length zero without
    /// pretending to be a real empty source range.
    pub const fn len(&self) -> u32 {
        if self.is_synthetic() {
            0
        } else {
            self.end - self.start
        }
    }

    /// Return true if this source span covers `offset`.
    pub const fn contains(&self, offset: u32) -> bool {
        !self.is_synthetic() && self.start <= offset && offset < self.end
    }

    /// Return true for real zero-width source spans.
    pub const fn is_empty(&self) -> bool {
        !self.is_synthetic() && self.start == self.end
    }

    /// Return the smallest source span covering both spans.
    ///
    /// Synthetic spans are identity values for unions because generated nodes
    /// should not erase the known source extent of parsed children.
    pub const fn union(&self, other: Self) -> Self {
        if self.is_synthetic() {
            other
        } else if other.is_synthetic() {
            *self
        } else {
            Self {
                start: if self.start < other.start {
                    self.start
                } else {
                    other.start
                },
                end: if self.end > other.end {
                    self.end
                } else {
                    other.end
                },
            }
        }
    }

    /// Return true if this span was synthesized and has no backing source text.
    pub const fn is_synthetic(&self) -> bool {
        self.start == u32::MAX && self.end == 0
    }
}

/// Interned identifier text.
///
/// A `Symbol` is only comparable within the resolver/interner that created it;
/// the same numeric value can mean different strings in another parse. This type
/// intentionally does not implement `Ord`: numeric order is interning order, not
/// lexicographic order. Resolve symbols to `&str` before sorted output.
///
/// # Serialization (`serde` feature)
///
/// A bare `Symbol` serializes as its **numeric interner key**, not its text — the
/// AST crate has no resolver, so it cannot do otherwise. That number is meaningful
/// ONLY alongside the exact resolver that produced it (same parse, same process);
/// it is NOT portable across parses. To serialize a portable, self-contained
/// document, serialize the owning `Parsed` root instead (in the `squonk`
/// crate): it co-serializes the resolver's string table and re-interns on load, so
/// symbols round-trip to the same text across processes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Symbol(NonZeroU32);

impl Symbol {
    /// Create a symbol from the resolver's one-based storage key.
    pub const fn new(raw: u32) -> Option<Self> {
        match NonZeroU32::new(raw) {
            Some(raw) => Some(Self(raw)),
            None => None,
        }
    }

    /// Return the resolver's one-based storage key.
    pub const fn as_u32(&self) -> u32 {
        self.0.get()
    }

    /// Return the zero-based resolver index.
    pub const fn index(&self) -> usize {
        (self.0.get() - 1) as usize
    }
}

/// Opt-in, dialect case-insensitive ("folded") identity for an interned [`Symbol`].
///
/// The canonical AST identity is the exact-case `Symbol`, which preserves source
/// spelling for round-trip rendering. A `FoldedSymbol` is its opt-in
/// counterpart: two `Symbol`s share a `FoldedSymbol` exactly when their text is
/// equal under the dialect's [`Casing`](crate::dialect::Casing). It is produced
/// only by an out-of-tree folded side table and is never stored in the AST, so
/// default structural equality — and thus round-trip fidelity — is unaffected.
///
/// Like [`Symbol`], a `FoldedSymbol` is only comparable within the side table that
/// created it, and it intentionally does not implement `Ord`: numeric order is
/// assignment order, not lexicographic order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FoldedSymbol(NonZeroU32);

impl FoldedSymbol {
    /// Create a folded symbol from the side table's one-based key.
    pub const fn new(raw: u32) -> Option<Self> {
        match NonZeroU32::new(raw) {
            Some(raw) => Some(Self(raw)),
            None => None,
        }
    }

    /// Return the side table's one-based key.
    pub const fn as_u32(&self) -> u32 {
        self.0.get()
    }
}

/// Per-parse node identity for out-of-tree side tables.
///
/// `NodeId` is not structural identity. It is assigned by the parser's per-parse
/// counter and intentionally disappears from derived equality through [`Meta`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct NodeId(NonZeroU32);

impl NodeId {
    /// Create a node id from the parser's one-based counter value.
    pub const fn new(raw: u32) -> Option<Self> {
        match NonZeroU32::new(raw) {
            Some(raw) => Some(Self(raw)),
            None => None,
        }
    }

    /// Return the parser's one-based counter value.
    pub const fn as_u32(&self) -> u32 {
        self.0.get()
    }
}

/// Metadata carried by every AST node.
///
/// This wrapper is always equal and contributes nothing to hashing or ordering.
/// That makes ordinary derives on AST nodes structural by default: semantic
/// fields participate, while source location and side-table identity do not.
///
/// Under the `serde` feature both fields round-trip verbatim: the `span` because
/// byte offsets are the portable location model, and the `node_id` because keeping
/// it round-trips exactly while remaining structural-equality-neutral (a re-derived
/// placeholder would be strictly worse — non-unique — with no gain). A `node_id`
/// read back from another process is still per-parse identity; a consumer building
/// fresh side tables reassigns ids from its own counter, exactly as after a parse.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Meta {
    /// Source range for diagnostics, slicing, and rendering context.
    pub span: Span,
    /// Side-table key assigned by the parser.
    pub node_id: NodeId,
}

impl Meta {
    /// Create metadata for a parsed or synthesized node.
    pub const fn new(span: Span, node_id: NodeId) -> Self {
        Self { span, node_id }
    }
}

impl PartialEq for Meta {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for Meta {}

impl Hash for Meta {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl PartialOrd for Meta {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Meta {
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}

/// Lazy source line index for byte-offset to `(line, column)` lookup.
///
/// Lines and columns are zero-based byte positions. UTF-16/editor columns are a
/// rendering concern layered on top of this byte-accurate index.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineIndex {
    newline_offsets: Vec<u32>,
}

impl LineIndex {
    /// Scan source text for newline byte offsets.
    ///
    /// The scan stays dependency-free in this foundational crate; faster search
    /// can be revisited when a ticket is allowed to change crate dependencies.
    // The ticket makes this the ergonomic constructor; `FromStr` is also
    // implemented for trait users, but its `Result` return is noise here.
    #[expect(
        clippy::should_implement_trait,
        reason = "the ticket requires an inherent LineIndex::from_str constructor"
    )]
    pub fn from_str(source: &str) -> Self {
        let mut newline_offsets = Vec::new();

        for (offset, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                let offset =
                    u32::try_from(offset).expect("source byte offset exceeds u32 span range");
                newline_offsets.push(offset);
            }
        }

        Self { newline_offsets }
    }

    /// Return the zero-based `(line, byte_column)` for `offset`.
    pub fn lookup(&self, offset: u32) -> (u32, u32) {
        let line = self
            .newline_offsets
            .partition_point(|&newline| newline < offset);
        let line_start = match line {
            0 => 0,
            _ => self.newline_offsets[line - 1] + 1,
        };

        (
            u32::try_from(line).expect("line count exceeds u32::MAX"),
            offset - line_start,
        )
    }
}

impl std::str::FromStr for LineIndex {
    type Err = std::convert::Infallible;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_str(source))
    }
}

/// Resolve interned symbols back to source text.
///
/// The concrete interner and frozen resolver live outside this crate so AST and
/// renderer code remain independent of parser internals.
pub trait Resolver {
    /// Return the string for `sym`, or `None` if it does not belong to this resolver.
    fn try_resolve(&self, sym: Symbol) -> Option<&str>;

    /// Return the string for `sym`.
    ///
    /// # Panics
    ///
    /// Panics if `sym` was not produced by this resolver.
    fn resolve(&self, sym: Symbol) -> &str {
        self.try_resolve(sym)
            .unwrap_or_else(|| panic!("unknown symbol {}", sym.as_u32()))
    }
}

/// Source storage accepted by the parsed root.
///
/// The trait confines source ownership choices to the root while the AST itself
/// stays non-generic and lifetime-free. The `'static` supertrait is the
/// load-bearing half of the owned-root design: it forbids a store from
/// threading a *transient* borrow (`&'a str`, `Cow<'a, str>`) into the root,
/// which is precisely the lifetime an owned-`'static` AST exists to keep out. A
/// store may own its text — `Arc<str>` (the default), `Rc<str>`, `String`,
/// `Box<str>` — or be a `'static` borrow (`&'static str`); either way the parsed
/// tree is `'static`, so it never forces a lifetime parameter onto node types.
///
/// The public parse entry points (`parse_with` → `Arc<str>`, `parse_rc_with` →
/// `Rc<str>`) always *materialize* an owned refcounted store from the borrowed
/// `&str` input; they never hand back a tree that borrows the caller's source.
/// Naming any other `'static` store is a deliberate opt-in, never the default.
pub trait SourceStore: Clone + Deref<Target = str> + 'static {}

impl<T> SourceStore for T where T: Clone + Deref<Target = str> + 'static {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::mem::size_of;
    use std::rc::Rc;
    use std::sync::Arc;

    #[test]
    fn compact_sizes_use_nonzero_niches() {
        assert_eq!(size_of::<Span>(), 8);
        assert_eq!(size_of::<Symbol>(), 4);
        assert_eq!(size_of::<Option<Symbol>>(), 4);
        assert_eq!(size_of::<FoldedSymbol>(), 4);
        assert_eq!(size_of::<Option<FoldedSymbol>>(), 4);
        assert_eq!(size_of::<NodeId>(), 4);
        // The foundational per-node metadata word these compose into: an 8-byte
        // `Span` plus a 4-byte `NodeId`. Governed by ADR-0002; previously pinned
        // only under `cargo bench`, never the nextest suite.
        assert_eq!(size_of::<Meta>(), 12);
    }

    #[test]
    fn span_methods_preserve_half_open_invariants() {
        let span = Span::new(2, 5);

        assert_eq!(span.start(), 2);
        assert_eq!(span.end(), 5);
        assert_eq!(span.len(), 3);
        assert!(!span.is_empty());
        assert!(span.contains(2));
        assert!(span.contains(4));
        assert!(!span.contains(5));
        assert!(!span.contains(1));

        let empty = Span::new(7, 7);
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
        assert!(!empty.contains(7));
    }

    #[test]
    fn span_union_and_synthetic_behave_correctly() {
        let left = Span::new(10, 20);
        let right = Span::new(3, 12);

        assert_eq!(left.union(right), Span::new(3, 20));
        assert_eq!(Span::SYNTHETIC.len(), 0);
        assert!(!Span::SYNTHETIC.is_empty());
        assert!(!Span::SYNTHETIC.contains(u32::MAX));
        assert!(Span::SYNTHETIC.is_synthetic());
        assert_eq!(Span::SYNTHETIC.union(left), left);
        assert_eq!(left.union(Span::SYNTHETIC), left);
        assert_eq!(Span::SYNTHETIC.union(Span::SYNTHETIC), Span::SYNTHETIC);
    }

    #[test]
    fn symbols_are_one_based_keys_with_zero_based_indexes() {
        assert!(Symbol::new(0).is_none());

        let symbol = Symbol::new(3).expect("non-zero symbol");
        assert_eq!(symbol.as_u32(), 3);
        assert_eq!(symbol.index(), 2);
    }

    #[test]
    fn folded_symbols_are_one_based_keys() {
        assert!(FoldedSymbol::new(0).is_none());

        let folded = FoldedSymbol::new(3).expect("non-zero folded symbol");
        assert_eq!(folded.as_u32(), 3);
    }

    #[test]
    fn node_ids_are_nonzero_counter_values() {
        assert!(NodeId::new(0).is_none());

        let node_id = NodeId::new(9).expect("non-zero node id");
        assert_eq!(node_id.as_u32(), 9);
    }

    #[test]
    fn meta_makes_derived_node_equality_structural() {
        #[derive(Debug, PartialEq, Eq, Hash)]
        struct TestNode {
            semantic: u8,
            meta: Meta,
        }

        let first = TestNode {
            semantic: 1,
            meta: meta(0, 5, 1),
        };
        let same_structure = TestNode {
            semantic: 1,
            meta: meta(100, 120, 2),
        };
        let different_structure = TestNode {
            semantic: 2,
            meta: meta(0, 5, 1),
        };

        assert_eq!(first, same_structure);
        assert_ne!(first, different_structure);

        // `Meta`'s `Ord`/`PartialOrd` are the always-`Equal` impls, so derived
        // ordering treats metadata as identity-transparent exactly as derived `Eq`
        // does: two `Meta` values with different spans and node ids still compare
        // Equal.
        assert_eq!(
            first.meta.cmp(&same_structure.meta),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            first.meta.partial_cmp(&same_structure.meta),
            Some(std::cmp::Ordering::Equal)
        );

        let mut map = HashMap::new();
        map.insert(first, "hit");
        assert_eq!(map.get(&same_structure), Some(&"hit"));
        assert_eq!(map.get(&different_structure), None);
    }

    #[test]
    fn line_index_maps_offsets_to_zero_based_byte_columns() {
        let index = LineIndex::from_str("ab\ncde\nz");

        assert_eq!(index.lookup(0), (0, 0));
        assert_eq!(index.lookup(1), (0, 1));
        assert_eq!(index.lookup(2), (0, 2));
        assert_eq!(index.lookup(3), (1, 0));
        assert_eq!(index.lookup(4), (1, 1));
        assert_eq!(index.lookup(7), (2, 0));
        assert_eq!(index.lookup(8), (2, 1));
    }

    #[test]
    fn line_index_handles_empty_input() {
        let index = LineIndex::from_str("");

        assert_eq!(index.lookup(0), (0, 0));
    }

    #[test]
    fn resolver_returns_text_for_known_symbols() {
        struct TestResolver(&'static [&'static str]);

        impl Resolver for TestResolver {
            fn try_resolve(&self, sym: Symbol) -> Option<&str> {
                self.0.get(sym.index()).copied()
            }
        }

        let resolver = TestResolver(&["users", "id"]);
        let users = Symbol::new(1).expect("symbol");
        let missing = Symbol::new(3).expect("symbol");

        assert_eq!(resolver.try_resolve(users), Some("users"));
        assert_eq!(resolver.resolve(users), "users");
        assert_eq!(resolver.try_resolve(missing), None);
    }

    #[test]
    fn source_store_admits_owned_and_static_stores() {
        fn as_str<S: SourceStore>(source: S) -> String {
            source.deref().to_owned()
        }

        // The refcounted ownership tiers (ADR-0001) and other owned stores qualify.
        let arc: Arc<str> = Arc::from("select");
        let rc: Rc<str> = Rc::from("update");
        let owned = String::from("from");
        let boxed: Box<str> = Box::from("delete");
        // A `'static` borrow is itself `'static`, so it satisfies the bound; the
        // public parse APIs still never produce one (they materialize Arc/Rc).
        let static_borrow: &'static str = "where";

        assert_eq!(as_str(arc), "select");
        assert_eq!(as_str(rc), "update");
        assert_eq!(as_str(owned), "from");
        assert_eq!(as_str(boxed), "delete");
        assert_eq!(as_str(static_borrow), "where");
    }

    #[test]
    fn source_store_is_always_static() {
        // ADR-0001's load-bearing invariant: every `SourceStore` is `'static`, so
        // a `Parsed<S>` can never thread a transient borrow into the otherwise
        // lifetime-free AST. If the `'static` supertrait is ever dropped this stops
        // compiling, because `S: SourceStore` would no longer imply `S: 'static` —
        // and a borrowed-lifetime store would silently become admissible.
        fn assert_static_store<S: SourceStore>() {
            fn needs_static<T: 'static>() {}
            needs_static::<S>();
        }

        assert_static_store::<Arc<str>>();
        assert_static_store::<Rc<str>>();
        assert_static_store::<String>();
    }

    fn meta(start: u32, end: u32, id: u32) -> Meta {
        Meta::new(
            Span::new(start, end),
            NodeId::new(id).expect("non-zero node id"),
        )
    }
}
