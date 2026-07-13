// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The owned parse root: source text, frozen resolver, and statements.

use std::borrow::Cow;
use std::fmt;
use std::sync::{Arc, OnceLock};

use crate::ast::dialect::StringLiteralSyntax;
use crate::ast::render::{Render, RenderConfig, RenderCtx, RenderExt as _};
use crate::ast::{
    Extension, LineIndex, Literal, LiteralValueError, NoExt, SourceStore, Span, Statement,
};
use crate::interner::FrozenResolver;
use crate::tokenizer::{TriviaIndex, TriviaRange};

use super::clause_marks::{ClauseMark, ClauseMarkIndex};

/// The owned, lifetime-free result of a parse.
///
/// `Parsed` is the *sole* holder of the source text, behind the [`SourceStore`]
/// trait, so the ~hundreds of AST node types stay non-generic and never carry a
/// lifetime. [`SourceStore`]'s `'static` bound is what guarantees this: the store
/// can own its text or be a `'static` borrow, but it can never thread a transient
/// borrow into the tree, so a `Parsed` is always `'static`. The default `Arc<str>`
/// additionally makes a parsed tree `Send`; an `Rc<str>` root (cheapest refcount,
/// single-thread) is available by naming the generic, and a bare `Vec<Statement>`
/// tier falls out of [`statements`](Parsed::statements) for callers that only want structure.
///
/// The [`FrozenResolver`] frozen here is the one that gives this tree's [`Symbol`]s
/// meaning; it travels with the tree precisely because a detached symbol is just
/// a `u32` otherwise.
///
/// Whole-tree `Display` renders canonical SQL from the source and resolver this
/// root owns; `std::fmt::Display` is reserved for the root because a
/// detached node lacks that context. `Debug` is derived for test/diagnostic use.
///
/// # Cross-tree safety
///
/// A node is meaningful only against **its own root's** `(source, resolver)` pair:
/// its [`Symbol`]s are indices into this root's interner, and its literal [`Span`]s
/// are byte ranges into this root's source. Grafting a node from one `Parsed` into
/// an operation against another root violates that contract, and the failure modes
/// are deliberately graded rather than guarded per node (per-node
/// provenance would grow every `Symbol`/`Meta` for a misuse the type system already
/// discourages):
///
/// - A foreign symbol **beyond** the resolver's table fails loud: canonical
///   rendering panics, and [`try_resolve`](crate::ast::Resolver::try_resolve)
///   returns `None` (the debug path prints a placeholder instead).
/// - A foreign symbol **within** the table silently resolves to the wrong text —
///   the accepted, unguarded case; the interner cannot distinguish it from a valid
///   symbol.
/// - A foreign literal span degrades **totally**, never panicking: out of range,
///   value accessors return [`LiteralValueError`] (`InvalidSourceRange`) and the
///   renderer falls back to a kind-based spelling; in range, it silently slices the
///   wrong text.
///
/// The sanctioned moves are: rewrite nodes **under the same root** (the generated
/// `VisitMut`), serialize/deserialize a whole `Parsed` (the `serde` feature carries
/// source + symbol table together, re-interning on load, and *validates* the loaded
/// document — out-of-bounds spans and out-of-table symbols are rejected at deserialize
/// rather than admitted to the graded render failures above, which remain reachable
/// only by same-process grafting that never passes through serde), and print a
/// detached node with the tolerant debug path
/// ([`RenderExt::debug_sql`](crate::ast::render::RenderExt::debug_sql)). There is
/// deliberately no cross-tree *adoption* operation: [`Parsed`] cannot be constructed
/// from parts outside this crate (`Parsed::new` is crate-internal for exactly this
/// reason), so an adopted node would have nowhere to live; if a public builder
/// surface ever ships, adoption (re-interning symbols, re-anchoring literals) ships
/// with it as one design.
///
/// [`Symbol`]: crate::ast::Symbol
#[derive(Debug)]
pub struct Parsed<S: SourceStore = Arc<str>, X: Extension = NoExt> {
    source: S,
    resolver: FrozenResolver,
    statements: Vec<Statement<X>>,
    /// The [`StringLiteralSyntax`] this tree was parsed under, retained so a string
    /// VALUE materialises dialect-correctly without the caller re-stating the dialect:
    /// a plain `'a\nb'` is `a`,`\`,`n`,`b` under ANSI/PostgreSQL but `a`,newline,`b`
    /// under MySQL (backslash escapes on), the one bit the source spelling alone cannot
    /// disambiguate. Only this small `Copy` slice of the parse's
    /// [`FeatureSet`](crate::ast::dialect::FeatureSet) is kept — the sole piece string
    /// materialisation consults — rather than the whole set.
    string_literals: StringLiteralSyntax,
    /// Lazily-built byte-offset → line/column index, cached on first request so
    /// the common no-diagnostic path pays nothing. `OnceLock` keeps a
    /// `Parsed<Arc<str>>` `Send + Sync`.
    line_index: OnceLock<LineIndex>,
    /// Out-of-band comment/whitespace spans, recoverable by offset for tooling.
    /// Empty unless the tree was produced by
    /// [`ParseConfig::capture_trivia`](crate::ParseConfig::capture_trivia): capture is opt-in so the
    /// default parse pays nothing, and an empty index owns an empty `Vec` (no
    /// allocation). The AST nodes never carry trivia, so structural equality and
    /// node sizes are unaffected — it lives only here, on the root.
    trivia: TriviaIndex,
    /// Per-clause-keyword offsets, recoverable by owning node and offset for the
    /// formatter's comment anchoring. Empty unless the tree was produced by
    /// [`ParseConfig::capture_trivia`](crate::ParseConfig::capture_trivia): clause-mark
    /// capture rides the *same* opt-in as trivia, so the default parse pays nothing
    /// and an empty index owns an empty `Vec` (no allocation). Like trivia it lives
    /// only here — the hot AST nodes never carry it, so node sizes and structural
    /// equality are unaffected.
    clause_marks: ClauseMarkIndex,
}

/// Stock parser root for dialects that use the built-in AST only.
pub type StockParsed<S = Arc<str>> = Parsed<S, NoExt>;

impl<S: SourceStore, X: Extension> Parsed<S, X> {
    /// Bundle the three owned pieces produced by a finished parse.
    ///
    /// Crate-internal: a `Parsed` is only ever produced by [`parse_with`], which
    /// guarantees the resolver is the one the statements' symbols were interned
    /// into. Handing those pieces out for arbitrary recombination would let a
    /// caller pair statements with a foreign resolver.
    ///
    /// [`parse_with`]: super::parse_with
    pub(crate) fn new(
        source: S,
        resolver: FrozenResolver,
        statements: Vec<Statement<X>>,
        string_literals: StringLiteralSyntax,
    ) -> Self {
        Self {
            source,
            resolver,
            statements,
            string_literals,
            line_index: OnceLock::new(),
            trivia: TriviaIndex::default(),
            clause_marks: ClauseMarkIndex::default(),
        }
    }

    /// Attach the out-of-band trivia captured by a trivia-enabled parse.
    ///
    /// Crate-internal builder step: the default [`new`](Self::new) leaves trivia
    /// empty (the common path), and only the trivia-capturing collector swaps in a
    /// populated index.
    pub(crate) fn with_trivia(mut self, trivia: TriviaIndex) -> Self {
        self.trivia = trivia;
        self
    }

    /// Attach the clause-keyword offsets captured by a clause-mark-capturing parse.
    ///
    /// Crate-internal builder step mirroring [`with_trivia`](Self::with_trivia): the
    /// default [`new`](Self::new) leaves the index empty (the common path), and only
    /// the opt-in capture path swaps in a populated index.
    pub(crate) fn with_clause_marks(mut self, clause_marks: ClauseMarkIndex) -> Self {
        self.clause_marks = clause_marks;
        self
    }

