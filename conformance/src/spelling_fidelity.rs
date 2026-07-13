// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Corpus-driven spelling-fidelity ratchet: token-diff every accepted corpus
//! statement against its canonical render
//! (`spike-formatter-spelling-fidelity-inventory`).
//!
//! The structural round-trip oracle ([`corpus_roundtrip`](crate::corpus_roundtrip))
//! proves parse -> render -> re-parse lands on an *equal tree*; it is blind to the
//! render re-*spelling* the surface (`temp` -> `TEMPORARY`, a dropped redundant
//! paren), because those collapse to the same tree. A source-fidelity formatter
//! cannot tolerate that blindness, so this module measures the spelling axis
//! directly:
//!
//! 1. For every vendored corpus statement the parser accepts, tokenize the ORIGINAL
//!    source and the CANONICAL render output under the same dialect.
//! 2. Compare the two token streams textually (case-insensitively, so keyword
//!    upper-casing — the render's documented normalization — does not drown the
//!    signal). A statement is `Exact` (byte-identical token texts), `CaseOnly`
//!    (differs only by ASCII case — the keywordCase-knob class), or `Lossy`.
//! 3. Align each lossy pair (common prefix/suffix + LCS on the middle) into
//!    deleted/inserted token hunks, and aggregate hunks by *signature*
//!    (`-[temp] +[temporary]`), so every spelling-lossy construct in the corpora is
//!    counted, not hand-enumerated.
//!
//! Each known signature is classified ([`SigClass`]) per the spike's taxonomy:
//! a method false-positive to exclude, a missing spelling tag (per the
//! `TypeName`/`QuoteStyle`/`BitwiseXorSpelling` doctrine), or a structural
//! normalization a fidelity formatter cannot tolerate. An UNKNOWN signature fails
//! the gate: a new spelling-lossy construct must be triaged here before it lands —
//! that is the ratchet. The per-corpus statement counts are pinned below
//! ([`CORPUS_PINS`]); regenerate with `REWRITE=1` (the repo-wide golden convention).
//!
//! An authored probe set ([`PROBES`]) covers the synonym pairs the corpora may not
//! exercise (TEMP/TEMPORARY, optional AS, `<>` vs `!=`, …), each pinned to its
//! current fidelity verdict so a landed spelling tag flips the pin loudly and the
//! probe promotes to `Exact`.

use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;

use squonk::dialect::{BuiltinDialect, parse_builtin, tokenize_with_builtin};
use squonk::tokenizer::{Punctuation, Token, TokenKind};
use squonk_ast::render::RenderMode;

use crate::verdict_harness::{
    sqlglot_complex_statements, sqlglot_identity_lines, sqllogictest_lines,
};

// ---------------------------------------------------------------------------
// Corpora
// ---------------------------------------------------------------------------
//
// The three multi-dialect fixtures come from `verdict_harness`'s shared loaders;
// the per-engine extractions are re-`include_str!`'d here (their owning sweeps keep
// the consts private, and `include_str!` of the same file costs nothing extra).

const PG_REGRESS: &str = include_str!("../corpus/pg-regress/statements.sql");
const SQLITE_TESTSUITE: &str = include_str!("../corpus/sqlite-testsuite/statements.sql");
const DUCKDB_TESTSUITE: &str = include_str!("../corpus/duckdb-testsuite/statements.sql");
const DUCKDB_CURATED: &str = include_str!("../corpus/duckdb/statements.sql");

/// The pg-regress flat statement view: every non-`# file:` marker, non-blank line
/// (mirrors `corpus_pg_verdicts::regress_statements`, which is private to its sweep).
fn pg_regress_statements() -> Vec<&'static str> {
    PG_REGRESS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .collect()
}

/// One line-per-statement engine extraction, blank lines skipped.
fn nonblank_lines(text: &'static str) -> Vec<&'static str> {
    text.lines().filter(|l| !l.trim().is_empty()).collect()
}

/// One vendored corpus: its statements and the preset chain to try, in order (the
/// first accepting dialect renders and tokenizes the statement — the same
/// Ansi-then-Postgres fallback `corpus_partition` uses for the multi-dialect
/// fixtures; the engine extractions go straight to their fitted preset).
struct Corpus {
    name: &'static str,
    statements: fn() -> Vec<&'static str>,
    dialects: &'static [BuiltinDialect],
}

const ANSI_THEN_POSTGRES: &[BuiltinDialect] = &[BuiltinDialect::Ansi, BuiltinDialect::Postgres];

const CORPORA: &[Corpus] = &[
    Corpus {
        name: "sqlglot_identity",
        statements: sqlglot_identity_lines,
        dialects: ANSI_THEN_POSTGRES,
    },
    Corpus {
        name: "sqllogictest",
        statements: sqllogictest_lines,
        dialects: ANSI_THEN_POSTGRES,
    },
    Corpus {
        name: "sqlglot_complex",
        statements: sqlglot_complex_statements,
        dialects: ANSI_THEN_POSTGRES,
    },
    Corpus {
        name: "pg_regress",
        statements: pg_regress_statements,
        dialects: &[BuiltinDialect::Postgres],
    },
    Corpus {
        name: "sqlite_testsuite",
        statements: || nonblank_lines(SQLITE_TESTSUITE),
        dialects: &[BuiltinDialect::Sqlite],
    },
    Corpus {
        name: "duckdb_testsuite",
        statements: || nonblank_lines(DUCKDB_TESTSUITE),
        dialects: &[BuiltinDialect::DuckDb],
    },
    Corpus {
        name: "duckdb_curated",
        statements: || nonblank_lines(DUCKDB_CURATED),
        dialects: &[BuiltinDialect::DuckDb],
    },
];

// ---------------------------------------------------------------------------
// Token-stream fidelity verdict
// ---------------------------------------------------------------------------

/// One token's comparison view: the kind (for the case-only keyword/identifier
/// split) and the exact source slice its span covers.
struct Tok<'a> {
    kind: TokenKind,
    text: &'a str,
}

