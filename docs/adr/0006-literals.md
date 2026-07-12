# ADR-0006: Literal representation

- **Status:** Accepted (2026-06-26)
- **Atoms:** A15

## Context

Literals (string / number / bool / null / date / interval) are where allocation and dependencies tend to creep into an AST (owned strings, a `bigdecimal` dep, Display panics on representable values).

## Decision

A literal node is **`Meta` + a `LiteralKind` tag** — `Meta.span` *is* the literal's source extent (no separate field), so a `Literal` carries **no owned data and no interning** and is pinned to ≤24 B (`LITERAL_SIZE_BUDGET`). `LiteralKind` is not a bare 1-byte tag: some variants carry a small payload the span alone can't recover (`Boolean(bool)`, `Time`/`Timestamp { time_zone }`, `Interval { fields, precision }`, `BitString { radix }`, `Money`). The typed value is otherwise **materialized lazily** from `source[span]` via typed accessors (`as_i64(source)`, `as_str(source) -> Cow`, `as_decimal_text(source)`). Valueless-from-span kinds like `Null` are the tag alone.

## Consequences

- **Dependency-free AST** — arbitrary-precision numbers don't drag `bigdecimal` into the published crate; the consumer materializes into its own numeric type.
- **Exact-formatting round-trip** — rendering a literal emits `source[span]` verbatim, so `0x1F`, `1_000`, `1.5e3`, and exact escape sequences survive byte-for-byte (retiring the prior art's Display-panics-on-literal bug).
- **Zero work until read** — pure parse/rewrite passes that don't inspect literal *values* pay nothing.
- Same constraint as ADR-0001: a literal detached from its source can't materialize or render — it needs the render ctx.

## Amendment (2026-06-27): signed numeric literals are *not* folded

A signed numeric literal (`-1`, `+2.5`) parses to `UnaryOp(sign, Literal)` over the **unsigned** literal — the sign is an ordinary operator node, never baked into the literal token. This falls out of the lazy-text decision and is worth stating because it diverges from how eager parsers represent the same syntax:

- PostgreSQL's grammar folds `'-' ICONST` / `'-' FCONST` into a single signed constant in its raw parse tree. Eager parsers *must* do this to represent `INT_MIN`: `-2147483648` cannot be formed as `negate(2147483648)` because the unsigned magnitude overflows `i32` before negation. The sign has to live inside the constant.
- Our literal is **source text materialized on demand** into the *consumer's* numeric type (this ADR), so that overflow never arises — the consumer applies the sign while parsing `source[span]` into `i32`/`i64`/bignum. Folding would buy nothing and would special-case the lexer/parser, so we keep the uniform `UnaryOp` shape (consistent with `- a`, `- (a + b)`, `- f(x)`).

Consequence for differential testing (ADR-0015): our `UnaryOp(±, numeric Literal)` and PostgreSQL's folded signed constant are **representation-equivalent**, so the protobuf→canonical structural mapping must normalize one to the other rather than reporting a divergence. This is a mapping concern (conformance), not a parser change. Surfaced by the PG differential fuzz loop (`prod-fuzz-differential-loop`); normalization tracked by `prod-pg-map-expressions`.

## Interconnects

- code: `crates/squonk-ast/src/ast/literal.rs` — `Literal`, `LiteralKind`
- invariant: a `Literal` is `{ kind, meta }` with a payload-carrying `LiteralKind` and no owned string data — values materialize lazily from the source span; a signed literal is a `UnaryOp` over the unsigned literal, never a stored sign.
- xtask: none — the `LITERAL` size budget is compile-time pinned by `crates/squonk-ast/src/generated/size_asserts.rs`.

## References

Atom A15. Interning literals is rejected (ADR-0003) — mostly unique, so near-zero dedup at full hash+insert cost.