    /// The parsed statements, in source order.
    pub fn statements(&self) -> &[Statement<X>] {
        &self.statements
    }

    /// Consume the root into its bare `Vec<Statement>`, dropping source and resolver.
    ///
    /// The structure-only tier: a caller that only inspects statement
    /// *shape* — never resolving a [`Symbol`] back to text nor slicing a literal
    /// from source — can discard the source and resolver entirely. The returned
    /// `Vec<Statement>` is `Send + 'static`. Any [`Symbol`] left in the tree is
    /// then a bare `u32` with no resolver to give it meaning, which is the whole
    /// point of this tier.
    ///
    /// [`Symbol`]: crate::ast::Symbol
    pub fn into_statements(self) -> Vec<Statement<X>> {
        self.statements
    }

    /// The original source text this tree was parsed from.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The frozen resolver that maps this tree's symbols back to text.
    pub fn resolver(&self) -> &FrozenResolver {
        &self.resolver
    }

    /// The [`StringLiteralSyntax`] this tree was parsed under.
    ///
    /// String VALUE materialisation is the one literal accessor that needs external
    /// dialect context — a plain `'a\nb'` is the four characters `a`,`\`,`n`,`b` under
    /// ANSI/PostgreSQL but `a`,newline,`b` under MySQL, where backslash escapes are
    /// on. A consumer holding a `Parsed` reads the parse's own syntax here
    /// instead of hand-passing a [`StringLiteralSyntax`] to
    /// [`Literal::as_str_in`](crate::ast::Literal::as_str_in); prefer the
    /// [`literal_str`](Self::literal_str) convenience, which threads this together with the source.
    pub fn string_literal_syntax(&self) -> StringLiteralSyntax {
        self.string_literals
    }

    /// Materialise a string `literal`'s value under *this* parse's own dialect and
    /// source.
    ///
    /// The dialect-correct counterpart to
    /// [`Literal::as_str`](crate::ast::Literal::as_str) for a literal that came from
    /// this tree: it threads both the root's own [`source`](Self::source) and its
    /// [`string_literal_syntax`](Self::string_literal_syntax) into
    /// [`Literal::as_str_in`](crate::ast::Literal::as_str_in), so a MySQL `'a\nb'`
    /// decodes its backslash escapes while an ANSI/PostgreSQL one does not — without the
    /// caller ever naming a [`StringLiteralSyntax`].
    ///
    /// Explicit context, no hidden global (mirroring the `debug_sql` precedent): the
    /// source and the syntax both come from this root, so they cannot disagree. Pass a
    /// `literal` from this tree — one from a foreign parse whose span does not slice
    /// this source reports
    /// [`InvalidSourceRange`](crate::ast::LiteralValueErrorKind::InvalidSourceRange),
    /// exactly as [`Literal::as_str_in`](crate::ast::Literal::as_str_in) would against a
    /// mismatched source.
    ///
    /// # Errors
    ///
    /// As [`Literal::as_str_in`](crate::ast::Literal::as_str_in): a
    /// [`LiteralValueError`] when `literal` is not a string, has no backing source, or
    /// is not a well-formed string constant.
    pub fn literal_str(&self, literal: &Literal) -> Result<Cow<'_, str>, LiteralValueError> {
        literal.as_str_in(self.source(), self.string_literals)
    }

    /// The lazily-built, cached [`LineIndex`] for this source.
    ///
    /// Built once on first call via a single newline scan and reused on every
    /// later call; a parse that never needs diagnostics never builds it.
    pub fn line_index(&self) -> &LineIndex {
        self.line_index
            .get_or_init(|| LineIndex::from_str(&self.source))
    }

    /// Recover the zero-based `(line, byte_column)` for a byte `offset`.
    ///
    /// Line and column are byte positions; editor/UTF-16 columns are a layer on
    /// top of this byte-accurate index.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        self.line_index().lookup(offset)
    }

    /// Recover the start and end `(line, byte_column)` of a source-backed `span`.
    ///
    /// Returns `None` for a synthetic span — a rewrite-synthesized node has no
    /// source location to resolve.
    pub fn span_line_col(&self, span: Span) -> Option<((u32, u32), (u32, u32))> {
        if span.is_synthetic() {
            return None;
        }
        let index = self.line_index();
        Some((index.lookup(span.start()), index.lookup(span.end())))
    }

    /// Every captured comment and whitespace run, in source order.
    ///
    /// Empty unless this tree was produced by
    /// [`ParseConfig::capture_trivia`](crate::ParseConfig::capture_trivia); trivia capture is opt-in so
    /// the default parse path stays trivia-free at zero cost. The text of
    /// any run slices back out of [`source`](Self::source) by its span, the same
    /// zero-copy contract the tokens use.
    pub fn trivia(&self) -> &[TriviaRange] {
        self.trivia.all()
    }

    /// The comment/whitespace runs fully contained in `span` (a binary search over
    /// the sorted trivia). Empty for a synthetic span or when none was captured.
    pub fn trivia_in(&self, span: Span) -> &[TriviaRange] {
        self.trivia.in_span(span)
    }

    /// The contiguous trivia immediately preceding `offset` — the leading
    /// comments/whitespace of a token starting there.
    ///
    /// Pass a token's [`Span::start`] to recover the comments attached to its front
    /// (e.g. a doc comment above a column). Empty when a real token directly abuts
    /// `offset` or when no trivia was captured.
    pub fn trivia_before(&self, offset: u32) -> &[TriviaRange] {
        self.trivia.before(offset)
    }

    /// Every recorded clause-introducing keyword, in source (offset) order.
    ///
    /// Empty unless this tree was produced by
    /// [`ParseConfig::capture_trivia`](crate::ParseConfig::capture_trivia); clause-mark
    /// capture is the same opt-in as trivia, so the default parse stays clause-mark
    /// free at zero cost. Each [`ClauseMark`] pairs the clause
    /// [`kind`](ClauseMark::kind) and keyword [`offset`](ClauseMark::offset) with the
    /// [`owner`](ClauseMark::owner) [`NodeId`](crate::ast::NodeId) of the node whose
    /// field the clause fills — the anchor a formatter needs to place a comment
    /// sitting before an otherwise node-less clause keyword.
    pub fn clause_marks(&self) -> &[ClauseMark] {
        self.clause_marks.all()
    }

    /// The clause marks whose keyword offset lies within `span` (a binary search over
    /// the sorted index). Empty for a synthetic span or when none was captured.
    ///
    /// Passing a node's own span recovers exactly the clause keywords that node
    /// heads.
    pub fn clause_marks_in(&self, span: Span) -> &[ClauseMark] {
        self.clause_marks.in_span(span)
    }
}

/// Whole-tree canonical `Display`: `format!("{parsed}")` renders the parsed
/// statements back to SQL.
///
/// `std::fmt::Display` is reserved for the `Parsed` root precisely because
/// the root owns the source and resolver a render needs; a *detached* node does
/// not, so it stays ctx-based through [`Displayed`](crate::ast::render::Displayed).
/// The body mirrors the conformance round-trip oracle: a canonical [`RenderCtx`]
/// over this root's own resolver and source, statements joined by `; ` — the same
/// `render_statements_into` loop
/// [`to_sql`](Self::to_sql) and [`render_into`](Self::render_into) run, factored out
/// so the three entry points cannot drift apart.
///
/// `Display`/`.to_string()` behave exactly as before and remain available, but
/// `ToString`'s blanket impl always starts from an empty buffer — rendering pays the
/// 8→16→…doubling chain up to the output size (the render-perf audit measured 7
/// reallocations for a 270-byte statement) — because `Display::fmt` is handed a
/// `Formatter` wrapping a buffer it does not own, so it has nothing to reserve
/// against. On a render-dominant path (transpile-many, rewrite→render — the
/// rewritable-AST value proposition), prefer [`to_sql`](Self::to_sql), or
/// [`render_into`](Self::render_into) with a reused buffer.
impl<S: SourceStore, X: Extension + Render> fmt::Display for Parsed<S, X> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let config = RenderConfig::default();
        let ctx = RenderCtx::new(self.resolver(), self.source(), &config);
        self.render_statements_into(&ctx, f)
    }
}