/// Tokenize `src` under `dialect` into comparison views, dropping trailing
/// statement-separator semicolons (corpus lines sometimes keep a trailing `;`;
/// the render never emits one).
fn toks(src: &str, dialect: BuiltinDialect) -> Result<Vec<Tok<'_>>, String> {
    let tokens: Vec<Token> = tokenize_with_builtin(src, dialect).map_err(|e| format!("{e:?}"))?;
    let mut out: Vec<Tok<'_>> = tokens
        .iter()
        .map(|t| Tok {
            kind: t.kind,
            text: &src[t.span.start() as usize..t.span.end() as usize],
        })
        .collect();
    while matches!(
        out.last().map(|t| t.kind),
        Some(TokenKind::Punctuation(Punctuation::Semicolon))
    ) {
        out.pop();
    }
    Ok(out)
}

/// How one accepted statement's canonical render compares to its source,
/// token-by-token.
enum Fidelity {
    /// Every token text is byte-identical — the construct round-trips its spelling.
    Exact,
    /// Token streams match up to ASCII case; carries how many differing positions
    /// were keywords vs identifiers/words vs anything else (the keywordCase data).
    CaseOnly {
        keyword: usize,
        ident: usize,
        other: usize,
    },
    /// The streams differ beyond case: each aligned hunk is a spelling-lossy
    /// construct.
    Lossy(Vec<Hunk>),
}

/// One aligned divergence: the original tokens the render dropped and the rendered
/// tokens it produced instead (either side may be empty). Texts keep their exact
/// case for the exemplar dump; signatures lowercase them.
struct Hunk {
    deleted: Vec<String>,
    inserted: Vec<String>,
}

impl Hunk {
    /// The stable aggregation key: `-[…] +[…]` over lowercased token texts.
    fn signature(&self) -> String {
        let lower = |v: &[String]| {
            v.iter()
                .map(|t| t.to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join(" ")
        };
        format!("-[{}] +[{}]", lower(&self.deleted), lower(&self.inserted))
    }
}

/// Compare original vs rendered token streams into a [`Fidelity`] verdict.
fn compare(orig: &[Tok<'_>], rend: &[Tok<'_>]) -> Fidelity {
    let case_insensitively_equal = orig.len() == rend.len()
        && orig
            .iter()
            .zip(rend)
            .all(|(a, b)| a.text.eq_ignore_ascii_case(b.text));
    if case_insensitively_equal {
        let (mut keyword, mut ident, mut other) = (0usize, 0usize, 0usize);
        for (a, b) in orig.iter().zip(rend) {
            if a.text != b.text {
                match a.kind {
                    TokenKind::Keyword(_) => keyword += 1,
                    TokenKind::Word | TokenKind::QuotedIdent => ident += 1,
                    _ => other += 1,
                }
            }
        }
        if keyword + ident + other == 0 {
            Fidelity::Exact
        } else {
            Fidelity::CaseOnly {
                keyword,
                ident,
                other,
            }
        }
    } else {
        Fidelity::Lossy(diff_hunks(orig, rend))
    }
}

/// Align two token streams into deleted/inserted hunks: trim the case-insensitive
/// common prefix and suffix, LCS the middle, coalesce edit runs. The middle is
/// almost always a handful of tokens; a pathological pair (quadratic-area guard)
/// degrades to one whole-middle hunk rather than an expensive DP.
fn diff_hunks(orig: &[Tok<'_>], rend: &[Tok<'_>]) -> Vec<Hunk> {
    let eq = |a: &Tok<'_>, b: &Tok<'_>| a.text.eq_ignore_ascii_case(b.text);

    let mut start = 0usize;
    while start < orig.len() && start < rend.len() && eq(&orig[start], &rend[start]) {
        start += 1;
    }
    let (mut oend, mut rend_end) = (orig.len(), rend.len());
    while oend > start && rend_end > start && eq(&orig[oend - 1], &rend[rend_end - 1]) {
        oend -= 1;
        rend_end -= 1;
    }
    let a = &orig[start..oend];
    let b = &rend[start..rend_end];

    let texts = |s: &[Tok<'_>]| s.iter().map(|t| t.text.to_owned()).collect::<Vec<_>>();
    if a.len().saturating_mul(b.len()) > 1_000_000 {
        return vec![Hunk {
            deleted: texts(a),
            inserted: texts(b),
        }];
    }

    // LCS length table over the trimmed middle (small by construction).
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[idx(i, j)] = if eq(&a[i], &b[j]) {
                lcs[idx(i + 1, j + 1)] + 1
            } else {
                lcs[idx(i + 1, j)].max(lcs[idx(i, j + 1)])
            };
        }
    }

    // Walk the table, coalescing consecutive delete/insert steps into hunks.
    let mut hunks: Vec<Hunk> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    let mut open: Option<Hunk> = None;
    while i < n || j < m {
        if i < n && j < m && eq(&a[i], &b[j]) {
            if let Some(h) = open.take() {
                hunks.push(h);
            }
            i += 1;
            j += 1;
        } else {
            let h = open.get_or_insert_with(|| Hunk {
                deleted: Vec::new(),
                inserted: Vec::new(),
            });
            // Prefer the delete when both moves preserve the LCS, so a
            // substitution reads `-[old] +[new]` deterministically.
            if j == m || (i < n && lcs[idx(i + 1, j)] >= lcs[idx(i, j + 1)]) {
                h.deleted.push(a[i].text.to_owned());
                i += 1;
            } else {
                h.inserted.push(b[j].text.to_owned());
                j += 1;
            }
        }
    }
    if let Some(h) = open.take() {
        hunks.push(h);
    }
    hunks
}

