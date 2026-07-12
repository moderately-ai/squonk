# Architecture Decision Records — squonk

These ADRs capture the design of **squonk** — a from-scratch, fast, multi-dialect, maintainable Rust SQL tokenizer + parser + AST, designed as the parsing base for a downstream SQL rewrite engine.

They were derived from three inputs: a verified static-analysis survey of `apache/datafusion-sqlparser-rs` ("Wave 1"), web research on Rust parsing/AST/testing techniques ("Wave 2"), and a focused deep-research on bidirectional/lens parsing. Each ADR groups one coherent decision area; the underlying atomic decisions (referenced as **A1–A36**) and their full blow-by-blow rationale live in the design tracker.

| # | Decision | Atoms |
|---|----------|-------|
| [0001](0001-owned-root-ast.md) | Owned-root AST & source ownership | A1 |
| [0002](0002-node-metadata-spans.md) | Node metadata: byte-range spans, NodeId, the Meta wrapper | A2–A5, A9 |
| [0003](0003-identifier-interning.md) | Identifier interning & Symbol identity | A6–A8 |
| [0004](0004-keyword-recognition.md) | Keyword recognition | A10 |
| [0005](0005-tokenizer.md) | Tokenizer: hand-written zero-copy cursor | A11–A14 |
| [0006](0006-literals.md) | Literal representation | A15 |
| [0007](0007-ast-memory-layout.md) | AST memory layout: owned Box tree | A16–A18 |
| [0008](0008-operator-precedence.md) | Operator precedence: one binding-power table | A19–A20 |
| [0009](0009-ast-extensibility.md) | AST extensibility: the Other(X) seam & Dialect::Ext | A21–A22 |
| [0010](0010-rendering.md) | Rendering: ctx-carrying renderer & render modes | A23–A24 |
| [0011](0011-dialect-as-data.md) | Dialect-as-data: FeatureSet, canonical AST, "generic" = ANSI | A25–A26 |
| [0012](0012-parser-engine.md) | Parser engine: Parser<D>, hand-RD + Pratt | A27 |
| [0013](0013-schema-codegen.md) | Schema-driven codegen: xtask, not proc-macro | A28 |
| [0014](0014-testing-strategy.md) | Testing: AST-as-oracle, structural eq, proptest, fuzz | A29–A31 |
| [0015](0015-source-of-truth-testing.md) | Source-of-truth differential testing & dialect milestones | A32–A35 |
| [0016](0016-perf-gating.md) | Performance & allocation gating | A36 |
| [0017](0017-engineering-policies.md) | Engineering policies: dependency minimalism & local-runnable checks | — |
| [0018](0018-rejected-bidirectional-parsing.md) | Considered & rejected: bidirectional/lens parsing | — |
| [0019](0019-oss-fuzz-readiness.md) | OSS-Fuzz onboarding: deferred (no-go for now) | — |
| [0020](0020-lossless-cst.md) | Lossless CST: deferred (no-go for now) | — |

**Status legend:** Accepted · Superseded · Deprecated. All current ADRs are Accepted (0001–0018 on 2026-06-26; 0019–0020 on 2026-06-29).

See [published-dependency-policy.md](published-dependency-policy.md) for the enforced published-crate dependency surface — each runtime dependency of `squonk`/`squonk-ast` with its purpose, transitive cost, and ADR-0017 justification, plus the `cargo xtask deps` regression gate that keeps it lean.