impl<S: SourceStore, X: Extension + Render> Parsed<S, X> {
    /// The loop `Display`, [`to_sql`](Self::to_sql), and [`render_into`](Self::render_into)
    /// all write through: each statement rendered via `ctx`, joined by `; `. Sharing
    /// one loop body means the three entry points can only ever differ in how (or
    /// whether) they size/own the output buffer, never in what gets written.
    fn render_statements_into<'r, W: fmt::Write>(
        &self,
        ctx: &'r RenderCtx<'r>,
        out: &mut W,
    ) -> fmt::Result {
        for (i, statement) in self.statements().iter().enumerate() {
            if i > 0 {
                out.write_str("; ")?;
            }
            write!(out, "{}", statement.displayed(ctx))?;
        }
        Ok(())
    }

    /// Render this tree's canonical SQL into a caller-owned buffer, appending after
    /// whatever `out` already holds.
    ///
    /// For a transpile-many or rewrite-then-render pipeline — the shape the
    /// rewritable AST exists for — reusing one `String` across many
    /// `render_into` calls (clearing it, or just reserving generously, between
    /// renders) skips a `String` allocation per tree entirely. [`to_sql`](Self::to_sql)
    /// is the single-tree, pre-sized convenience over this; `Display`/`.to_string()`
    /// remain available for the common case that needs neither.
    pub fn render_into(&self, out: &mut String) -> fmt::Result {
        let config = RenderConfig::default();
        let ctx = RenderCtx::new(self.resolver(), self.source(), &config);
        self.render_statements_into(&ctx, out)
    }

    /// Render this tree's canonical SQL to a freshly-allocated, pre-sized `String`.
    ///
    /// The Tier-1 primary path's pre-sized entry (the render-perf audit):
    /// canonical output tracks source length closely (a 276-byte source measured 270
    /// rendered bytes), so reserving `source().len()` up front renders in one
    /// allocation. Prefer this over `Display`/`.to_string()`, which start from
    /// `String::new()` and pay the empty-buffer doubling chain to grow into the same
    /// output — `Display::fmt` itself cannot reserve, since it is handed a
    /// `Formatter` over a buffer it does not own, which is exactly why this inherent
    /// method exists alongside it.
    pub fn to_sql(&self) -> String {
        let mut out = String::with_capacity(self.source().len());
        self.render_into(&mut out)
            .expect("rendering to a String cannot fail");
        out
    }
}

/// Portable, self-contained (de)serialization of the parse root (`serde` feature).
///
/// A `Parsed` is the top-level unit the ticket makes serializable, because it is the
/// only thing that owns a `Symbol`'s meaning: the AST nodes carry bare numeric
/// interner keys, which are portable only when paired with the resolver that minted
/// them. The serialized document is therefore `{ source, symbols, string_literals,
/// statements }` — the statement tree keeps its numeric symbols, and the resolver's
/// dynamic string table (`symbols`, in `Symbol` index order) travels alongside so it
/// can be re-interned into an identical resolver on load. Keyword symbols are omitted
/// (fixed low slots backed by the static, build-stable `Keyword::ALL`). This is a
/// deliberate departure from inlining each symbol's text at every node: inlining
/// would need the resolver threaded into every node's `Serialize`, which serde only
/// supports via a thread-local raw pointer (`unsafe` — forbidden workspace-wide) or
/// a parallel string-typed AST; the co-serialized table is smaller (each identifier
/// stored once, not once per occurrence) and re-interns to identical symbols, giving
/// the same cross-process portability the ticket requires.
///
/// `Deserialize` reconstructs the resolver and requires `S: From<String>` (satisfied
/// by `Arc<str>`, `Rc<str>`, `String`, `Box<str>` — every materializing store; a
/// `&'static str` store cannot be deserialized into, which is correct). The
/// recursive statement tree is deserialized through
/// [`DepthLimited`](crate::ast::serde_depth::DepthLimited) so untrusted input cannot
/// build a hostile-deep tree that overflows the stack on its first drop/render/visit
/// — the deserialize counterpart of the parser's recursion guard.
///
/// Because `Deserialize` rebuilds each `Span` and `Symbol` field-by-field (so the
/// `SYNTHETIC` sentinel round-trips), a hand-crafted document can otherwise pair the
/// tree with content the parser would never emit. Deserialization therefore
/// *validates* the whole document before returning it: every non-synthetic span is
/// checked in bounds for `source` (`start <= end <= source.len()`), every `Symbol` is
/// checked to resolve in the rebuilt table, and a table with duplicate entries — which
/// would silently misresolve symbols, since `intern_nonkeyword` dedupes — is rejected.
/// A violation is a serde error naming the first offending span/symbol rather than a
/// tree that later panics on canonical render or slices the wrong source text.
///
/// The depth guard uses [`DEFAULT_DESERIALIZE_DEPTH`](crate::ast::serde_depth::DEFAULT_DESERIALIZE_DEPTH)
/// by default. A legitimately deep tree (e.g. a long left-associative operator chain,
/// parsed iteratively so it clears the parser's own guard) serializes fine yet exceeds
/// that default on load; it round-trips only through
/// [`deserialize_with_depth`](Parsed::deserialize_with_depth) with a raised budget.
///
/// The lazy line index and out-of-band trivia are not serialized: the index is a
/// pure cache re-derived from `source`, and trivia is opt-in diagnostic data (empty
/// on the default parse); a round-tripped tree renders byte-identically regardless.
#[cfg(any(feature = "serde-serialize", feature = "serde-deserialize"))]
mod serde_impls {
    use super::*;
    // Glob the AST vocabulary: the span-validation walk below overrides a `visit_*`
    // method per span-bearing node type, so it names ~100 node types — the same set
    // the generated `NodeIdWalk`/`Spanned` impls cover. Listing them individually
    // would be noise; the walker is exactly the kind of whole-tree pass that globs the
    // AST (cf. `conformance`'s span/symbol walkers).
    #[cfg(feature = "serde-deserialize")]
    use crate::ast::generated::visit::{self, Visit};
    #[cfg(feature = "serde-deserialize")]
    use crate::ast::serde_depth::{DEFAULT_DESERIALIZE_DEPTH, DepthLimited};
    #[cfg(feature = "serde-deserialize")]
    use crate::ast::*;
    #[cfg(feature = "serde-deserialize")]
    use crate::interner::Interner;
    #[cfg(feature = "serde-deserialize")]
    use serde::Deserialize;
    #[cfg(feature = "serde-serialize")]
    use serde::Serialize;
    #[cfg(feature = "serde-deserialize")]
    use serde::de::{Deserializer, Error as _};