/// Parse `sql` under the first accepting dialect of `dialects`; on acceptance,
/// canonical-render it and compare token streams. `None` = no listed dialect
/// accepts (outside the measured surface). `Some(Err(_))` = the canonical render
/// does not re-tokenize under the same dialect — a render bug, gated to zero.
fn statement_fidelity(
    sql: &str,
    dialects: &[BuiltinDialect],
) -> Option<Result<(BuiltinDialect, String, Fidelity), String>> {
    for &dialect in dialects {
        let Ok(parsed) = parse_builtin(sql, dialect) else {
            continue;
        };
        let rendered = crate::render_statements(&parsed, RenderMode::Canonical);
        let orig = match toks(sql, dialect) {
            Ok(t) => t,
            Err(e) => return Some(Err(format!("original does not tokenize: {e}"))),
        };
        let rend = match toks(&rendered, dialect) {
            Ok(t) => t,
            Err(e) => {
                return Some(Err(format!(
                    "canonical render {rendered:?} does not tokenize under {dialect:?}: {e}"
                )));
            }
        };
        let fidelity = compare(&orig, &rend);
        return Some(Ok((dialect, rendered, fidelity)));
    }
    None
}

// ---------------------------------------------------------------------------
// Signature classification (the spike's four-class taxonomy)
// ---------------------------------------------------------------------------

/// Where a lossy hunk signature lands in the spike's taxonomy. Assigned by a human
/// reading the exemplars (like [`RoundtripDefect`](crate::corpus_roundtrip::RoundtripDefect));
/// the harness only detects and aggregates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SigClass {
    /// A false positive of the token-diff method, not a spelling loss: the token
    /// change is required output (binding-table parens, a lexically-mandatory
    /// respelling) or otherwise carries zero surface information.
    MethodFalsePositive,
    /// A synonym pair collapsing because no spelling tag records which synonym the
    /// source used — the class that feeds the missing-tag shortlist.
    MissingSpellingTag,
    /// The render drops or materializes surface syntax (redundant parens, implicit
    /// -> explicit rewrites): a normalization a source-fidelity formatter cannot
    /// tolerate. Flagged loudly in the report.
    StructuralNormalization,
}

