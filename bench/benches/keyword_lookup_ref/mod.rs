// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared harness for the ADR-0004 / ADR-0017 keyword-recognition comparison: the
//! dependency-free generated lookup vs a `phf` perfect-hash map over the full
//! ANSI/PostgreSQL keyword inventory. Shared by the wall-clock bench and the parity
//! test, mirroring how `logos_ref/` serves the cursor-vs-logos comparison.
//!
//! Why this exists. ADR-0004 chose dep-free, codegen-generated keyword recognition
//! and kept `phf` only as a *measured* fallback ("backlog #3: dep-free generated vs
//! phf"), to be adopted only if lookup cost ever shows up in a profile. ADR-0017
//! names `phf` as the archetypal rejected dep — it drags `phf_shared` + `siphasher`
//! — so the published crates stay dependency-free. This harness is the concrete
//! measurement behind those decisions: over the *full* inventory (far past the M1
//! subset), does the perfect hash beat the length-bucketed binary search by enough
//! to justify the subtree?
//!
//! What is compared, and why it is fair:
//! - `lookup_keyword`     — the REAL generated path (`squonk-sourcegen` renders
//!   it into `OUT_DIR` from the same `keyword_data/*.csv`): a case-insensitive,
//!   allocation-free length-bucketed binary search over sorted byte tables.
//! - `lookup_keyword_phf` — a perfect-hash probe over the SAME inventory. It lowers
//!   the word into the SAME fixed stack buffer the generated path uses, then hashes
//!   the lowered *bytes* (the map is keyed on `&[u8]`, so there is no extra UTF-8
//!   re-validation). Both are case-insensitive and allocation-free, so the only
//!   difference measured is bucketed-binary-search vs perfect-hash.
//!
//! Both lookups are allocation-free by construction, so there is no heap comparison
//! to make (unlike cursor-vs-logos); the runtime axis is purely lookup throughput.
//! The build-time axes the ticket also asks for — cold compile, binary size,
//! dependency tree — are reproduced with these commands (none of which run in the
//! default gates, since every `phf-compare` target is `required-features`-gated):
//!
//! ```text
//! # dependency subtree a production swap would add (runtime crates):
//! cargo tree -p squonk-bench --features phf-compare -e normal -i phf
//! # cold compile cost of those crates (read the phf_* rows):
//! cargo clean && cargo build -p squonk-bench --features phf-compare --timings
//! # binary-size delta (stripped release; the std baseline cancels in the diff):
//! cargo build -p squonk-bench --features phf-compare --release \
//!     --example keyword_size_generated --example keyword_size_phf
//! ```
//!
//! Each consumer uses a different slice of this module (the bench never asserts, the
//! test never builds the report, each size example calls only one lookup), so a
//! module-level `allow(dead_code, unused_imports)` keeps `-D warnings` green — the
//! same convention `logos_ref/mod.rs` uses for its partially-used shared surface.
#![allow(dead_code, unused_imports)]

// The REAL dependency-free generated lookup over the full inventory (ADR-0004),
// rendered by the production codegen in `build.rs`. Defines `Keyword`,
// `lookup_keyword`, the per-length bucket tables, and the per-dialect reserved
// bitsets — the last typed as `super::KeywordSet`, satisfied by the shim below.
// `clippy::all` is allowed here because the codegen's own inner allow is stripped
// when its file header is dropped for inclusion (see `build.rs`).
#[allow(clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_keywords.rs"));
}

pub use generated::{Keyword, lookup_keyword};

// The generated reserved-bitset consts are typed `super::KeywordSet` (production
// places the lookup beside the real `KeywordSet`). This lookup-speed comparison
// never reads reserved-ness, so a zero-sized placeholder lets the generated file
// compile verbatim — keeping the lookup path under test byte-identical to what the
// codegen emits, rather than editing the generated output.
pub struct KeywordSet;

impl KeywordSet {
    pub const fn from_keywords(_: &[Keyword]) -> Self {
        Self
    }
}

// The perfect-hash map over the same inventory; defines `PHF_KEYWORDS` and
// `PHF_MAX_KEYWORD_LEN`, with `Keyword` values resolved from the re-export above.
include!(concat!(env!("OUT_DIR"), "/phf_keywords.rs"));

/// Case-insensitive, allocation-free `phf` keyword lookup — the `phf` counterpart
/// of the generated `lookup_keyword`.
///
/// Mirrors the generated path's lower-casing exactly (the same fixed stack buffer)
/// so the comparison isolates the recognition step: here a single perfect-hash
/// probe, there a length-bucketed binary search.
pub fn lookup_keyword_phf(word: &str) -> Option<Keyword> {
    if word.len() > PHF_MAX_KEYWORD_LEN {
        return None;
    }
    let mut lowered = [0u8; PHF_MAX_KEYWORD_LEN];
    for (slot, byte) in lowered.iter_mut().zip(word.as_bytes()) {
        *slot = byte.to_ascii_lowercase();
    }
    PHF_KEYWORDS.get(&lowered[..word.len()]).copied()
}