    /// serde remote proxy for the foreign `StringLiteralSyntax` (defined in the AST
    /// crate's `dialect` module, which this crate cannot add a derive to). Mirrors its
    /// public `bool` fields; a new field there surfaces here as a compile error.
    #[cfg_attr(feature = "serde-serialize", derive(Serialize))]
    #[cfg_attr(feature = "serde-deserialize", derive(Deserialize))]
    #[serde(remote = "StringLiteralSyntax")]
    struct StringLiteralSyntaxDef {
        escape_strings: bool,
        dollar_quoted_strings: bool,
        national_strings: bool,
        double_quoted_strings: bool,
        backslash_escapes: bool,
        unicode_strings: bool,
        bit_string_literals: bool,
        blob_literals: bool,
        charset_introducers: bool,
        same_line_adjacent_concat: bool,
    }

    #[cfg(feature = "serde-serialize")]
    impl<S: SourceStore, X: Extension + Serialize> Serialize for Parsed<S, X> {
        fn serialize<Sr>(&self, serializer: Sr) -> Result<Sr::Ok, Sr::Error>
        where
            Sr: serde::Serializer,
        {
            // Borrowing view so serialization copies nothing: `&str` source, the
            // resolver's string slice, and the statement slice are written in place.
            #[derive(Serialize)]
            #[serde(bound(serialize = "X: Serialize"))]
            struct ParsedRef<'a, X: Extension> {
                source: &'a str,
                symbols: &'a [Box<str>],
                #[serde(with = "StringLiteralSyntaxDef")]
                string_literals: StringLiteralSyntax,
                statements: &'a [Statement<X>],
            }