/// The curated signature ledger: every lossy-hunk signature the corpora + probes
/// currently produce, with its triage class. An unlisted signature fails the gate
/// (the ratchet): triage it, add it here, and — for a real spelling loss — make
/// sure a tag/ticket exists before widening the ledger.
///
/// Triage rules for mixed hunks (LCS alignment can fuse two adjacent losses into
/// one hunk): a hunk that *drops* source parens is `StructuralNormalization` even
/// when fused with a synonym respell; a hunk that only *adds* render-required
/// parens (binding-table/re-lexability parens, e.g. `- - 96` -> `-(-96)`, nested
/// `JOIN` grouping) is a `MethodFalsePositive`; otherwise the genuine spelling
/// loss decides.
const KNOWN_SIGNATURES: &[(&str, SigClass)] = &[
    // --- Method false positives: parens the canonical render MUST add (the
    // binding table or re-lexability requires them; the source spelled the same
    // structure without them), plus pure alignment noise of those additions.
    ("-[] +[(]", SigClass::MethodFalsePositive),
    ("-[] +[)]", SigClass::MethodFalsePositive),
    ("-[] +[) )]", SigClass::MethodFalsePositive),
    // The newly-parsing `(X).* > (1, 1) IS NOT NULL` comparison-then-`IS NOT NULL` chain
    // (`is-predicate-precedence-below-comparison`): the render adds the grouping parens the
    // source omitted around the row comparison and its `(base).*` composite base.
    ("-[] +[( (]", SigClass::MethodFalsePositive),
    ("-[-] +[]", SigClass::MethodFalsePositive),
    ("-[] +[-]", SigClass::MethodFalsePositive),
    // --- Structural normalization: redundant source parens are DROPPED (the AST
    // records no parenthesization; a fidelity formatter cannot re-emit them).
    ("-[(] +[]", SigClass::StructuralNormalization),
    ("-[)] +[]", SigClass::StructuralNormalization),
    ("-[( (] +[]", SigClass::StructuralNormalization),
    ("-[) )] +[]", SigClass::StructuralNormalization),
    ("-[( ( (] +[]", SigClass::StructuralNormalization),
    ("-[) ) )] +[]", SigClass::StructuralNormalization),
    ("-[( ( ( (] +[]", SigClass::StructuralNormalization),
    ("-[) ) ) )] +[]", SigClass::StructuralNormalization),
    ("-[( ( ( ( (] +[]", SigClass::StructuralNormalization),
    ("-[) ) ) ) )] +[]", SigClass::StructuralNormalization),
    // Empty type-modifier list dropped: `DEC()` -> `DEC`.
    ("-[( )] +[]", SigClass::StructuralNormalization),
    // Redundant parens elided around a prefix operator whose operand is a tighter-binding
    // unary/cast: `~(-50::UHUGEINT)` -> `~-50::UHUGEINT` (DuckDB's general operator surface,
    // `duckdb-pg-operator-spelling-under-acceptance`). The `(` between `~` and `-` is dropped
    // (minimal-paren render); the closing `)` rides the sibling `-[)] +[]` hunk above.
    ("-[~ ( -] +[~-]", SigClass::StructuralNormalization),
    // A trailing clause our parser leaves unmodelled leaks as a SEPARATE `;`-joined
    // statement on render, materializing an interior `;` the source lacks. The sole
    // producer is PostgreSQL's `CREATE FUNCTION ... SET <guc> = <value>` attribute: the
    // function-scoped `SET` is not modelled, so it re-homes as a top-level `SET`.
    // (Embedded `CREATE SCHEMA` elements are NOT a producer — they parse as children of
    // the CreateSchema node and render embedded, one statement; see
    // render-create-schema-element-split.)
    ("-[] +[;]", SigClass::StructuralNormalization),
    // Trailing commas dropped (DuckDB trailing comma; SQLite dangling
    // `CONSTRAINT name`, the comma before it materialized on render).
    //
    // VERDICT (sqlite-spelling-fidelity-parser-fixes item 2): the SQLite table-level
    // trailing bare `CONSTRAINT <name>` (`UNIQUE (x) CONSTRAINT c`) is a *faithful*
    // normalization, not a semantic re-homing bug. SQLite's grammar separates table
    // constraints with an OPTIONAL comma (`tconscomma ::= COMMA | .`), so
    // `UNIQUE (x) CONSTRAINT c` genuinely IS two table-constraint entries — a `UNIQUE`
    // and a bodyless `CONSTRAINT c` — exactly as our parser models it (a `UNIQUE`
    // element plus an inert `TableConstraint::Bare`); the render always emits the comma
    // it may omit. The bodyless name is a SQLite no-op (`ccons ::= CONSTRAINT nm` sets a
    // pending name that names nothing when no body follows): sqlite 3.43.2 accepts BOTH
    // the elided-comma source and the comma-inserted render, and a bare `CONSTRAINT c`
    // creates NO index/constraint (only `UNIQUE`'s `sqlite_autoindex` appears in
    // `pragma_index_list`). Column-level dangling names (`x INT CHECK (…) CONSTRAINT one`)
    // stay on their column (`ColumnOption::Bare`) and round-trip verbatim — no re-homing
    // there at all. So attachment semantics are preserved on both surfaces; only the
    // elided comma is normalized, the same optional-token class as the rest of this block.
    ("-[,] +[]", SigClass::StructuralNormalization),
    ("-[] +[,]", SigClass::StructuralNormalization),
    // OFFSET-before-LIMIT re-ordered to canonical LIMIT..OFFSET.
    ("-[offset 1] +[]", SigClass::StructuralNormalization),
    ("-[] +[1 offset]", SigClass::StructuralNormalization),
    ("-[offset 990] +[]", SigClass::StructuralNormalization),
    ("-[] +[offset 990]", SigClass::StructuralNormalization),
    // DuckDB method-call syntax rewritten to a plain call: `x.f(a)` -> `f(x, a)`.
    (
        "-[. list_transform (] +[,]",
        SigClass::StructuralNormalization,
    ),
    ("-[] +[list_transform (]", SigClass::StructuralNormalization),
    // DuckDB `ORDER BY ALL` rewritten to `ORDER BY COLUMNS(*)`.
    ("-[all] +[columns ( * )]", SigClass::StructuralNormalization),
    ("-[all] +[columns ( *]", SigClass::StructuralNormalization),
    // --- Missing spelling tags: a synonym/optional-word spelling collapsing to
    // one canonical form because no AST tag records which one the source used.
    //
    // The optional `AS` before an alias has no entry here: the `AliasSpelling` tag on
    // every alias carrier (`SelectItem::Expr`, `TableAlias`, `DmlTarget`/`InsertTarget`,
    // `PivotExpr`, `UnpivotColumn`) makes a bare alias round-trip, so `-[] +[as]` is not
    // a lossy signature the sweep produces (spelling-tag-alias-as).
    // Optional (NATURAL) CROSS join noise word dropped. INNER/OUTER now round-trip via
    // the `inner`/`outer` bool tags on `JoinOperator` (spelling-tags-keyword-operator-batch);
    // `CROSS` is the optimizer-hint spelling of `INNER` (SQLite `NATURAL CROSS JOIN`
    // normalizes to a natural inner join), untagged and outside that batch.
    ("-[cross] +[]", SigClass::MissingSpellingTag),
    // SELECT INTO TABLE t -> SELECT INTO t (the `TABLE` keyword after SELECT INTO). The
    // TRUNCATE `-[] +[table]` counterpart now round-trips via `Statement::Truncate`'s
    // `table_keyword` tag (spelling-tags-keyword-operator-batch).
    ("-[table] +[]", SigClass::MissingSpellingTag),
    // WITH UNIQUE -> WITH UNIQUE KEYS; default WITHOUT UNIQUE KEYS elided;
    // WITH [UNCONDITIONAL [ARRAY]] WRAPPER noise words collapsed.
    ("-[] +[keys]", SigClass::MissingSpellingTag),
    ("-[without unique keys] +[]", SigClass::MissingSpellingTag),
    ("-[unconditional array] +[]", SigClass::MissingSpellingTag),
    ("-[unconditional] +[]", SigClass::MissingSpellingTag),
    ("-[array] +[]", SigClass::MissingSpellingTag),
    // UNPIVOT INTO NAME .. VALUES -> VALUE.
    ("-[values] +[value]", SigClass::MissingSpellingTag),
    // CREATE SEQUENCE INCREMENT 10 -> INCREMENT BY 10 (START 0 -> START WITH 0).
    ("-[] +[by]", SigClass::MissingSpellingTag),
    // COPY: optional WITH materialized for option lists, legacy `WITH .. AS`
    // noise words dropped for bare options.
    ("-[] +[with]", SigClass::MissingSpellingTag),
    ("-[with] +[]", SigClass::MissingSpellingTag),
    ("-[as] +[]", SigClass::MissingSpellingTag),
    // Column `WITH OPTIONS` dropped (CREATE TABLE .. PARTITION OF / OF type).
    ("-[with options] +[]", SigClass::MissingSpellingTag),
    // KEEP/OMIT QUOTES ON SCALAR STRING default scope elided.
    ("-[on scalar string] +[]", SigClass::MissingSpellingTag),
    // MERGE WHEN NOT MATCHED BY TARGET -> WHEN NOT MATCHED.
    ("-[by target] +[]", SigClass::MissingSpellingTag),
    // DuckDB's *table-factor* prefix alias `FROM b : a` respelled as the suffix
    // `FROM a AS b`. The select-item prefix form (`SELECT j: expr`) now round-trips via
    // `AliasSpelling::PrefixColon` (spelling-tag-alias-as); the table-factor form cannot
    // fold the same way — its correlation name lands in the factor's *trailing* alias
    // slot, which renders after the relation, so the colon position is unrepresentable
    // without a factor-render restructure. It canonicalizes to `AS` (round-trip-safe),
    // leaving these two paired hunks per statement.
    ("-[b :] +[]", SigClass::MissingSpellingTag),
    ("-[\"b\" :] +[]", SigClass::MissingSpellingTag),
    ("-[] +[as b]", SigClass::MissingSpellingTag),
    ("-[] +[as \"b\"]", SigClass::MissingSpellingTag),
];

