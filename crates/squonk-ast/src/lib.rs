// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

// docs.rs feature-gate banners (ticket docs-rs-feature-gate-banners): turn on rustdoc's `doc_cfg` for the nightly docs.rs build only — gated by the `docsrs` cfg docs.rs sets — so feature-gated items render an "Available on crate feature X" banner; auto_cfg is on by default at crate level once `doc_cfg` is enabled (the old `doc_auto_cfg` gate was merged into `doc_cfg` and removed in Rust 1.92), and the whole thing is inert on the pinned stable toolchain (the cfg is never set there).
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
// Compiler-enforced, uncappable: the pure AST vocabulary has no perf hatch to reserve,
// so it takes the stronger `forbid` where the workspace-wide lint only `deny`s
// unsafe_code (`Cargo.toml`, `[workspace.lints.rust]`) to leave the parser a reviewed
// opt-in. A green build of this crate is the proof no unsafe slips in.
#![forbid(unsafe_code)]
//! Abstract syntax tree for `squonk`.
//!
//! This crate holds the dialect-agnostic SQL AST: the node vocabulary ([`vocab`]),
//! node types ([`ast`]), dialect data ([`dialect`]), and precedence data
//! ([`precedence`]). It deliberately depends on nothing in the parser, so
//! downstream tooling (rewriters, linters, formatters) can build on the AST
//! without pulling in the tokenizer/parser. To *parse* SQL into these nodes, depend
//! on the `squonk` crate, which re-exports this one as `squonk::ast` and
//! carries the end-to-end parse → inspect → render examples.
//!
//! # Design records
//!
//! The architectural decisions behind this AST — the owned tree, byte-range spans,
//! interned identifiers, and the canonical-shape-plus-tag policy among them — are
//! recorded as ADRs in the repository's
//! [`docs/adr`](https://github.com/moderately-ai/squonk/tree/main/docs/adr)
//! directory.
//!
//! # Rendering
//!
//! Rendering is configured by [`render::RenderConfig`]: a [`mode`](render::RenderMode)
//! (canonical, fully parenthesized, or redacted) and a
//! [`spelling`](render::RenderSpelling) (source-preserving or target-dialect). The
//! `squonk` crate threads these through a [`render::RenderCtx`] over a parsed
//! tree; see that crate for the runnable parse → render flows.
//!
//! ```
//! use squonk_ast::render::{RenderConfig, RenderMode, RenderSpelling};
//!
//! let config = RenderConfig::default();
//! assert_eq!(config.mode, RenderMode::Canonical);
//! assert_eq!(config.spelling, RenderSpelling::PreserveSource);
//! ```

pub mod ast;
pub mod dialect;
pub mod generated;
pub mod precedence;
pub mod render;
pub mod vocab;

/// Deserialization recursion-depth guard, available only under the `serde` feature.
#[cfg(feature = "serde-deserialize")]
pub mod serde_depth;

pub use ast::*;
pub use dialect::{
    Keyword, KeywordSet, RESERVED_BARE_ALIAS, RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME,
    RESERVED_TYPE_NAME, lookup_keyword,
};
pub use vocab::{FoldedSymbol, LineIndex, Meta, NodeId, Resolver, SourceStore, Span, Symbol};