            ParsedRef {
                source: self.source(),
                symbols: self.resolver.dynamic_strings(),
                string_literals: self.string_literals,
                statements: &self.statements,
            }
            .serialize(serializer)
        }
    }

    #[cfg(feature = "serde-deserialize")]
    impl<'de, S, X> Deserialize<'de> for Parsed<S, X>
    where
        S: SourceStore + From<String>,
        X: Extension + Deserialize<'de>,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            Self::deserialize_with_depth(deserializer, DEFAULT_DESERIALIZE_DEPTH)
        }
    }

    #[cfg(feature = "serde-deserialize")]
    impl<S: SourceStore + From<String>, X: Extension> Parsed<S, X> {
        /// Deserialize a parse root, bounding deserialization nesting to `max_depth`
        /// instead of [`DEFAULT_DESERIALIZE_DEPTH`].
        ///
        /// Serialization is asymmetric with the *default* deserialize: a legitimately
        /// deep tree — e.g. a long left-associative operator chain, which the parser
        /// builds iteratively and so never trips its own recursion guard — serializes
        /// fine yet is *rejected* on load by [`Deserialize`], because the default cap
        /// deliberately sits below the point where the deserialization recursion (as
        /// deep as the tree) would itself risk the stack. Such a tree round-trips only
        /// through this method with a raised `max_depth`, which the caller opts into at
        /// their own stack-safety risk.
        ///
        /// Like the default path, this validates the deserialized document's content
        /// (span bounds, symbol resolvability, no duplicate table entries) before
        /// returning; only the depth budget differs.
        ///
        /// # Errors
        ///
        /// The format's deserialization error, including a depth-limit-exceeded error
        /// when the input nests past `max_depth`, and a content-validation error naming
        /// the first out-of-bounds span, unresolvable symbol, or duplicate table entry.
        pub fn deserialize_with_depth<'de, D>(
            deserializer: D,
            max_depth: usize,
        ) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
            X: Deserialize<'de>,
        {
            #[derive(Deserialize)]
            #[serde(bound(deserialize = "X: Deserialize<'de>"))]
            struct ParsedData<X: Extension> {
                source: String,
                symbols: Vec<String>,
                #[serde(with = "StringLiteralSyntaxDef")]
                string_literals: StringLiteralSyntax,
                statements: Vec<Statement<X>>,
            }

            // Wrap the whole deserializer in the format-agnostic depth guard so the
            // recursive statement tree cannot reconstruct a hostile-deep tree that
            // overflows the stack on its first drop/render/visit — the deserialize
            // counterpart of the parser's recursion guard (ADR-0012). The few
            // fixed-depth levels the outer `{ source, symbols, ... }` frame spends are
            // negligible against the budget.
            let data = ParsedData::<X>::deserialize(DepthLimited::new(deserializer, max_depth))?;

            // Re-intern the dynamic strings in index order so the tree's numeric
            // symbols resolve to the same text. `intern_nonkeyword` reproduces the
            // exact `strings` order for a table that honours the interner invariants
            // (no duplicates, no canonical keyword spellings).
            let mut interner = Interner::new();
            for text in &data.symbols {
                interner.intern_nonkeyword(text);
            }
            let resolver = interner.freeze();

            // Reject a table carrying duplicate entries. `intern_nonkeyword` dedupes,
            // so a duplicate collapses two slots and shifts every later symbol's
            // resolution — silent misresolution, the worst outcome for a hostile
            // document. A genuine parse never emits duplicates, so a shorter rebuilt
            // table than the document declared is proof of corruption.
            if resolver.dynamic_strings().len() != data.symbols.len() {
                return Err(D::Error::custom(format!(
                    "symbol table carries duplicate entries: {} declared, {} distinct \
                     after interning (duplicates would silently misresolve symbols)",
                    data.symbols.len(),
                    resolver.dynamic_strings().len(),
                )));
            }

            // Reject hostile tree content the field-by-field rebuild would otherwise
            // admit: out-of-bounds spans (silent wrong render text) and symbols absent
            // from the rebuilt table (a panic on the first canonical render).
            validate(&data.source, &resolver, &data.statements).map_err(D::Error::custom)?;

            Ok(Parsed::new(
                S::from(data.source),
                resolver,
                data.statements,
                data.string_literals,
            ))
        }
    }

    /// Walk the deserialized statement tree once, asserting every non-synthetic span
    /// is in bounds for `source` (`start <= end <= source.len()`) and every `Symbol`
    /// resolves in `resolver`. Returns the first violation as a serde-mappable message.
    ///
    /// In-range-but-wrong values (a valid-looking span slicing the wrong text, a
    /// valid-looking symbol resolving to the wrong identifier) are inherently
    /// indistinguishable from intent and stay covered by the cross-tree-safety
    /// contract's documented grading, not by this pass.
    #[cfg(feature = "serde-deserialize")]
    fn validate<X: Extension>(
        source: &str,
        resolver: &FrozenResolver,
        statements: &[Statement<X>],
    ) -> Result<(), String> {
        let source_len = u32::try_from(source.len())
            .map_err(|_| "source length exceeds the u32 span range".to_string())?;
        let mut validator = DocumentValidator {
            source_len,
            resolver,
            visited: 0,
            error: None,
        };
        for statement in statements {
            if validator.error.is_some() {
                break;
            }
            validator.visit_statement(statement);
        }
        match validator.error {
            Some(message) => Err(message),
            None => Ok(()),
        }
    }

    /// The whole-tree validation walk. Checks the span of every node carrying a
    /// `Meta` — the nodes whose span is a serialized field, hence independently
    /// corruptible (via the `check_spans!` list below, mirroring the generated
    /// `NodeIdWalk`'s node set) — and resolves every raw `Symbol` leaf (the five sites
    /// `conformance`'s `SymbolResolvability` overrides). A node whose span is *derived*
    /// from its children (e.g. `ObjectName`, folded from its `Ident` parts) carries no
    /// serialized span of its own and is covered transitively: its span is the union
    /// of children already checked here. Records the first violation and stops
    /// descending, so the walk is `O(nodes until the first fault)`.
    #[cfg(feature = "serde-deserialize")]
    struct DocumentValidator<'a> {
        source_len: u32,
        resolver: &'a FrozenResolver,
        /// Count of `Meta`-bearing nodes visited, cross-checked against the generated
        /// `NodeIdWalk`'s `Meta` count by `tests::span_walk_covers_every_node` so the
        /// hand-mirrored `check_spans!` list cannot silently drift from the AST's real
        /// `Meta`-bearing node set.
        #[cfg_attr(not(test), allow(dead_code))]
        visited: usize,
        error: Option<String>,
    }

    #[cfg(feature = "serde-deserialize")]
    impl DocumentValidator<'_> {
        fn check_span(&mut self, node: &'static str, span: Span) {
            self.visited += 1;
            if span.is_synthetic() {
                return;
            }
            let start = span.start();
            let end = span.end();
            if start > end || end > self.source_len {
                self.error.get_or_insert_with(|| {
                    format!(
                        "node {node} carries span {start}..{end}, out of bounds for the \
                         {}-byte source (requires start <= end <= source length)",
                        self.source_len,
                    )
                });
            }
        }

        fn check_symbol(&mut self, sym: Symbol) {
            if self.resolver.try_resolve(sym).is_none() {
                self.error.get_or_insert_with(|| {
                    format!(
                        "symbol {} is absent from the deserialized symbol table (would \
                         panic on canonical render)",
                        sym.as_u32(),
                    )
                });
            }
        }
    }

    /// One span-checking `visit_*` override per `Meta`-bearing node type: bail if a
    /// fault is already recorded, else check the node's span and descend. The list
    /// mirrors the generated `NodeIdWalk`'s override set exactly (the same `Meta`
    /// walk), minus the four `Meta`-bearing sites that also carry a `Symbol` and so are
    /// hand-written below; a brand-new node *type* needs its line added here, guarded
    /// by `tests::span_walk_covers_every_node`.
    #[cfg(feature = "serde-deserialize")]
    macro_rules! check_spans {
        ($lt:lifetime, $(($method:ident, $walk:ident, $ty:ty)),+ $(,)?) => {
            $(
                fn $method(&mut self, node: &$lt $ty) {
                    if self.error.is_some() {
                        return;
                    }
                    self.check_span(stringify!($method), node.span());
                    // Pin `X` explicitly: a non-generic node type (`Ident`,
                    // `SessionStatement`, …) does not carry `X`, so the generated
                    // `walk_*`'s extension parameter is otherwise unconstrained (this
                    // impl is `Visit` for every `X`).
                    visit::$walk::<Self, X>(self, node);
                }
            )+
        };
    }

    #[cfg(feature = "serde-deserialize")]
    impl<'ast, X: Extension> Visit<'ast, X> for DocumentValidator<'_> {
        check_spans!('ast,
            (visit_session_statement, walk_session_statement, SessionStatement<X>),
            (visit_set_value, walk_set_value, SetValue),
            (visit_set_parameter_value, walk_set_parameter_value, SetParameterValue),
            (visit_special_set_value, walk_special_set_value, SpecialSetValue),
            (visit_constraints_target, walk_constraints_target, ConstraintsTarget),
            (visit_set_names_value, walk_set_names_value, SetNamesValue),
            (visit_config_parameter, walk_config_parameter, ConfigParameter),
            (visit_access_control_statement, walk_access_control_statement, AccessControlStatement<X>),
            (visit_privileges, walk_privileges, Privileges),
            (visit_privilege, walk_privilege, Privilege),
            (visit_grant_object, walk_grant_object, GrantObject<X>),
            (visit_routine_signature, walk_routine_signature, RoutineSignature<X>),
            (visit_grantee, walk_grantee, Grantee),
            (visit_role_spec, walk_role_spec, RoleSpec),
            (visit_create_table, walk_create_table, CreateTable<X>),
            (visit_create_table_body, walk_create_table_body, CreateTableBody<X>),
            (visit_table_element, walk_table_element, TableElement<X>),
            (visit_column_def, walk_column_def, ColumnDef<X>),
            (visit_column_constraint, walk_column_constraint, ColumnConstraint<X>),
            (visit_constraint_characteristics, walk_constraint_characteristics, ConstraintCharacteristics),
            (visit_column_option, walk_column_option, ColumnOption<X>),
            (visit_foreign_key_ref, walk_foreign_key_ref, ForeignKeyRef),
            (visit_referential_action, walk_referential_action, ReferentialAction),
            (visit_generated_column, walk_generated_column, GeneratedColumn<X>),
            (visit_identity_column, walk_identity_column, IdentityColumn<X>),
            (visit_identity_option, walk_identity_option, IdentityOption<X>),
            (visit_table_constraint_def, walk_table_constraint_def, TableConstraintDef<X>),
            (visit_table_constraint, walk_table_constraint, TableConstraint<X>),
            (visit_create_table_option, walk_create_table_option, CreateTableOption<X>),
            (visit_create_table_option_kind, walk_create_table_option_kind, CreateTableOptionKind<X>),
            (visit_table_option, walk_table_option, TableOption),
            (visit_table_option_value, walk_table_option_value, TableOptionValue),
            (visit_table_storage_parameter, walk_table_storage_parameter, TableStorageParameter<X>),
            (visit_alter_table, walk_alter_table, AlterTable<X>),
            (visit_alter_table_action, walk_alter_table_action, AlterTableAction<X>),
            (visit_alter_column_action, walk_alter_column_action, AlterColumnAction<X>),
            (visit_drop_statement, walk_drop_statement, DropStatement),
            (visit_comment_on_statement, walk_comment_on_statement, CommentOnStatement<X>),
            (visit_create_schema, walk_create_schema, CreateSchema<X>),
            (visit_create_view, walk_create_view, CreateView<X>),
            (visit_create_index, walk_create_index, CreateIndex<X>),
            (visit_index_column, walk_index_column, IndexColumn<X>),
            (visit_create_trigger, walk_create_trigger, CreateTrigger<X>),
            (visit_trigger_event, walk_trigger_event, TriggerEvent),
            (visit_create_database, walk_create_database, CreateDatabase),
            (visit_create_function, walk_create_function, CreateFunction<X>),
            (visit_function_param, walk_function_param, FunctionParam<X>),
            (visit_function_option, walk_function_option, FunctionOption<X>),
            (visit_insert, walk_insert, Insert<X>),
            (visit_insert_target, walk_insert_target, InsertTarget),
            (visit_insert_source, walk_insert_source, InsertSource<X>),
            (visit_insert_values, walk_insert_values, InsertValues<X>),
            (visit_insert_value, walk_insert_value, InsertValue<X>),
            (visit_dml_target, walk_dml_target, DmlTarget),
            (visit_update, walk_update, Update<X>),
            (visit_update_assignment, walk_update_assignment, UpdateAssignment<X>),
            (visit_update_value, walk_update_value, UpdateValue<X>),
            (visit_update_tuple_source, walk_update_tuple_source, UpdateTupleSource<X>),
            (visit_delete, walk_delete, Delete<X>),
            (visit_dml_selection, walk_dml_selection, DmlSelection<X>),
            (visit_default_value, walk_default_value, DefaultValue),
            (visit_returning, walk_returning, Returning<X>),
            (visit_upsert, walk_upsert, Upsert<X>),
            (visit_on_conflict, walk_on_conflict, OnConflict<X>),
            (visit_conflict_target, walk_conflict_target, ConflictTarget<X>),
            (visit_conflict_action, walk_conflict_action, ConflictAction<X>),
            (visit_merge, walk_merge, Merge<X>),
            (visit_merge_when_clause, walk_merge_when_clause, MergeWhenClause<X>),
            (visit_merge_action, walk_merge_action, MergeAction<X>),
            (visit_subscript_expr, walk_subscript_expr, SubscriptExpr<X>),
            (visit_collate_expr, walk_collate_expr, CollateExpr<X>),
            (visit_at_time_zone_expr, walk_at_time_zone_expr, AtTimeZoneExpr<X>),
            (visit_array_expr, walk_array_expr, ArrayExpr<X>),
            (visit_row_expr, walk_row_expr, RowExpr<X>),
            (visit_field_selection_expr, walk_field_selection_expr, FieldSelectionExpr<X>),
            (visit_function_call, walk_function_call, FunctionCall<X>),
            (visit_case_expr, walk_case_expr, CaseExpr<X>),
            (visit_when_clause, walk_when_clause, WhenClause<X>),
            (visit_extract_expr, walk_extract_expr, ExtractExpr<X>),
            (visit_literal, walk_literal, Literal),
            (visit_query, walk_query, Query<X>),
            (visit_set_expr, walk_set_expr, SetExpr<X>),
            (visit_with, walk_with, With<X>),
            (visit_cte, walk_cte, Cte<X>),
            (visit_values, walk_values, Values<X>),
            (visit_values_item, walk_values_item, ValuesItem<X>),
            (visit_select, walk_select, Select<X>),
            (visit_into_target, walk_into_target, IntoTarget),
            (visit_group_by_item, walk_group_by_item, GroupByItem<X>),
            (visit_select_item, walk_select_item, SelectItem<X>),
            (visit_select_distinct, walk_select_distinct, SelectDistinct<X>),
            (visit_table_with_joins, walk_table_with_joins, TableWithJoins<X>),
            (visit_table_alias, walk_table_alias, TableAlias),
            (visit_table_sample, walk_table_sample, TableSample<X>),
            (visit_table_function_column, walk_table_function_column, TableFunctionColumn<X>),
            (visit_rows_from_item, walk_rows_from_item, RowsFromItem<X>),
            (visit_table_factor, walk_table_factor, TableFactor<X>),
            (visit_join, walk_join, Join<X>),
            (visit_join_operator, walk_join_operator, JoinOperator<X>),
            (visit_join_constraint, walk_join_constraint, JoinConstraint<X>),
            (visit_order_by_expr, walk_order_by_expr, OrderByExpr<X>),
            (visit_order_by_using, walk_order_by_using, OrderByUsing),
            (visit_limit, walk_limit, Limit<X>),
            (visit_statement, walk_statement, Statement<X>),
            (visit_transaction_statement, walk_transaction_statement, TransactionStatement),
            (visit_transaction_mode, walk_transaction_mode, TransactionMode),
            (visit_data_type, walk_data_type, DataType<X>),
            (visit_copy_statement, walk_copy_statement, CopyStatement<X>),
            (visit_copy_source, walk_copy_source, CopySource<X>),
            (visit_copy_target, walk_copy_target, CopyTarget),
            (visit_copy_option, walk_copy_option, CopyOption),
            (visit_copy_option_value, walk_copy_option_value, CopyOptionValue),
            (visit_explain_statement, walk_explain_statement, ExplainStatement<X>),
            (visit_explain_option, walk_explain_option, ExplainOption),
            (visit_pragma_statement, walk_pragma_statement, PragmaStatement),
            (visit_attach_statement, walk_attach_statement, AttachStatement<X>),
            (visit_detach_statement, walk_detach_statement, DetachStatement),
            (visit_vacuum_statement, walk_vacuum_statement, VacuumStatement<X>),
            (visit_reindex_statement, walk_reindex_statement, ReindexStatement),
            (visit_analyze_statement, walk_analyze_statement, AnalyzeStatement),
            (visit_use_statement, walk_use_statement, UseStatement),
            (visit_pivot, walk_pivot, Pivot<X>),
            (visit_unpivot, walk_unpivot, Unpivot<X>),
            (visit_pivot_expr, walk_pivot_expr, PivotExpr<X>),
            (visit_pivot_column, walk_pivot_column, PivotColumn<X>),
            (visit_unpivot_column, walk_unpivot_column, UnpivotColumn<X>),
            (visit_window_spec, walk_window_spec, WindowSpec<X>),
            (visit_window_definition, walk_window_definition, WindowDefinition<X>),
            (visit_window_frame, walk_window_frame, WindowFrame<X>),
            (visit_window_frame_bound, walk_window_frame_bound, WindowFrameBound<X>),
            (visit_named_window, walk_named_window, NamedWindow<X>),
        );

        // `Ident`, `Expr`, `FunctionArg`, and `NamedOperatorExpr` are meta-bearing
        // (so they check their span, like the macro list) *and* carry a raw `Symbol`
        // (so they also resolve it). `ParameterKind::Named` carries a symbol but no
        // `Meta`, so it checks only the symbol — it is deliberately absent from the
        // macro list and from `NodeIdWalk`. Together these five are the raw-`Symbol`
        // leaf sites `conformance`'s `SymbolResolvability` mirrors.
        fn visit_ident(&mut self, node: &'ast Ident) {
            if self.error.is_some() {
                return;
            }
            self.check_span("visit_ident", node.span());
            self.check_symbol(node.sym);
            visit::walk_ident::<Self, X>(self, node);
        }

        fn visit_expr(&mut self, node: &'ast Expr<X>) {
            if self.error.is_some() {
                return;
            }
            self.check_span("visit_expr", node.span());
            if let Expr::SessionVariable { name, .. } = node {
                self.check_symbol(*name);
            }
            visit::walk_expr(self, node);
        }

        fn visit_function_arg(&mut self, node: &'ast FunctionArg<X>) {
            if self.error.is_some() {
                return;
            }
            self.check_span("visit_function_arg", node.span());
            if let Some(name) = node.name {
                self.check_symbol(name);
            }
            visit::walk_function_arg(self, node);
        }

        fn visit_named_operator_expr(&mut self, node: &'ast NamedOperatorExpr<X>) {
            if self.error.is_some() {
                return;
            }
            self.check_span("visit_named_operator_expr", node.span());
            self.check_symbol(node.op);
            visit::walk_named_operator_expr(self, node);
        }

        fn visit_parameter_kind(&mut self, node: &'ast ParameterKind) {
            if self.error.is_some() {
                return;
            }
            if let ParameterKind::Named { name, .. } = node {
                self.check_symbol(*name);
            }
            visit::walk_parameter_kind::<Self, X>(self, node);
        }
    }

    // Gated on `serde-deserialize`, not merely `test`: `DocumentValidator`, its
    // span-checking `Visit` impl, and the `Visit` trait import all live behind that
    // feature, but the parent `serde_impls` module is `any(serialize, deserialize)` —
    // so a serialize-without-deserialize build (the platform macOS lane's four-package
    // unification) would otherwise compile this test against items that don't exist.
    #[cfg(all(test, feature = "serde-deserialize"))]
    mod tests {
        use super::*;
        use crate::ast::generated::NodeIdWalk;
        use crate::parser::{TestDialect, parse_with};

        /// The hand-mirrored `check_spans!` list must cover exactly the AST's real
        /// span-bearing node set. `NodeIdWalk` is generated from the node types, so
        /// its `Meta` count is that set's ground truth: if a node type is added and
        /// the `check_spans!` list is not updated, the validator visits fewer nodes
        /// than `NodeIdWalk` records here and this fails — the drift guard that lets
        /// the span check stay a hand-written list without a silent coverage hole.
        #[test]
        fn span_walk_covers_every_node() {
            // A deliberately broad tree: DDL, DML, a rich SELECT (joins, functions,
            // CASE, window frame, subquery, IN-list), and session/transaction
            // statements, so most node types appear at least once.
            let sql = "CREATE TABLE t (id INT PRIMARY KEY, name TEXT); \
                       INSERT INTO t (id, name) VALUES (1, 'a'); \
                       UPDATE t SET name = 'b' WHERE id = 1; \
                       DELETE FROM t WHERE id > 0; \
                       SELECT a + 1 AS n, count(DISTINCT b), \
                       CASE a WHEN 1 THEN b ELSE c END, \
                       avg(a) OVER (ORDER BY b ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) \
                       FROM s.t AS t JOIN u ON t.a = u.b \
                       WHERE a IN (SELECT x FROM w) ORDER BY n";
            let parsed =
                parse_with(sql, crate::ParseConfig::new(TestDialect)).expect("rich corpus parses");

            let mut walk = NodeIdWalk::default();
            let mut validator = DocumentValidator {
                source_len: parsed.source().len() as u32,
                resolver: parsed.resolver(),
                visited: 0,
                error: None,
            };
            for statement in parsed.statements() {
                walk.visit_statement(statement);
                validator.visit_statement(statement);
            }

            assert!(validator.error.is_none(), "valid tree must not fault");
            assert_eq!(
                validator.visited,
                walk.metas.len(),
                "the check_spans! list drifted from the generated node set: the \
                 validator visited {} span-bearing nodes but NodeIdWalk recorded {}",
                validator.visited,
                walk.metas.len(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interner::Interner;
    use crate::parser::{ParseConfig, TestDialect, parse_rc_with, parse_with};
    use std::rc::Rc;

    /// A root over arbitrary source with an empty resolver and no statements —
    /// enough to exercise the source-derived line index in isolation.
    fn parsed_for(source: &str) -> Parsed {
        Parsed::new(
            Arc::from(source),
            Interner::new().freeze(),
            Vec::new(),
            StringLiteralSyntax::ANSI,
        )
    }

    #[test]
    fn line_col_maps_offsets_across_lines() {
        let parsed = parsed_for("ab\ncde\nz");

        assert_eq!(parsed.line_col(0), (0, 0));
        assert_eq!(parsed.line_col(2), (0, 2));
        assert_eq!(parsed.line_col(3), (1, 0));
        assert_eq!(parsed.line_col(7), (2, 0));
        // The EOF offset (one past the last byte) resolves on the final line.
        assert_eq!(parsed.line_col(8), (2, 1));
    }

    #[test]
    fn empty_input_has_a_single_origin_line() {
        let parsed = parsed_for("");

        assert_eq!(parsed.line_col(0), (0, 0));
    }

    #[test]
    fn line_index_is_built_once_and_reused() {
        let parsed = parsed_for("a\nb");

        // The cached index is the same instance on every call, not rebuilt.
        assert!(std::ptr::eq(parsed.line_index(), parsed.line_index()));
    }

    #[test]
    fn span_line_col_spans_lines_and_skips_synthetic() {
        let parsed = parsed_for("ab\ncde");

        assert_eq!(
            parsed.span_line_col(Span::new(1, 5)),
            Some(((0, 1), (1, 2)))
        );
        assert_eq!(parsed.span_line_col(Span::SYNTHETIC), None);
    }

    #[test]
    fn parsed_arc_root_stays_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        // Caching the line index must not regress the ADR-0001 Send + Sync tier.
        assert_send_sync::<Parsed>();
    }

    #[test]
    fn public_roots_are_static_and_never_borrow_the_input() {
        fn assert_static<T: 'static>() {}
        // Both public ownership tiers (ADR-0001) are `'static`: `parse_with` and
        // `parse_rc_with` materialize an owned store from the `&str` input rather
        // than borrowing it, so no public root threads the caller's source lifetime
        // into the AST. `Send + Sync` above does not by itself imply `'static`.
        assert_static::<Parsed>(); // the Arc<str> default tier
        assert_static::<Parsed<Rc<str>>>();
    }

    #[test]
    fn arc_root_parses_and_renders_canonical_sql() {
        // parse_with is the Arc tier (the default store), rendered canonically.
        let parsed = parse_with("select 1, a", crate::ParseConfig::new(TestDialect))
            .expect("Arc root parses");
        assert_eq!(parsed.source(), "select 1, a");
        assert_eq!(format!("{parsed}"), "SELECT 1, a");
    }

    #[test]
    fn rc_root_parses_and_renders_canonical_sql() {
        // parse_rc_with names the single-thread Rc tier (ADR-0001); same render.
        let parsed =
            parse_rc_with("select 1, a", ParseConfig::new(TestDialect)).expect("Rc root parses");
        assert_eq!(parsed.source(), "select 1, a");
        assert_eq!(format!("{parsed}"), "SELECT 1, a");
    }

    #[test]
    fn into_statements_yields_a_structure_only_send_vec() {
        fn assert_send<T: Send>() {}
        // ADR-0001's third tier: a bare Vec<Statement> is Send and structure-only.
        assert_send::<Vec<Statement>>();

        let parsed =
            parse_with("SELECT 1; SELECT 2", crate::ParseConfig::new(TestDialect)).expect("parses");
        let statements = parsed.into_statements();
        // Source and resolver are dropped; only statement structure survives.
        assert_eq!(statements.len(), 2);
        assert!(matches!(statements[0], Statement::Query { .. }));
    }

    #[test]
    fn display_joins_statements_and_renders_empty_input() {
        // Mirrors render_statements: canonical per statement, joined by "; ".
        let parsed =
            parse_with("select a; select b", crate::ParseConfig::new(TestDialect)).expect("parses");
        assert_eq!(format!("{parsed}"), "SELECT a; SELECT b");
        assert_eq!(parsed.to_string(), "SELECT a; SELECT b");

        // No statements renders to the empty string (no stray separators).
        let empty =
            parse_with("  ; ;  ", crate::ParseConfig::new(TestDialect)).expect("only separators");
        assert!(empty.statements().is_empty());
        assert_eq!(format!("{empty}"), "");
    }

    #[test]
    fn default_parse_homes_no_trivia_on_the_root() {
        // The off path (the common `parse_with`): trivia is never captured, so the
        // root's index is empty even when the source is full of comments/whitespace.
        // This is the zero-overhead-when-off proof at the root API: nothing to query.
        let parsed = parse_with(
            "SELECT /* c */ 1 -- note\n",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("parses");
        assert!(
            parsed.trivia().is_empty(),
            "default parse captures no trivia"
        );
        assert!(parsed.trivia_in(Span::new(0, 100)).is_empty());
        assert!(parsed.trivia_before(15).is_empty());
    }

    #[test]
    fn trivia_config_homes_recoverable_trivia_on_the_root() {
        use crate::tokenizer::TriviaKind::{BlockComment, LineComment, Whitespace};

        let src = "SELECT /* c */ 1 -- note\n";
        let parsed = parse_with(
            src,
            crate::ParseConfig::new(TestDialect).capture_trivia(true),
        )
        .expect("parses with trivia");

        // Every comment/whitespace run is recoverable from the root, in source order.
        let kinds: Vec<_> = parsed.trivia().iter().map(|r| r.kind()).collect();
        assert_eq!(
            kinds,
            [
                Whitespace,   // after SELECT
                BlockComment, // /* c */
                Whitespace,   // before 1
                Whitespace,   // before --
                LineComment,  // -- note
                Whitespace,   // trailing newline
            ],
        );

        // The block comment's span slices back to its exact text (zero-copy, ADR-0005).
        let block = parsed.trivia()[1];
        assert_eq!(block.kind(), BlockComment);
        let span = block.span();
        assert_eq!(&src[span.start() as usize..span.end() as usize], "/* c */");
    }

    #[test]
    fn root_trivia_queries_recover_runs_by_offset() {
        use crate::tokenizer::TriviaKind::{BlockComment, LineComment};

        //            0         1         2
        //            0123456789012345678901234567
        let src = "SELECT /* c */ 1 -- note\n";
        let parsed = parse_with(
            src,
            crate::ParseConfig::new(TestDialect).capture_trivia(true),
        )
        .expect("parses with trivia");

        // `trivia_in`: only the block comment lies fully inside the SELECT..1 gap.
        let inner = parsed.trivia_in(Span::new(7, 14));
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].kind(), BlockComment);

        // `trivia_before`: the leading trivia of the `1` token (which starts at 15)
        // is the contiguous run ` /* c */ ` ending right at it.
        let leading = parsed.trivia_before(15);
        assert_eq!(
            leading.first().map(|r| r.span().start()),
            Some(6),
            "the leading run reaches back to just after SELECT",
        );
        assert_eq!(
            leading.last().map(|r| r.span().end()),
            Some(15),
            "and ends exactly at the token",
        );

        // The line comment is recoverable as the leading trivia ending at the newline.
        let before_newline = parsed.trivia_before(24);
        assert!(before_newline.iter().any(|r| r.kind() == LineComment));
    }

    #[test]
    fn trivia_capture_does_not_change_statement_structure() {
        // Out-of-band: the parsed statements are identical with capture on or off, so
        // structural equality is unaffected — trivia lives only on the root.
        let src = "SELECT a, /* x */ b FROM t -- trailing";
        let plain = parse_with(src, crate::ParseConfig::new(TestDialect)).expect("plain");
        let with_trivia = parse_with(
            src,
            crate::ParseConfig::new(TestDialect).capture_trivia(true),
        )
        .expect("with trivia");

        assert_eq!(plain.statements(), with_trivia.statements());
        assert!(plain.trivia().is_empty());
        assert!(!with_trivia.trivia().is_empty());
    }

    #[test]
    fn capture_trivia_config_is_deterministic() {
        use crate::parser::{ParseConfig, parse_with};

        // Equivalent `ParseConfig` values must capture byte-for-byte identical trivia.
        let src = "SELECT /* c */ a -- note\nFROM t";
        let first = parse_with(
            src,
            crate::ParseConfig::new(TestDialect).capture_trivia(true),
        )
        .expect("parses with trivia");
        let second = parse_with(src, ParseConfig::new(TestDialect).capture_trivia(true))
            .expect("parses with trivia");
        assert!(!second.trivia().is_empty());
        assert_eq!(first.trivia(), second.trivia());

        // Default options (`capture_trivia: false`) stay trivia-free, matching
        // `parse_with`'s zero-cost-when-off contract.
        let off = parse_with(src, ParseConfig::new(TestDialect)).expect("parses without trivia");
        assert!(off.trivia().is_empty());
    }

    #[test]
    fn clause_marks_capture_kinds_owners_and_offsets() {
        use crate::ast::SetExpr;
        use crate::parser::ClauseKw;

        // A statement with every SELECT-body clause and a query-tail ORDER BY/LIMIT.
        let src = "SELECT a FROM t WHERE b GROUP BY c HAVING d ORDER BY e LIMIT 1";
        let parsed = parse_with(
            src,
            crate::ParseConfig::new(TestDialect).capture_trivia(true),
        )
        .expect("parses with clause marks");

        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let query_id = query.meta.node_id;
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let select_id = select.meta.node_id;

        let marks = parsed.clause_marks();
        assert_eq!(
            marks.iter().map(ClauseMark::kind).collect::<Vec<_>>(),
            [
                ClauseKw::From,
                ClauseKw::Where,
                ClauseKw::GroupBy,
                ClauseKw::Having,
                ClauseKw::OrderBy,
                ClauseKw::Limit,
            ],
        );

        // Each mark's offset is the source byte position of its keyword.
        let at = |kw: &str| src.find(kw).expect("keyword present") as u32;
        assert_eq!(marks[0].offset(), at("FROM"));
        assert_eq!(marks[1].offset(), at("WHERE"));
        assert_eq!(marks[2].offset(), at("GROUP BY"));
        assert_eq!(marks[3].offset(), at("HAVING"));
        assert_eq!(marks[4].offset(), at("ORDER BY"));
        assert_eq!(marks[5].offset(), at("LIMIT"));

        // SELECT-body clauses are owned by the `Select`; the query-tail clauses by the
        // enclosing `Query`.
        assert_eq!(marks[0].owner(), select_id);
        assert_eq!(marks[1].owner(), select_id);
        assert_eq!(marks[2].owner(), select_id);
        assert_eq!(marks[3].owner(), select_id);
        assert_eq!(marks[4].owner(), query_id);
        assert_eq!(marks[5].owner(), query_id);

        // Recorded in source order, so the index is sorted by offset (the query
        // contract) — asserted here, not just relied on.
        assert!(marks.windows(2).all(|w| w[0].offset() <= w[1].offset()));

        // `clause_marks_in` over the Select's own span recovers exactly its four body
        // clauses; the query-tail ORDER BY/LIMIT lie past the Select and are excluded.
        let in_select = parsed.clause_marks_in(select.meta.span);
        assert_eq!(
            in_select.iter().map(ClauseMark::kind).collect::<Vec<_>>(),
            [
                ClauseKw::From,
                ClauseKw::Where,
                ClauseKw::GroupBy,
                ClauseKw::Having,
            ],
        );
        // Over the whole-statement span, every mark is recovered.
        assert_eq!(parsed.clause_marks_in(query.meta.span).len(), 6);
    }

    #[test]
    fn default_parse_homes_no_clause_marks() {
        // The off path (`parse_with`): clause marks ride the trivia opt-in, so a
        // default parse records none even for a fully-claused statement — the
        // zero-overhead-when-off proof at the root API.
        let parsed = parse_with(
            "SELECT a FROM t WHERE b GROUP BY c ORDER BY d LIMIT 1",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("parses");
        assert!(
            parsed.clause_marks().is_empty(),
            "default parse captures no clause marks"
        );
        assert!(parsed.clause_marks_in(Span::new(0, 100)).is_empty());
    }

    #[test]
    fn nested_query_clause_marks_are_owned_by_the_inner_nodes() {
        use crate::ast::SetExpr;
        // The subquery's WHERE/FROM must belong to the *inner* Select, not the outer,
        // so a formatter anchors an inner-clause comment to the inner node.
        let src = "SELECT a FROM t WHERE a IN (SELECT x FROM u WHERE y) ORDER BY a";
        let parsed = parse_with(
            src,
            crate::ParseConfig::new(TestDialect).capture_trivia(true),
        )
        .expect("parses");

        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let outer_query_id = query.meta.node_id;
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let outer_select_id = select.meta.node_id;

        let marks = parsed.clause_marks();
        let owner_at = |offset: u32| {
            marks
                .iter()
                .find(|m| m.offset() == offset)
                .unwrap_or_else(|| panic!("a mark at offset {offset}"))
                .owner()
        };

        let outer_where = src.find("WHERE").expect("outer WHERE") as u32;
        let inner_where = src.rfind("WHERE").expect("inner WHERE") as u32;
        let inner_from = src.rfind("FROM").expect("inner FROM") as u32;
        let order_by = src.find("ORDER BY").expect("ORDER BY") as u32;

        // Outer WHERE and the query-tail ORDER BY belong to the outer nodes.
        assert_eq!(owner_at(outer_where), outer_select_id);
        assert_eq!(owner_at(order_by), outer_query_id);

        // The inner WHERE is owned by a *different* node — the subquery's own Select,
        // not the outer Select or the outer Query.
        let inner_owner = owner_at(inner_where);
        assert_ne!(inner_owner, outer_select_id);
        assert_ne!(inner_owner, outer_query_id);
        // ...and the inner FROM shares that same inner-Select owner.
        assert_eq!(owner_at(inner_from), inner_owner);
    }
}