fn classify(signature: &str) -> Option<SigClass> {
    KNOWN_SIGNATURES
        .iter()
        .find(|(s, _)| *s == signature)
        .map(|&(_, c)| c)
}

// ---------------------------------------------------------------------------
// Corpus sweep + pinned inventory
// ---------------------------------------------------------------------------

/// Per-corpus statement-level tallies, pinned by [`CORPUS_PINS`].
#[derive(Default, Debug, PartialEq, Eq)]
struct CorpusTally {
    /// Statements some listed dialect accepts (the measured subset).
    accepted: usize,
    /// Accepted statements whose token texts round-trip byte-identically.
    exact: usize,
    /// Accepted statements that differ only by ASCII case (keywordCase class).
    case_only: usize,
    /// Accepted statements with at least one spelling-lossy hunk.
    lossy: usize,
}

/// The pinned per-corpus inventory: (corpus, accepted, exact, case_only, lossy).
/// Regenerate with `REWRITE=1` (prints the replacement table); a drift without a
/// deliberate parser/render change is a spelling-fidelity regression.
const CORPUS_PINS: &[(&str, usize, usize, usize, usize)] = &[
    // +1 accepted / +1 lossy: `duckdb-geometry-type-and-overlaps-operator` made the
    // malformed-CRS `GEOMETRY('GEOGCRS[…')` query parse.
    // +1 accepted / +1 lossy: `duckdb-tranche2-small-gaps` added DuckDB `INSERT BY NAME`.
    // +1 accepted / +1 exact: `duckdb-pg-operator-spelling-under-acceptance` armed the general
    // symbolic operator surface, so one further generic-`Op` statement parses and round-trips
    // byte-identically.
    ("duckdb_curated", 1330, 918, 284, 128),
    // +9 accepted (the closed GEOMETRY + `&&` gap family): +3 exact, +6 lossy.
    // +1 accepted / +1 lossy: `duckdb-not-null-postfix-flip` turned on the two-word `NOT NULL`
    // postfix, so the `… list_sum(a::INT[]) % 2 == 0 NOT NULL` statement now parses; the postfix
    // itself round-trips exact (`NOT NULL` verbatim), but the statement carries an unrelated
    // `INT` -> `INTEGER` type-name canonicalization, so it lands lossy.
    // +1 accepted / +1 exact: `duckdb-drop-macro` made `DROP MACRO plus1` parse via the
    // `create_macro`-gated `DropObjectKind::Macro`; it round-trips byte-identically.
    // +1 accepted / +1 lossy: `duckdb-pg-operator-spelling-under-acceptance` armed the general
    // symbolic operator surface, so `SELECT ~(-50::UHUGEINT), -(-(50::UHUGEINT))` now parses;
    // its render elides the redundant parens (`~-50::UHUGEINT`), landing lossy.
    ("duckdb_testsuite", 5655, 3747, 1590, 318),
    // The `U&"…" [UESCAPE 'c']` identifier form carries a `QuoteStyle::UnicodeDouble`
    // spelling the canonical (`PreserveSource`) render replays verbatim, so statements whose
    // only lossy hunk was a decoded `U&"…"` -> `"…"` identifier round-trip exactly.
    ("pg_regress", 29519, 14719, 14126, 674),
    ("sqlglot_complex", 232, 137, 46, 49),
    ("sqlglot_identity", 514, 479, 2, 33),
    // +4 accepted / +4 exact: `parse-mysql-trigger-ddl` added the shared name-only
    // `DROP TRIGGER [IF EXISTS] <name>` object kind, gated by SQLite's `create_trigger` flag as
    // well as MySQL's `compound_statements`; the SQLite corpus's `DROP TRIGGER` statements now
    // parse and round-trip byte-identically.
    ("sqlite_testsuite", 2354, 2140, 158, 56),
    ("sqllogictest", 366, 305, 4, 57),
];

/// Everything one sweep aggregates, for the report + gates.
#[derive(Default)]
struct Sweep {
    tallies: BTreeMap<&'static str, CorpusTally>,
    /// signature -> (total, per-corpus counts, up to 3 exemplars).
    signatures: BTreeMap<String, SigEntry>,
    /// Case-only position tallies across all corpora (keyword vs ident vs other).
    case_keyword: usize,
    case_ident: usize,
    case_other: usize,
    /// Render outputs that failed to re-tokenize (must stay empty).
    unlexable: Vec<String>,
}

#[derive(Default)]
struct SigEntry {
    total: usize,
    per_corpus: BTreeMap<&'static str, usize>,
    exemplars: Vec<Exemplar>,
}

struct Exemplar {
    corpus: &'static str,
    dialect: BuiltinDialect,
    sql: String,
    rendered: String,
}

