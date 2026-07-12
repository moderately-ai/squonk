# ADR-0005: Tokenizer — hand-written zero-copy cursor

- **Status:** Accepted (2026-06-26)
- **Atoms:** A11, A12, A13, A14

## Context

The tokenizer is the hot loop and the root of the prior art's allocation problem (a heap `String` per token, in-band whitespace tokens, a per-char `&dyn Dialect` vtable). It must be fast, dialect-parameterized, and zero-copy.

## Decision

- **Zero-copy tokens.** Tokens borrow `&'a str` from the source; the `'a` is strictly parser-internal (identifiers are interned, literals are span-recorded, before the slice drops). `Cow<'a, str>` only on an actual escape — and literal-unescaping is deferred to lazy materialization (ADR-0006), so the only transient owned case is a quoted identifier with an embedded escaped delimiter.
- **Hand-written byte-offset cursor** (`Cursor { src: &'a str, pos: u32 }`, rustc_lexer style) emitting `(kind, Span)` tokens — **not `logos`.** `logos`'s DFA cannot express non-regular Postgres `$tag$…$tag$` dollar-quoting or nested comments, and its closed token enum fights cross-dialect extensibility. `logos` is kept as a perf *reference/fallback* (benchmark backlog #4).
- **`[u8; 256]` class-bitset table**, `const` for builtin dialects, built once for custom — the hot loop is `CLASS[b] & MASK`, killing the per-char vtable. Bytes ≥ 0x80 route to a permissive (dep-free) Unicode slow path; `unicode-ident` (a leaf crate) is an optional precision upgrade.
- **Token-index cursor** over a lazily-growable per-statement buffer (random-access speculation/backtracking), a `statements()` streaming iterator (bounded memory across a script), **trivia out-of-band** (offset-recoverable), and a structured `ParseError { span, expected, found }` threaded through an **error-sink seam** (fail-fast v1, resilience-ready). A lossless CST / resilient parser is deferred (Phase 6) but not foreclosed.

## Error-type evolvability seam (amended 2026-07-11, pre-first-publish)

The structured `ParseError` and its kind enums are frozen by the 1.0 API. Before the first publish — the last free moment — the error surface was reshaped so diagnostics can grow post-1.0 without a major bump. This is a deliberate amendment recorded here and in [`stable-api.md`](../stable-api.md)'s exhaustiveness inventory.

- **`#[non_exhaustive]` on the kind enums.** `ParseErrorKind`, `LexErrorKind`, and the bindings' `BindingTokenKind` are growth axes — new robustness guards, new lexical faults from future dialect scanners, new token categories from a recovering tokenizer — so they reserve additive variants for a minor release instead of freezing the set. In-crate matches stay exhaustive with no wildcard (the attribute only forces a `_` arm downstream); construction of individual variants is unaffected, so the wire-schema test still builds `BindingTokenKind` instances directly. `BindingTokenKind` being non-exhaustive also aligns the Rust surface with the wire contract, which already classes "a new variant on a `#[non_exhaustive]` enum" as additive-compatible ([`schema-contract.md`](../schema-contract.md)).
- **Carried lexical kind.** `ParseErrorKind::Lexical(LexErrorKind)` carries the tokenizer's machine-matchable kind across the `From<LexError>` widening at the `?` boundary, replacing the prior collapse to `kind = Syntax`. The span and a faithful message are preserved; both bindings surface a distinct `ParseDiagnostic.kind` (via `LexErrorKind::machine_kind()`, a stable snake_case string kept separate from the human `message()`), so an editor can tell an unterminated string from a grammar mismatch programmatically. Because `ParseDiagnostic.kind` is a `&'static str`, the new strings are additive with no wire-shape change.
- **Hint channel via a private field.** `ParseError` gains a private `hint: Option<Cow<'static, str>>`, reached only through the `hint()` accessor and set through the `with_hint()` builder. The private-field shape was chosen over `#[non_exhaustive]` on the struct because it is strictly more encapsulating: a private field already closes downstream struct-literal construction and forces downstream matches to end in `..`, so any *later* field is absorbed by that rest pattern — the struct is future-proofed without committing the hint's representation to the public surface (its storage type can change freely behind the accessor). Internal callers construct only through `ParseError::{new, recursion_limit_exceeded, lexical}`, so their ergonomics are unchanged; v1 ships the channel empty and a follow-up dialect-aware hinting pass populates it.

## Consequences

- Per-token allocation, the in-band-whitespace blow-up, and the per-char vtable are all eliminated. Databend's hand-built zero-copy SQL lexer reports ~3.3× faster parsing at ~1.2×-of-input memory.
- A hand cursor risks off-by-one/UTF-8 bugs — mitigated by proptest + fuzz + a differential token-stream oracle (ADR-0014/0015).
- Streaming via `statements()` is the real answer to huge inputs (not wider spans, ADR-0002).

## Interconnects

- code: `crates/squonk/src/tokenizer/token.rs` — `Token`, `TokenKind`; `crates/squonk/src/tokenizer/mod.rs` — `tokenize`
- invariant: `Token` is `Copy`, borrows nothing, and pins `size_of::<Token>() == 12`; no `logos` in the published surface.
- xtask: `cargo xtask deps`; pinned by the tokenizer `Token` size test.

## References

Atoms A11–A14. rustc_lexer; serde_json borrow-on-no-escape; `memchr` for literal/comment inner loops.
