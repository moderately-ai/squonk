# squonk-ast

The dialect-agnostic SQL abstract syntax tree behind [`squonk`][parser]: the node types, byte-range `Span`s, interned identifier `Symbol`s, the dialect `FeatureSet` data, the one binding-power table, and the context-carrying `Render` trait.

## When to depend on this crate

Most users should depend on [`squonk`][parser] instead. It re-exports this whole crate as `squonk::ast`, so parsing, inspecting, and rendering all reach these types without a second dependency.

Depend on `squonk-ast` **directly** only when you consume or produce the AST but never parse it yourself — a rewriter, linter, or formatter that receives an already-parsed tree, or a tool that builds nodes by hand. Taking the AST crate alone keeps the tokenizer and parser engine out of your dependency graph.

## Dependency-free by policy

This crate is kept effectively dependency-free: the only non-optional dependency is the small `thin-vec` leaf (a one-word child-sequence container), and `serde` support is opt-in behind its feature. Downstream tooling can therefore pin the AST vocabulary without inheriting a parser or a dependency tree.

## Documentation

- API reference: [docs.rs/squonk-ast][docs]
- The end-to-end parse → inspect → render examples live on the [`squonk`][parser] crate, which drives these nodes.
- Repository and design notes: [the squonk workspace][repo]

## License

Licensed under the [MIT License](https://github.com/moderately-ai/squonk/blob/main/LICENSE).

[parser]: https://docs.rs/squonk
[docs]: https://docs.rs/squonk-ast
[repo]: https://github.com/moderately-ai/squonk