fn sweep_corpora() -> Sweep {
    let mut sweep = Sweep::default();
    for corpus in CORPORA {
        let tally = sweep.tallies.entry(corpus.name).or_default();
        for sql in (corpus.statements)() {
            let Some(result) = statement_fidelity(sql, corpus.dialects) else {
                continue;
            };
            tally.accepted += 1;
            let (dialect, rendered, fidelity) = match result {
                Ok(ok) => ok,
                Err(msg) => {
                    sweep
                        .unlexable
                        .push(format!("[{}] {sql:?}: {msg}", corpus.name));
                    continue;
                }
            };
            match fidelity {
                Fidelity::Exact => tally.exact += 1,
                Fidelity::CaseOnly {
                    keyword,
                    ident,
                    other,
                } => {
                    tally.case_only += 1;
                    sweep.case_keyword += keyword;
                    sweep.case_ident += ident;
                    sweep.case_other += other;
                }
                Fidelity::Lossy(hunks) => {
                    tally.lossy += 1;
                    for hunk in hunks {
                        let entry = sweep.signatures.entry(hunk.signature()).or_default();
                        entry.total += 1;
                        *entry.per_corpus.entry(corpus.name).or_default() += 1;
                        if entry.exemplars.len() < 3 {
                            entry.exemplars.push(Exemplar {
                                corpus: corpus.name,
                                dialect,
                                sql: sql.to_owned(),
                                rendered: rendered.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    sweep
}

/// Whether the run is in golden-rewrite mode (repo-wide `REWRITE=1` convention).
fn rewrite_mode() -> bool {
    env::var_os("REWRITE").is_some()
}

fn print_report(sweep: &Sweep) {
    eprintln!("spelling-fidelity inventory (statement verdicts per corpus):");
    eprintln!(
        "  {:<20} {:>9} {:>9} {:>10} {:>7}",
        "corpus", "accepted", "exact", "case_only", "lossy"
    );
    for (name, t) in &sweep.tallies {
        eprintln!(
            "  {:<20} {:>9} {:>9} {:>10} {:>7}",
            name, t.accepted, t.exact, t.case_only, t.lossy
        );
    }
    eprintln!(
        "case-only differing token positions: keyword {} / identifier {} / other {}",
        sweep.case_keyword, sweep.case_ident, sweep.case_other
    );

    // Per-class hunk counts per corpus: the spike's "counts per class per corpus".
    let mut per_class: BTreeMap<(String, &'static str), usize> = BTreeMap::new();
    for (sig, entry) in &sweep.signatures {
        let class = classify(sig)
            .map(|c| format!("{c:?}"))
            .unwrap_or_else(|| "UNCLASSIFIED".to_owned());
        for (&corpus, &n) in &entry.per_corpus {
            *per_class.entry((class.clone(), corpus)).or_default() += n;
        }
    }
    eprintln!("lossy hunks per class per corpus:");
    for ((class, corpus), n) in &per_class {
        eprintln!("  {class:<24} {corpus:<20} {n}");
    }

    // Signatures, most frequent first, with their triage class and exemplars.
    let mut by_count: Vec<(&String, &SigEntry)> = sweep.signatures.iter().collect();
    by_count.sort_by(|a, b| b.1.total.cmp(&a.1.total).then_with(|| a.0.cmp(b.0)));
    eprintln!("lossy hunk signatures ({}):", by_count.len());
    for (sig, entry) in &by_count {
        let class = classify(sig)
            .map(|c| format!("{c:?}"))
            .unwrap_or_else(|| "UNCLASSIFIED".to_owned());
        let per_corpus = entry
            .per_corpus
            .iter()
            .map(|(c, n)| format!("{c}:{n}"))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("  {:>6}x [{class}] {sig}   ({per_corpus})", entry.total);
        for ex in &entry.exemplars {
            eprintln!(
                "         e.g. [{}/{:?}] {:?} -> {:?}",
                ex.corpus, ex.dialect, ex.sql, ex.rendered
            );
        }
    }
}

fn print_rewrite_blocks(sweep: &Sweep) {
    let mut pins = String::new();
    for (name, t) in &sweep.tallies {
        writeln!(
            pins,
            "    (\"{name}\", {}, {}, {}, {}),",
            t.accepted, t.exact, t.case_only, t.lossy
        )
        .expect("writing to String cannot fail");
    }
    eprintln!("REWRITE: CORPUS_PINS should be:\n{pins}");

    let unknown: Vec<&String> = sweep
        .signatures
        .keys()
        .filter(|s| classify(s).is_none())
        .collect();
    eprintln!(
        "REWRITE: {} signatures lack a KNOWN_SIGNATURES entry (triage-label each):",
        unknown.len()
    );
    for sig in unknown {
        eprintln!("    (\"{}\", SigClass::???),", sig.replace('"', "\\\""));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ratchet gate: the canonical render never emits un-lexable output, every
    /// lossy signature is triaged in [`KNOWN_SIGNATURES`], and the per-corpus
    /// statement tallies match [`CORPUS_PINS`] exactly. `REWRITE=1` prints the
    /// replacement blocks instead of asserting.
    #[test]
    fn corpus_spelling_fidelity_is_pinned_and_triaged() {
        let sweep = sweep_corpora();
        print_report(&sweep);

        assert!(
            sweep.unlexable.is_empty(),
            "canonical render produced un-lexable output (render bug):\n{}",
            sweep.unlexable.join("\n")
        );

        if rewrite_mode() {
            print_rewrite_blocks(&sweep);
            return;
        }

        let untriaged: Vec<&String> = sweep
            .signatures
            .keys()
            .filter(|s| classify(s).is_none())
            .collect();
        assert!(
            untriaged.is_empty(),
            "new spelling-lossy signatures need triage into KNOWN_SIGNATURES \
             (run with REWRITE=1 for the block): {untriaged:?}"
        );

        let measured: Vec<(&str, usize, usize, usize, usize)> = sweep
            .tallies
            .iter()
            .map(|(name, t)| (*name, t.accepted, t.exact, t.case_only, t.lossy))
            .collect();
        assert_eq!(
            measured, CORPUS_PINS,
            "spelling-fidelity inventory drifted; if intentional, regenerate CORPUS_PINS \
             with REWRITE=1"
        );
    }

    // -----------------------------------------------------------------------
    // Authored synonym-pair probes (constructs the corpora may not exercise)
    // -----------------------------------------------------------------------

    /// A probe's pinned verdict. `Rejected` documents that the pair cannot collapse
    /// because the spelling is outside the parse surface; `Lossy` pins the exact
    /// hunk signatures the collapse produces today, so a landed spelling tag flips
    /// the probe loudly (update it to `Exact` and remove the ledger entry).
    #[derive(Debug)]
    enum Expect {
        Rejected,
        Exact,
        CaseOnly,
        Lossy(&'static [&'static str]),
    }

    struct SpellingProbe {
        /// The synonym family the probe exercises (groups the report).
        family: &'static str,
        dialect: BuiltinDialect,
        sql: &'static str,
        expect: Expect,
    }

    const PROBES: &[SpellingProbe] = &[
        // Keyword-case control: lowercase keywords are the CaseOnly class (the
        // keywordCase knob's territory), never a lossy hunk.
        SpellingProbe {
            family: "keyword-case",
            dialect: BuiltinDialect::Ansi,
            sql: "select a from t where a = 1",
            expect: Expect::CaseOnly,
        },
        // TEMP vs TEMPORARY.
        SpellingProbe {
            family: "temp/temporary",
            dialect: BuiltinDialect::Postgres,
            sql: "CREATE TEMP TABLE t (x INT)",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "temp/temporary",
            dialect: BuiltinDialect::Postgres,
            sql: "CREATE TEMPORARY TABLE t (x INT)",
            expect: Expect::Exact,
        },
        // Optional AS before column/table aliases.
        SpellingProbe {
            family: "optional-as",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT a AS b FROM t",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-as",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT a b FROM t",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-as",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT * FROM t AS u",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-as",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT * FROM t u",
            expect: Expect::Exact,
        },
        // The bare alias round-trips on the DML target (`DELETE FROM t x`) and inside a
        // DuckDB PIVOT aggregate list (`sum(x) total`) too.
        SpellingProbe {
            family: "optional-as",
            dialect: BuiltinDialect::Postgres,
            sql: "DELETE FROM t x WHERE x.a > 1",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-as",
            dialect: BuiltinDialect::Postgres,
            sql: "DELETE FROM t AS x WHERE x.a > 1",
            expect: Expect::Exact,
        },
        // DuckDB prefix-colon alias on a select item folds onto `AliasSpelling::PrefixColon`
        // and round-trips; the table-factor prefix form still canonicalizes to `AS`.
        SpellingProbe {
            family: "prefix-colon",
            dialect: BuiltinDialect::DuckDb,
            sql: "SELECT j: a FROM t",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "prefix-colon",
            dialect: BuiltinDialect::DuckDb,
            sql: "SELECT * FROM b : a",
            expect: Expect::Lossy(&["-[b :] +[]", "-[] +[as b]"]),
        },
        // Optional INNER / OUTER join noise words.
        SpellingProbe {
            family: "optional-inner-outer",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT 1 FROM a JOIN b ON a.x = b.x",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-inner-outer",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT 1 FROM a INNER JOIN b ON a.x = b.x",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-inner-outer",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT 1 FROM a LEFT JOIN b ON a.x = b.x",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-inner-outer",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT 1 FROM a LEFT OUTER JOIN b ON a.x = b.x",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-inner-outer",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT 1 FROM a FULL OUTER JOIN b ON a.x = b.x",
            expect: Expect::Exact,
        },
        // MySQL UNIQUE KEY vs UNIQUE INDEX table-constraint spellings: outside the
        // current MySql parse surface, so the pair cannot collapse (yet). When the
        // grammar lands, these flip and force a fidelity verdict here.
        SpellingProbe {
            family: "unique-key/index",
            dialect: BuiltinDialect::MySql,
            sql: "CREATE TABLE t (x INT, UNIQUE KEY k (x))",
            expect: Expect::Rejected,
        },
        SpellingProbe {
            family: "unique-key/index",
            dialect: BuiltinDialect::MySql,
            sql: "CREATE TABLE t (x INT, UNIQUE INDEX k (x))",
            expect: Expect::Rejected,
        },
        SpellingProbe {
            family: "unique-key/index",
            dialect: BuiltinDialect::MySql,
            sql: "CREATE TABLE t (x INT, UNIQUE k (x))",
            expect: Expect::Rejected,
        },
        // BEGIN [TRANSACTION|WORK] vs START TRANSACTION.
        SpellingProbe {
            family: "begin/start",
            dialect: BuiltinDialect::Postgres,
            sql: "BEGIN",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "begin/start",
            dialect: BuiltinDialect::Postgres,
            sql: "BEGIN TRANSACTION",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "begin/start",
            dialect: BuiltinDialect::Postgres,
            sql: "BEGIN WORK",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "begin/start",
            dialect: BuiltinDialect::Postgres,
            sql: "START TRANSACTION",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "begin/start",
            dialect: BuiltinDialect::Postgres,
            sql: "COMMIT WORK",
            expect: Expect::Exact,
        },
        // Optional COLUMN in ALTER TABLE.
        SpellingProbe {
            family: "optional-column",
            dialect: BuiltinDialect::Postgres,
            sql: "ALTER TABLE t ADD COLUMN c INT",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-column",
            dialect: BuiltinDialect::Postgres,
            sql: "ALTER TABLE t ADD c INT",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-column",
            dialect: BuiltinDialect::Postgres,
            sql: "ALTER TABLE t DROP COLUMN c",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "optional-column",
            dialect: BuiltinDialect::Postgres,
            sql: "ALTER TABLE t DROP c",
            expect: Expect::Exact,
        },
        // `<>` vs `!=`.
        SpellingProbe {
            family: "noteq",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT a <> b FROM t",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "noteq",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT a != b FROM t",
            expect: Expect::Exact,
        },
        // String concatenation spellings (`||` operator vs CONCAT call — distinct
        // AST shapes, so both should hold their spelling).
        SpellingProbe {
            family: "concat",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT 'a' || 'b'",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "concat",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT CONCAT('a', 'b')",
            expect: Expect::Exact,
        },
        // SQLite `==` (EqualsSpelling tag) and bare `IS` (IS NOT DISTINCT FROM).
        SpellingProbe {
            family: "sqlite-equality",
            dialect: BuiltinDialect::Sqlite,
            sql: "SELECT 1 == 1",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "sqlite-equality",
            dialect: BuiltinDialect::Sqlite,
            sql: "SELECT 1 = 1",
            expect: Expect::Exact,
        },
        // SQLite's bare `IS`/`IS NOT` round-trip verbatim (the
        // `IsNotDistinctFromSpelling::Is` / `IsDistinctFromSpelling::Is` tags carry the
        // surface form); the explicit keyword forms, also valid under SQLite, stay Exact too.
        SpellingProbe {
            family: "sqlite-is",
            dialect: BuiltinDialect::Sqlite,
            sql: "SELECT 1 IS 2",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "sqlite-is",
            dialect: BuiltinDialect::Sqlite,
            sql: "SELECT 1 IS NOT 2",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "sqlite-is",
            dialect: BuiltinDialect::Sqlite,
            sql: "SELECT 1 IS DISTINCT FROM 2",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "sqlite-is",
            dialect: BuiltinDialect::Sqlite,
            sql: "SELECT 1 IS NOT DISTINCT FROM 2",
            expect: Expect::Exact,
        },
        // TypeName spelling-tag doctrine controls (must stay Exact).
        SpellingProbe {
            family: "typename-control",
            dialect: BuiltinDialect::Ansi,
            sql: "CREATE TABLE t (x INT)",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "typename-control",
            dialect: BuiltinDialect::Ansi,
            sql: "CREATE TABLE t (x INTEGER)",
            expect: Expect::Exact,
        },
        // TRUNCATE [TABLE].
        SpellingProbe {
            family: "truncate-table",
            dialect: BuiltinDialect::Postgres,
            sql: "TRUNCATE TABLE t",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "truncate-table",
            dialect: BuiltinDialect::Postgres,
            sql: "TRUNCATE t",
            expect: Expect::Exact,
        },
        // SET assignment spelling.
        SpellingProbe {
            family: "set-to",
            dialect: BuiltinDialect::Postgres,
            sql: "SET x TO 1",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "set-to",
            dialect: BuiltinDialect::Postgres,
            sql: "SET x = 1",
            expect: Expect::Exact,
        },
        // Redundant expression parens (the structural-normalization control).
        SpellingProbe {
            family: "redundant-parens",
            dialect: BuiltinDialect::Ansi,
            sql: "SELECT (1 + 2)",
            expect: Expect::Lossy(&["-[(] +[]", "-[)] +[]"]),
        },
        // UNPIVOT null-marker fidelity (`Unpivot::null_inclusion`): a written
        // `INCLUDE NULLS` / `EXCLUDE NULLS` round-trips, and the unwritten default
        // stays bare — the explicit `EXCLUDE NULLS` (semantically the default) is the
        // pair that used to elide.
        SpellingProbe {
            family: "unpivot-nulls",
            dialect: BuiltinDialect::DuckDb,
            sql: "SELECT * FROM t UNPIVOT (v FOR n IN (a, b))",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "unpivot-nulls",
            dialect: BuiltinDialect::DuckDb,
            sql: "SELECT * FROM t UNPIVOT INCLUDE NULLS (v FOR n IN (a, b))",
            expect: Expect::Exact,
        },
        SpellingProbe {
            family: "unpivot-nulls",
            dialect: BuiltinDialect::DuckDb,
            sql: "SELECT * FROM t UNPIVOT EXCLUDE NULLS (v FOR n IN (a, b))",
            expect: Expect::Exact,
        },
    ];

    #[test]
    fn synonym_pair_probes_are_pinned() {
        let mut failures = Vec::new();
        for probe in PROBES {
            let verdict = statement_fidelity(probe.sql, &[probe.dialect]);
            let (label, sigs): (&str, Vec<String>) = match &verdict {
                None => ("Rejected", Vec::new()),
                Some(Err(msg)) => panic!(
                    "probe {:?} render does not re-tokenize (render bug): {msg}",
                    probe.sql
                ),
                Some(Ok((_, _, Fidelity::Exact))) => ("Exact", Vec::new()),
                Some(Ok((_, _, Fidelity::CaseOnly { .. }))) => ("CaseOnly", Vec::new()),
                Some(Ok((_, _, Fidelity::Lossy(hunks)))) => {
                    ("Lossy", hunks.iter().map(Hunk::signature).collect())
                }
            };
            let rendered = match &verdict {
                Some(Ok((_, r, _))) => r.clone(),
                _ => String::new(),
            };
            eprintln!(
                "probe [{}] {:?} ({:?}): {label} {sigs:?} -> {rendered:?}",
                probe.family, probe.sql, probe.dialect
            );
            let ok = match (&probe.expect, label) {
                (Expect::Rejected, "Rejected")
                | (Expect::Exact, "Exact")
                | (Expect::CaseOnly, "CaseOnly") => true,
                (Expect::Lossy(expected), "Lossy") => {
                    sigs.iter().map(String::as_str).collect::<Vec<_>>() == **expected
                }
                _ => false,
            };
            if !ok {
                failures.push(format!(
                    "probe [{}] {:?}: expected {:?}, measured {label} {sigs:?}",
                    probe.family, probe.sql, probe.expect
                ));
            }
        }
        if rewrite_mode() {
            return;
        }
        assert!(
            failures.is_empty(),
            "probe verdicts drifted (a spelling tag landed or regressed — update the pins):\n{}",
            failures.join("\n")
        );
    }
}