/// One probe corpus: a named list of word tokens fed to both lookups.
pub struct Probe {
    pub name: &'static str,
    pub words: &'static [&'static str],
}

/// Non-keyword identifiers — every entry must miss in both lookups (the parity test
/// asserts this so the "pure miss" scenario stays honest). Compound `snake_case`
/// names model the common failure path: most word tokens in real SQL are
/// identifiers, so miss latency dominates the hot path.
pub const IDENTIFIERS: &[&str] = &[
    "user_id",
    "created_at",
    "updated_at",
    "customer_email",
    "order_total",
    "line_item",
    "shipping_addr",
    "txn_ref",
    "acct_no",
    "prod_sku",
    "qty_on_hand",
    "unit_price_usd",
    "is_active",
    "uuid_pk",
    "fk_owner",
    "foo",
    "bar",
    "baz",
    "xyzzy",
    "col_1",
];

/// A realistic word-token stream (keywords and identifiers interleaved as they
/// appear in queries), so the throughput reflects the production mix rather than an
/// all-hit or all-miss extreme. Punctuation/number tokens are omitted because they
/// never reach keyword lookup.
pub const MIXED_QUERY_WORDS: &[&str] = &[
    // SELECT id, customer_email FROM orders WHERE is_active = true ORDER BY created_at DESC
    "select",
    "id",
    "customer_email",
    "from",
    "orders",
    "where",
    "is_active",
    "true",
    "order",
    "by",
    "created_at",
    "desc",
    // INSERT INTO line_item (order_total, qty_on_hand) VALUES ...
    "insert",
    "into",
    "line_item",
    "order_total",
    "qty_on_hand",
    "values",
    // UPDATE prod_sku SET unit_price_usd = price WHERE acct_no IS NOT NULL
    "update",
    "prod_sku",
    "set",
    "unit_price_usd",
    "where",
    "acct_no",
    "is",
    "not",
    "null",
];

/// The fixed probe corpora the wall-clock bench iterates. `all_keywords` (every
/// spelling — the deepest binary-search paths) is built at run time from
/// `Keyword::ALL` by the bench, since it is far too large to spell out here.
pub const PROBES: &[Probe] = &[
    Probe {
        name: "identifiers",
        words: IDENTIFIERS,
    },
    Probe {
        name: "mixed_query",
        words: MIXED_QUERY_WORDS,
    },
];

/// A small, mixed word list the size-isolation examples loop over so the linker
/// keeps the lookup they call (and its tables) live. The exact words do not matter
/// — only that the call is reachable and not const-folded away.
pub const SIZE_PROBE: &[&str] = &[
    "select",
    "from",
    "where",
    "user_id",
    "created_at",
    "x",
    "materialized",
    "is",
];

/// The self-describing context block the bench prints, so a raw log explains the
/// tradeoff on its own (ADR-0017: state the caveat, never leave it silent).
pub fn report_header() -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# keyword recognition: dependency-free generated lookup vs phf perfect hash"
    );
    let _ = writeln!(
        out,
        "#   generated : squonk-sourcegen codegen (ADR-0004) — case-insensitive,"
    );
    let _ = writeln!(
        out,
        "#               allocation-free length-bucketed binary search, ZERO deps,"
    );
    let _ = writeln!(out, "#               shipped as plain checked-in code.");
    let _ = writeln!(
        out,
        "#   phf       : perfect-hash map over the same inventory. O(1) probe, but"
    );
    let _ = writeln!(
        out,
        "#               adds the `phf` runtime crate -> `phf_shared` + `siphasher`"
    );
    let _ = writeln!(
        out,
        "#               (ADR-0017's archetypal rejected subtree) + a `phf_codegen`"
    );
    let _ = writeln!(out, "#               build-dep.");
    let _ = writeln!(
        out,
        "#   ratio     = generated / phf  (> 1.0 ⇒ the generated lookup is slower)"
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# Inventory size: {} keywords (full ANSI/PostgreSQL union, far past the M1 set).",
        Keyword::ALL.len(),
    );
    let _ = writeln!(
        out,
        "# Both lower the word into the SAME fixed stack buffer and are allocation-"
    );
    let _ = writeln!(
        out,
        "# free, so the rows isolate bucketed-binary-search vs perfect-hash only."
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# Decision frame (ADR-0004 backlog #3): adopt phf only if its O(1) probe"
    );
    let _ = writeln!(
        out,
        "# beats the bucketed search by enough to justify the subtree. The compile /"
    );
    let _ = writeln!(
        out,
        "# size / dependency-tree cost is reproduced via the commands in this"
    );
    let _ = writeln!(
        out,
        "# module's docs; the published crates stay dependency-free either way."
    );
    out
}
