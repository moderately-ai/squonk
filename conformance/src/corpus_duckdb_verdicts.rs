// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! DuckDB accept/reject **parity gate** over the vendored signature-surface corpus,
//! behind a per-file setup driver (`duckdb-corpus-oracle-at-scale`; phase 0 was
//! `duckdb-dialect-100-percent-programme`).
//!
//! Phase 0 was an *assessment* sweep: it counted an inventory but asserted nothing
//! about divergences, and it ran over a *bare* database, which left two measurement
//! holes. This module promotes it to the allowlist-gated accept/reject parity test
//! that is deliverable (b) of the 100% definition, and adds the setup driver that
//! closes the two holes.
//!
//! # The two holes the bare-DB assessment left, and how the setup driver closes them
//!
//! [`DuckDbOracle`](crate::m2::DuckDbOracle) is
//! [`PrepareBind`](crate::oracle::OracleSemantics::PrepareBind): `prepare` parses *and*
//! binds, so an unknown table/column rejects. The vendored test SQL is schema-heavy
//! (`FROM integers`), so over a *bare* in-memory database DuckDB rejects most
//! statements for a *binding* reason our parse-only parser never sees. Phase 0
//! measured two blind quadrants over the bare DB: `over-accept-binding` (we accept,
//! DuckDB binding-rejects — pure noise, not a parser bug) and the binding-masked share
//! of `agree-reject` (DuckDB might accept with a schema, but we cannot tell).
//!
//! The **setup driver** provisions each statement's *own file's* schema before any
//! `prepare`. DuckDB's `.test` files each carry their `statement ok CREATE TABLE …`
//! records right beside the queries; `extract.py` re-emits the corpus as
//! `(setup_ddl[], query)` groups per source file
//! ([`statements_with_schema.sql`](DUCKDB_WITH_SCHEMA)). For each group the file's
//! concrete `CREATE` DDL is `execute_batch`ed onto a fresh connection via the
//! [`with_schema`](crate::m2::DuckDbOracle::with_schema) seam — the DDL is the *only*
//! thing executed; queries stay `prepare`-only, so the never-execute guarantee holds —
//! and the query verdict is read against that provisioned schema. Because the schema is
//! the query's real schema (not a synthetic guess), a statement whose names resolve is
//! **comparable**: DuckDB either accepts it (a real coverage gap where we reject) or
//! rejects it for *syntax* (a reason a schema cannot mask).
//!
//! DuckDB's accept verdict is taken as `bare-accepts OR schema-accepts`. Bare-accept
//! captures the schema-independent statements (`SELECT [1,2,3]`) and neutralizes the
//! one self-collision the per-file DDL would otherwise cause — a `CREATE TABLE t …`
//! that is *both* a statement-under-test and part of its file's setup would else
//! catalog-reject "table t already exists" against its own provisioned schema; bare
//! (empty DB) accepts it, which is the honest verdict. Schema-accept captures the
//! schema-dependent statements the file's DDL unblinds.
//!
//! # What the gate asserts
//!
//! - **Over-acceptance is the ledgered class.** *We accept and DuckDB rejects for
//!   syntax* is a real validator-correctness divergence: a syntax reject is
//!   schema-independent (bare and provisioned agree), so no schema can have caused or
//!   hidden it. Every such case must be fixed or named in
//!   [`DUCKDB_DIVERGENCE_ALLOWLIST`] with an existing ticket — the exact-SQL PG-ledger
//!   pattern ([`pg::PG_DIVERGENCE_ALLOWLIST`](crate::pg::PG_DIVERGENCE_ALLOWLIST) /
//!   [`m2::M2_DIVERGENCE_ALLOWLIST`](crate::m2::M2_DIVERGENCE_ALLOWLIST)), staleness
//!   enforced. Phase 0 found **zero**; the per-file driver keeps it honest at scale.
//! - **Coverage gaps route to the grammar-family children.** *DuckDB accepts, we
//!   reject* is the expected bulk class for a dialect whose grammar families are not yet
//!   modelled. We now parse under the fitted [`CORPUS_DIALECT`] preset
//!   (`duckdb-featureset-preset`), so the gap surface is the true dialect delta — the
//!   DuckDB-specific grammar the child tickets own — not the PostgreSQL-shared surface a
//!   strict-`Ansi` pass also flagged. Individually allowlisting hundreds of gaps by
//!   exact SQL is neither tractable nor useful; each gap is attributed to a signature
//!   family (`signature_families`) whose child ticket *is* its allowlist entry
//!   ([`GAP_FAMILIES`]), and the per-family counts are pinned (deliverable (e)) so a
//!   closed gap or a fresh regression drifts a pin and fails loudly.
//! - **The residual is counted, not silenced (the STOP fallback).** The corpus name
//!   space is unbounded — a query can reference a table created by templated DDL
//!   (`CREATE TABLE t (i {type})`), by another file, or by an unloaded extension, which
//!   the per-file concrete DDL cannot provision. Such a statement stays a *binding
//!   residual*: counted and pinned, never gated (a binding reject is never ledgered).
//!   This is the documented provisioned-subset bound, not a green-forcing silence.
//!
//! [`CORPUS_DIALECT`] is the fitted `DuckDb` preset, so the residual coverage gaps are the
//! true dialect delta. The `json_serialize_sql` sampling is kept to keep sizing the
//! structural-oracle child (`duckdb-structural-oracle-select`).

use crate::verdict_harness::{DivergenceEntry, assert_entries_are_ticketed, ticket_exists};
use squonk::dialect::DuckDb;
use squonk::parse_with;

const DUCKDB_STATEMENTS: &str = include_str!("../corpus/duckdb/statements.sql");
const DUCKDB_DOCS_EXAMPLES: &str = include_str!("../corpus/duckdb/docs_examples.sql");
/// The grouped setup-driver artifact: the same selected statements as
/// [`DUCKDB_STATEMENTS`], regrouped under their source file with that file's concrete
/// `CREATE` setup DDL (`# file:` / `# setup` / `# query` section markers). Emitted by
/// `extract.py`; a coherence test asserts its query set equals `statements.sql`.
const DUCKDB_WITH_SCHEMA: &str = include_str!("../corpus/duckdb/statements_with_schema.sql");

/// Pinned statement counts — a line vanishing from a fixture trips these
/// (anti-vanishing, mirroring the sibling corpus loaders).
const STATEMENTS_PINNED: usize = 1350;
const DOCS_PINNED: usize = 31;
/// Grouped-artifact anti-vanishing pins: source-file groups and total concrete setup
/// DDL statements. Re-pinned whenever `extract.py` is re-run against a new upstream
/// reference (its query set is pinned against `statements.sql` by the coherence test).
const GROUPED_FILES_PINNED: usize = 298;
const GROUPED_SETUP_DDL_PINNED: usize = 643;

// --- The core-tranche spec-audit corpus (spec-audit-duckdb-test-suite-corpus) --------
//
// A SECOND, independent DuckDB corpus, vendored under `corpus/duckdb-testsuite/` and
// pinned to the exact upstream tag our oracle links (v1.5.4 /
// 08e34c447bae34eaee3723cac61f2878b6bdf787). Where `corpus/duckdb/` above is
// *signature-weighted* (biased toward DuckDB's distinctive grammar and now fully
// closed), this one is a *broad* slice of nine core `test/sql` directories
// (select/join/subquery/aggregate/window/cte/order/limit/types) — the executable spec
// the signature weighting skipped — extracted to MEASURE the true residual grammar-gap
// inventory. It is a pure measurement surface: its sweep PINS the quadrant counts and
// PRINTS the family/over-accept inventory, but files no tickets and gates nothing to
// zero (the ranked inventory drives separate fix tickets). See
// `corpus/duckdb-testsuite/README.md` + `extract_core.py`.
//
// Three artifacts, one `extract_core.py` run:
// - `statements.sql`  — flat accepts (`statement ok` + `query` bodies), one per line.
// - `rejects.sql`     — flat rejects (`statement error` bodies) — the over-acceptance
//   differential's food (statements DuckDB is known to reject).
// - `statements_with_schema.sql` — the same queries AND rejects regrouped under their
//   source `.test` file with that file's concrete `CREATE` setup DDL, driving the same
//   per-file setup driver as `schema_groups()` (`# file:`/`# setup`/`# query`/`# reject`).
const CORE_STATEMENTS: &str = include_str!("../corpus/duckdb-testsuite/statements.sql");
const CORE_REJECTS: &str = include_str!("../corpus/duckdb-testsuite/rejects.sql");
const CORE_WITH_SCHEMA: &str =
    include_str!("../corpus/duckdb-testsuite/statements_with_schema.sql");

/// Core-tranche anti-vanishing count pins (a line vanishing trips these). Measured off
/// the v1.5.4 extraction at per-file caps 12 (accepts) / 8 (rejects).
const CORE_STATEMENTS_PINNED: usize = 5668;
const CORE_REJECTS_PINNED: usize = 1119;
const CORE_GROUPED_FILES_PINNED: usize = 801;
const CORE_GROUPED_SETUP_DDL_PINNED: usize = 1194;

// --- The tranche-2 spec-audit corpus (spec-audit-duckdb-remaining-tranches) -----------
//
// A THIRD, independent DuckDB corpus, vendored under `corpus/duckdb-testsuite-tranche2/`
// and pinned to the same upstream tag our oracle links (v1.5.4 /
// 08e34c447bae34eaee3723cac61f2878b6bdf787). Tranche 1 (`corpus/duckdb-testsuite/`
// above) covered nine core `test/sql` directories; this tranche covers the nineteen
// remaining directories the ticket names — insert/update/delete/merge (DML),
// create/alter/index/constraints (DDL), copy/storage/attach, function, pivot, prepared,
// optimizer, pragma/settings, tpch/tpcds — the rest of the executable spec. It is a pure
// measurement surface with the same shape as tranche 1: its sweep PINS the two quadrant
// tuples and PRINTS the family/over-accept inventory, files no tickets, and gates
// nothing to zero (the ranked inventory drives separate fix tickets). Its pins are
// a SEPARATE set of `TRANCHE2_*` consts that never touch the tranche-1 `CORE_*` pins.
// See `corpus/duckdb-testsuite-tranche2/README.md` + `extract_tranche2.py`.
const TRANCHE2_STATEMENTS: &str =
    include_str!("../corpus/duckdb-testsuite-tranche2/statements.sql");
const TRANCHE2_REJECTS: &str = include_str!("../corpus/duckdb-testsuite-tranche2/rejects.sql");
const TRANCHE2_WITH_SCHEMA: &str =
    include_str!("../corpus/duckdb-testsuite-tranche2/statements_with_schema.sql");

/// Tranche-2 anti-vanishing count pins (a line vanishing trips these). Measured off the
/// v1.5.4 extraction at per-file caps 12 (accepts) / 8 (rejects) — the same caps as
/// tranche 1.
const TRANCHE2_STATEMENTS_PINNED: usize = 7634;
const TRANCHE2_REJECTS_PINNED: usize = 1319;
const TRANCHE2_GROUPED_FILES_PINNED: usize = 1429;
const TRANCHE2_GROUPED_SETUP_DDL_PINNED: usize = 2060;

/// The dialect the DuckDB corpus is parsed under for the differential.
///
/// The fitted [`DuckDb`] preset (PostgreSQL-derived), so the residual coverage gaps are the
/// true dialect delta — the grammar-family surface the child tickets own. Every other
/// reference in this module goes through this const, so the dialect is fixed in exactly one
/// place.
const CORPUS_DIALECT: DuckDb = DuckDb;

// --- DuckDB top-level `stmt` production inventory (spec-coverage-duckdb-production-inventory) ---
//
// The reproducible production DENOMINATOR: the 43 direct alternatives of DuckDB's
// top-level `stmt` grammar production, extracted from the pinned `statements.list`
// manifest (`scripts/generate_grammar.py` materializes the bison `stmt:` rule verbatim
// from it) at the EXACT v1.5.4 / 08e34c447b commit our in-process libduckdb oracle
// links. See `corpus/duckdb-grammar-inventory/`. This is independent negative space — it
// names an upstream production even when no vendored corpus statement reaches it — kept
// DISTINCT from squonk acceptance and from structural parity (separate pins below).

/// The pinned DuckDB `stmt` production denominator, one sorted production per line.
const DUCKDB_STMT_PRODUCTIONS: &str =
    include_str!("../corpus/duckdb-grammar-inventory/stmt-productions.txt");

/// The denominator as a set. Independent of any corpus, unlike a corpus-family inventory.
fn duckdb_stmt_productions() -> std::collections::BTreeSet<&'static str> {
    DUCKDB_STMT_PRODUCTIONS
        .lines()
        .filter(|line| !line.is_empty())
        .collect()
}

/// The depth-0 uppercased word tokens of `sql`, in order — the tokens outside every
/// parenthesis and outside string/identifier quoting. Parenthesis depth is tracked so a
/// subquery's verbs stay hidden (a CTE body, an `IN (SELECT …)`); single- and
/// double-quoted spans are skipped so their contents never leak a keyword. A word token
/// is a maximal run of ASCII alphanumerics/underscore, uppercased.
fn depth0_word_tokens(sql: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth: i32 = 0;
    let mut in_single = false;
    let mut in_double = false;
    for ch in sql.chars() {
        if in_single {
            in_single = ch != '\'';
            continue;
        }
        if in_double {
            in_double = ch != '"';
            continue;
        }
        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '(' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                depth += 1;
            }
            ')' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                depth = depth.saturating_sub(1);
            }
            c if depth == 0 && (c.is_ascii_alphanumeric() || c == '_') => {
                current.push(c.to_ascii_uppercase());
            }
            _ => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Classify one complete, engine-accepted statement to the DuckDB top-level `stmt`
/// production it reduces through. DuckDB's `stmt:` dispatch is keyword-deterministic on
/// the leading token(s) — each of the 43 alternatives opens with a distinct keyword
/// signature — so the leading-keyword form of an accepted statement identifies its
/// production without a raw parse tree (libduckdb's C API exposes none). The arms mirror
/// the vendored grammar fragments (`third_party/libpg_query/grammar/statements/*.y` at
/// the pinned commit); the ones that need more than the first keyword are noted inline.
/// `None` is a form outside the top-level grammar (a bare fragment), counted as unmapped.
fn duckdb_stmt_production(sql: &str) -> Option<&'static str> {
    // A leading `(` is only reachable through `select_with_parens` (incl. the
    // `'(' VariableShowStmt ')'` arm); no other `stmt` alternative opens with one.
    if sql.trim_start().starts_with('(') {
        return Some("SelectStmt");
    }
    let tokens = depth0_word_tokens(sql);
    let first = tokens.first()?.as_str();

    // WITH-prefixed DML: SELECT/INSERT/UPDATE/DELETE/MERGE all accept a leading
    // `opt_with_clause`, so skip the CTE list (its bodies sit at depth ≥ 1, hidden by the
    // tokenizer) to the first depth-0 statement verb.
    const VERBS: &[&str] = &[
        "SELECT",
        "FROM",
        "VALUES",
        "VALUE",
        "TABLE",
        "PIVOT",
        "PIVOT_WIDER",
        "UNPIVOT",
        "PIVOT_LONGER",
        "INSERT",
        "UPDATE",
        "DELETE",
        "TRUNCATE",
        "MERGE",
    ];
    let verb_idx = if first == "WITH" {
        tokens.iter().position(|w| VERBS.contains(&w.as_str()))?
    } else {
        0
    };
    let verb = tokens[verb_idx].as_str();
    let next = tokens.get(verb_idx + 1).map(String::as_str);

    match verb {
        // SelectStmt: SELECT / FROM-first / VALUES / TABLE shorthand / PIVOT / UNPIVOT.
        "SELECT" | "FROM" | "VALUES" | "VALUE" | "TABLE" | "PIVOT" | "PIVOT_WIDER" | "UNPIVOT"
        | "PIVOT_LONGER" => Some("SelectStmt"),
        "INSERT" => Some("InsertStmt"),
        // `opt_with_clause UPDATE EXTENSIONS …` vs `opt_with_clause UPDATE <rel> …`.
        "UPDATE" => Some(if next == Some("EXTENSIONS") {
            "UpdateExtensionsStmt"
        } else {
            "UpdateStmt"
        }),
        // delete.y folds TRUNCATE into DeleteStmt.
        "DELETE" | "TRUNCATE" => Some("DeleteStmt"),
        "MERGE" => Some("MergeIntoStmt"),

        "CREATE" => duckdb_create_production(&tokens),
        "ALTER" => duckdb_alter_production(&tokens),
        // drop_secret.y is the only DROP with its own `stmt` alternative.
        "DROP" => Some(if next == Some("SECRET") {
            "DropSecretStmt"
        } else {
            "DropStmt"
        }),
        "COPY" => Some("CopyStmt"),
        "ATTACH" => Some("AttachStmt"),
        "DETACH" => Some("DetachStmt"),
        "CALL" => Some("CallStmt"),
        "CHECKPOINT" => Some("CheckPointStmt"),
        // checkpoint.y `FORCE CHECKPOINT` vs load.y `FORCE INSTALL`.
        "FORCE" => match next {
            Some("CHECKPOINT") => Some("CheckPointStmt"),
            Some("INSTALL") => Some("LoadStmt"),
            _ => None,
        },
        "COMMENT" => Some("CommentOnStmt"),
        "ANALYZE" => Some("AnalyzeStmt"),
        "VACUUM" => Some("VacuumStmt"),
        // load.y carries both LOAD and INSTALL.
        "LOAD" | "INSTALL" => Some("LoadStmt"),
        "PRAGMA" => Some("PragmaStmt"),
        "PREPARE" => Some("PrepareStmt"),
        "EXECUTE" => Some("ExecuteStmt"),
        "DEALLOCATE" => Some("DeallocateStmt"),
        "EXPLAIN" => Some("ExplainStmt"),
        "EXPORT" => Some("ExportStmt"),
        "IMPORT" => Some("ImportStmt"),
        "SET" => Some("VariableSetStmt"),
        "RESET" => Some("VariableResetStmt"),
        // variable_show.y `show_or_describe` = SHOW | DESCRIBE | DESC; SUMMARIZE joins it.
        "SHOW" | "DESCRIBE" | "DESC" | "SUMMARIZE" => Some("VariableShowStmt"),
        "USE" => Some("UseStmt"),
        "BEGIN" | "START" | "COMMIT" | "END" | "ROLLBACK" | "ABORT" => Some("TransactionStmt"),
        _ => None,
    }
}

/// CREATE-family sub-dispatch. `CREATE [OR REPLACE] [TEMP|…] <object> …`; the object
/// keyword selects the production, and only `TABLE` splits further (a depth-0 `AS` is
/// CTAS → CreateAsStmt, otherwise the column-list form → CreateStmt).
fn duckdb_create_production(tokens: &[String]) -> Option<&'static str> {
    // Qualifiers between CREATE and the object keyword; RECURSIVE only precedes VIEW and
    // UNIQUE only precedes INDEX, so skipping them lands on the object keyword either way.
    const QUALIFIERS: &[&str] = &[
        "OR",
        "REPLACE",
        "TEMP",
        "TEMPORARY",
        "PERSISTENT",
        "LOCAL",
        "GLOBAL",
        "TRANSIENT",
        "UNLOGGED",
        "RECURSIVE",
    ];
    let mut idx = 1; // past CREATE
    while tokens
        .get(idx)
        .map(String::as_str)
        .is_some_and(|t| QUALIFIERS.contains(&t))
    {
        idx += 1;
    }
    if tokens.get(idx).map(String::as_str) == Some("UNIQUE") {
        idx += 1; // CREATE UNIQUE INDEX
    }
    match tokens.get(idx).map(String::as_str)? {
        // A depth-0 `AS` is CTAS; a generated-column `… AS (expr)` sits at depth ≥ 1.
        "TABLE" => Some(if tokens.iter().any(|t| t == "AS") {
            "CreateAsStmt"
        } else {
            "CreateStmt"
        }),
        "VIEW" => Some("ViewStmt"),
        "SEQUENCE" => Some("CreateSeqStmt"),
        "TYPE" => Some("CreateTypeStmt"),
        "SCHEMA" => Some("CreateSchemaStmt"),
        "SECRET" => Some("CreateSecretStmt"),
        "MACRO" | "FUNCTION" => Some("CreateFunctionStmt"),
        "INDEX" => Some("IndexStmt"),
        _ => None,
    }
}

/// ALTER-family sub-dispatch. All open with ALTER; the object and the operation split
/// them: `ALTER DATABASE …` → AlterDatabaseStmt; a depth-0 RENAME → RenameStmt (rename.y
/// covers ALTER SCHEMA/TABLE/VIEW/SEQUENCE/INDEX … RENAME); an adjacent `SET SCHEMA` →
/// AlterObjectSchemaStmt; a bare `ALTER SEQUENCE …` (sequence options) → AlterSeqStmt;
/// everything else (ALTER TABLE/INDEX/VIEW … cmds) → AlterTableStmt.
fn duckdb_alter_production(tokens: &[String]) -> Option<&'static str> {
    let object = tokens.get(1).map(String::as_str);
    if object == Some("DATABASE") {
        return Some("AlterDatabaseStmt");
    }
    if tokens.iter().any(|t| t == "RENAME") {
        return Some("RenameStmt");
    }
    if tokens.windows(2).any(|w| w[0] == "SET" && w[1] == "SCHEMA") {
        return Some("AlterObjectSchemaStmt");
    }
    if object == Some("SEQUENCE") {
        return Some("AlterSeqStmt");
    }
    Some("AlterTableStmt")
}

/// One canonical, engine-valid statement per top-level production — the classifier
/// fixture (and, per-production, an engine-reach anchor). Every production in
/// `stmt-productions.txt` appears exactly once and the classifier must map each `sql`
/// back to its `production`; a pinned test asserts the two lists partition each other.
const DUCKDB_STMT_PRODUCTION_FORMS: &[(&str, &str)] = &[
    (
        "AlterDatabaseStmt",
        "ALTER DATABASE probe_db SET ALIAS TO probe_alias",
    ),
    (
        "AlterObjectSchemaStmt",
        "ALTER TABLE probe_t SET SCHEMA probe_s",
    ),
    ("AlterSeqStmt", "ALTER SEQUENCE probe_seq RESTART"),
    (
        "AlterTableStmt",
        "ALTER TABLE probe_t ADD COLUMN probe_c INTEGER",
    ),
    ("AnalyzeStmt", "ANALYZE"),
    ("AttachStmt", "ATTACH 'probe.db' AS probe_att"),
    ("CallStmt", "CALL probe_proc()"),
    ("CheckPointStmt", "CHECKPOINT"),
    ("CommentOnStmt", "COMMENT ON TABLE probe_t IS 'note'"),
    ("CopyStmt", "COPY probe_t TO 'probe.csv'"),
    ("CreateAsStmt", "CREATE TABLE probe_ctas AS SELECT 1 AS a"),
    ("CreateFunctionStmt", "CREATE MACRO probe_macro(a) AS a + 1"),
    ("CreateSchemaStmt", "CREATE SCHEMA probe_schema"),
    ("CreateSecretStmt", "CREATE SECRET probe_secret (TYPE http)"),
    ("CreateSeqStmt", "CREATE SEQUENCE probe_sequence"),
    ("CreateStmt", "CREATE TABLE probe_table (a INTEGER)"),
    ("CreateTypeStmt", "CREATE TYPE probe_type AS ENUM ('x')"),
    ("DeallocateStmt", "DEALLOCATE probe_prepared"),
    ("DeleteStmt", "DELETE FROM probe_t"),
    ("DetachStmt", "DETACH probe_att"),
    ("DropSecretStmt", "DROP SECRET probe_secret"),
    ("DropStmt", "DROP TABLE probe_t"),
    ("ExecuteStmt", "EXECUTE probe_prepared"),
    ("ExplainStmt", "EXPLAIN SELECT 1"),
    ("ExportStmt", "EXPORT DATABASE 'probe_dir'"),
    ("ImportStmt", "IMPORT DATABASE 'probe_dir'"),
    ("IndexStmt", "CREATE INDEX probe_index ON probe_t (a)"),
    ("InsertStmt", "INSERT INTO probe_t VALUES (1)"),
    ("LoadStmt", "LOAD 'probe_ext'"),
    (
        "MergeIntoStmt",
        "MERGE INTO probe_t USING probe_s ON probe_t.a = probe_s.a WHEN MATCHED THEN DELETE",
    ),
    ("PragmaStmt", "PRAGMA database_list"),
    ("PrepareStmt", "PREPARE probe_prepared AS SELECT 1"),
    ("RenameStmt", "ALTER TABLE probe_t RENAME TO probe_t2"),
    ("SelectStmt", "SELECT 1"),
    ("TransactionStmt", "BEGIN TRANSACTION"),
    ("UpdateExtensionsStmt", "UPDATE EXTENSIONS"),
    ("UpdateStmt", "UPDATE probe_t SET a = 1"),
    ("UseStmt", "USE probe_db"),
    ("VacuumStmt", "VACUUM"),
    ("VariableResetStmt", "RESET memory_limit"),
    ("VariableSetStmt", "SET memory_limit = '1GB'"),
    ("VariableShowStmt", "SHOW TABLES"),
    ("ViewStmt", "CREATE VIEW probe_view AS SELECT 1 AS a"),
];

/// One vendored statement, tagged with its corpus label.
struct Entry {
    corpus: &'static str,
    sql: &'static str,
}

/// Every vendored statement in a fixed order (test-suite slice then docs anchors) —
/// the flat, schema-independent view used by the always-on parse check.
fn entries() -> Vec<Entry> {
    let mut out = Vec::new();
    for sql in DUCKDB_STATEMENTS.lines().filter(|l| !l.trim().is_empty()) {
        out.push(Entry {
            corpus: "test-suite",
            sql,
        });
    }
    for sql in DUCKDB_DOCS_EXAMPLES
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        out.push(Entry {
            corpus: "docs",
            sql,
        });
    }
    out
}

// --- The grouped setup-driver corpus -------------------------------------------------

/// A source-file group in the setup-driver corpus: the file's concrete `CREATE` setup
/// DDL and the selected statements-under-test drawn from it (see `extract.py`).
struct SchemaGroup {
    /// Source `.test` path (provenance; surfaced in the untriaged-over-acceptance report).
    file: &'static str,
    setup: Vec<&'static str>,
    queries: Vec<&'static str>,
}

/// Parse [`DUCKDB_WITH_SCHEMA`] into per-file groups. The format is line-oriented with
/// `# file:` / `# setup` / `# query` section markers; no extracted statement begins
/// with `#`, so the markers are unambiguous.
fn schema_groups() -> Vec<SchemaGroup> {
    #[derive(Clone, Copy)]
    enum Section {
        None,
        Setup,
        Query,
    }
    let mut groups: Vec<SchemaGroup> = Vec::new();
    let mut section = Section::None;
    for line in DUCKDB_WITH_SCHEMA.lines() {
        if let Some(file) = line.strip_prefix("# file:") {
            groups.push(SchemaGroup {
                file: file.trim(),
                setup: Vec::new(),
                queries: Vec::new(),
            });
            section = Section::None;
        } else if line == "# setup" {
            section = Section::Setup;
        } else if line == "# query" {
            section = Section::Query;
        } else if !line.trim().is_empty() {
            let group = groups
                .last_mut()
                .expect("a statement line precedes its `# file:` header");
            match section {
                Section::Setup => group.setup.push(line),
                Section::Query => group.queries.push(line),
                Section::None => panic!("statement outside a section: {line:?}"),
            }
        }
    }
    groups
}

/// A source-file group in the core-tranche corpus. Like [`SchemaGroup`] but carries a
/// third `rejects` list (the file's `statement error` bodies, `# reject` section), so
/// one sweep measures both coverage gaps (over `queries`) and over-acceptances (over
/// `rejects`) against the same per-file provisioned schema.
struct CoreSchemaGroup {
    file: &'static str,
    setup: Vec<&'static str>,
    queries: Vec<&'static str>,
    rejects: Vec<&'static str>,
}

/// Parse [`CORE_WITH_SCHEMA`] into per-file groups. Same line-oriented marker format as
/// [`schema_groups`] with an added `# reject` section; no extracted statement begins
/// with `#`, so the markers are unambiguous.
fn core_schema_groups() -> Vec<CoreSchemaGroup> {
    parse_core_groups(CORE_WITH_SCHEMA)
}

/// Parse a grouped setup-driver artifact (`# file:`/`# setup`/`# query`/`# reject`) into
/// [`CoreSchemaGroup`]s. Shared by the tranche-1 core corpus ([`core_schema_groups`]) and
/// the tranche-2 corpus ([`tranche2_schema_groups`]) — both `extract_*.py` emit the exact
/// same three-section format.
fn parse_core_groups(src: &'static str) -> Vec<CoreSchemaGroup> {
    #[derive(Clone, Copy)]
    enum Section {
        None,
        Setup,
        Query,
        Reject,
    }
    let mut groups: Vec<CoreSchemaGroup> = Vec::new();
    let mut section = Section::None;
    for line in src.lines() {
        if let Some(file) = line.strip_prefix("# file:") {
            groups.push(CoreSchemaGroup {
                file: file.trim(),
                setup: Vec::new(),
                queries: Vec::new(),
                rejects: Vec::new(),
            });
            section = Section::None;
        } else if line == "# setup" {
            section = Section::Setup;
        } else if line == "# query" {
            section = Section::Query;
        } else if line == "# reject" {
            section = Section::Reject;
        } else if !line.trim().is_empty() {
            let group = groups
                .last_mut()
                .expect("a statement line precedes its `# file:` header");
            match section {
                Section::Setup => group.setup.push(line),
                Section::Query => group.queries.push(line),
                Section::Reject => group.rejects.push(line),
                Section::None => panic!("statement outside a section: {line:?}"),
            }
        }
    }
    groups
}

/// Parse [`TRANCHE2_WITH_SCHEMA`] into per-file groups (the tranche-2 corpus). Same
/// three-section format and parser as [`core_schema_groups`].
fn tranche2_schema_groups() -> Vec<CoreSchemaGroup> {
    parse_core_groups(TRANCHE2_WITH_SCHEMA)
}

/// The docs-anchor statements as a schema-independent group (no source `.test` file, so
/// no DDL — the canonical forms are self-contained and comparable over a bare DB).
#[cfg(feature = "oracle-engines")]
fn docs_queries() -> Vec<&'static str> {
    DUCKDB_DOCS_EXAMPLES
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect()
}

// --- Over-acceptance ledger (the PG-ledger pattern, exact SQL) ------------------------

/// Current DuckDB over-acceptances allowed by the gate — each a
/// [`DivergenceEntry`](crate::verdict_harness::DivergenceEntry): a statement our parser
/// accepts that DuckDB *syntax*-rejects even with the file's schema provisioned, a real
/// validator-correctness divergence we knowingly tolerate. Mirrors the shared PG-ledger
/// pattern: every entry names an a non-empty provenance label, and the gate asserts each
/// still diverges, so a fixed over-acceptance cannot stay silently allowlisted.
/// The gate asserts every remaining entry still diverges (we accept, DuckDB *syntax*-rejects),
/// so a later change that closed one would flip it to a clean removal rather than a silent
/// weakening.
pub const DUCKDB_DIVERGENCE_ALLOWLIST: &[DivergenceEntry] = &[
    DivergenceEntry {
        sql: r#"FROM query_table([''])"#,
        ticket: "duckdb-from-clause-parse-overaccept",
        reason: "early-bind: DuckDB re-parses query_table()'s constant string arg as a table \
                 reference at parse (name-keyed); the empty name re-parses to end-of-input. \
                 query_table('a') parses and only Catalog-rejects at bind, so the constraint \
                 is on the argument value, not grammar shape — not replicated in a parse-only \
                 validator",
    },
    DivergenceEntry {
        sql: r#"from query($$select col."$$ || getvariable('col_name') || $$"::$$ || getvariable('col_type') || ' from tbl')"#,
        ticket: "duckdb-from-clause-parse-overaccept",
        reason: "early-bind: DuckDB constant-folds query()'s string arg and re-parses it as a \
                 subquery at parse (name-keyed); the non-constant getvariable folds to NULL -> \
                 'syntax error at or near NULL'. query('select 1') accepts, so the constraint \
                 is on argument constancy, not grammar shape — not replicated in a parse-only \
                 validator",
    },
    DivergenceEntry {
        sql: r#"from query('select extracted::' || getvariable('col_type') || ' from intermediate')"#,
        ticket: "duckdb-from-clause-parse-overaccept",
        reason: "early-bind: DuckDB constant-folds query()'s string arg and re-parses it as a \
                 subquery at parse (name-keyed); the non-constant getvariable folds to NULL -> \
                 'syntax error at or near NULL'. query('select 1') accepts, so the constraint \
                 is on argument constancy, not grammar shape — not replicated in a parse-only \
                 validator",
    },
    DivergenceEntry {
        sql: r#"PREPARE v3 AS PIVOT (SELECT empid, amount + ? AS amount, month FROM monthly_sales) ON MONTH USING SUM(AMOUNT)"#,
        ticket: "duckdb-prepare-execute-call",
        reason: "parse-time semantic reject: DuckDB refuses a `?` parameter inside the source of a \
                 *data-extracted* PIVOT — one whose ON columns carry no explicit `IN (...)` list, so \
                 the pivot columns are discovered by executing the source ('PIVOT statements with \
                 pivot elements extracted from the data cannot have parameters in their source'). The \
                 constraint is on the parameter's placement within the pivot's execution model, not \
                 grammar shape: the same `?` accepts in every other position (bare `SELECT ?`, \
                 `LIMIT ?`, the `USING SUM(x + ?)` aggregate, and a PIVOT source with an explicit IN \
                 list). Replicating it would mean modelling DuckDB's PIVOT rewrite + tracking \
                 parameters through the source subquery — semantic work a parse-only validator does \
                 not own (the early-bind `query`/`query_table` precedent above)",
    },
    DivergenceEntry {
        sql: "CREATE VIEW v1 AS PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)",
        ticket: "duckdb-statement-in-query-position",
        reason: "materialization restriction: DuckDB parse-rejects a *dynamic* PIVOT (no \
                 explicit ON … IN (…) values) in a VIEW body — 'PIVOT statements with pivot \
                 elements extracted from the data cannot be used in views' — because a stored \
                 view cannot scan data to fix its columns. The same dynamic PIVOT is accepted \
                 in a CTE / CREATE TABLE AS body (both materialize), so the reject is a \
                 data-dependent view-semantics rule, not a grammar shape — not replicated in a \
                 parse-only validator (the query_table/query() early-bind precedent above)",
    },
    DivergenceEntry {
        sql: "CREATE VIEW pivot_view AS PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT)",
        ticket: "duckdb-statement-in-query-position",
        reason: "materialization restriction: the same dynamic-PIVOT-in-a-VIEW reject as the \
                 entry above (no explicit ON … IN (…) values), here over a subquery source — a \
                 data-dependent view-semantics rule, not a grammar shape",
    },
    DivergenceEntry {
        sql: "CREATE MACRO xt2(a) as TABLE PIVOT sales ON d USING SUM(amount)",
        ticket: "duckdb-statement-in-query-position",
        reason: "materialization restriction: DuckDB parse-rejects a *dynamic* PIVOT (no \
                 explicit ON … IN (…) values) in a `CREATE MACRO … AS TABLE` body for the same \
                 reason as a VIEW — the macro's columns cannot be data-extracted at definition. \
                 The dynamic form is accepted in a CTE / CREATE TABLE AS body, so the reject is \
                 the same data-dependent deferred-body rule as the two VIEW entries above, not \
                 a grammar shape",
    },
];

// --- Coverage-gap routing (per-family, to the grammar-family children) ----------------

/// Each signature family (`signature_families`): the child ticket that owns closing its
/// coverage gaps, and its pinned oracle-verified coverage-gap count. A coverage gap
/// (DuckDB accepts, we reject) tagged with a family is allowlisted by that family
/// ticket; the gate asserts every ticket exists and every per-family count matches its
/// pin. Gaps that match no family route to the umbrella programme
/// ([`UNCLASSIFIED_GAP_TICKET`]).
const GAP_FAMILIES: &[(&str, &str, usize)] = &[
    ("star-modifiers", "duckdb-select-star-modifiers", 0),
    ("group/order-by-all", "duckdb-group-order-by-all", 0),
    ("from-first", "duckdb-from-first-select", 0),
    ("from-values", "duckdb-from-values-table-factor", 0),
    ("pivot/unpivot", "duckdb-pivot-unpivot", 0),
    ("collection-literals", "duckdb-collection-literals", 0),
    ("lambda", "duckdb-lambda-expressions", 0),
    ("nonstandard-joins", "duckdb-nonstandard-joins", 0),
    ("qualify", "duckdb-qualify-clause", 0),
    ("semi-anti-join", "duckdb-semi-anti-join", 0),
    ("union-by-name", "duckdb-union-by-name", 0),
    ("macro", "duckdb-create-macro", 0),
    (
        "settings-session",
        "duckdb-settings-and-session-statements",
        0,
    ),
    ("composite-types", "duckdb-composite-type-syntax", 0),
    ("prepare/execute/call", "duckdb-prepare-execute-call", 0),
    ("describe-show", "duckdb-statement-in-query-position", 0),
];

/// Where coverage gaps matching no signature family are tracked (the general-fill
/// surface: `DISTINCT ON`, `CREATE TABLE ... AS`, and other non-signature DuckDB-isms).
const UNCLASSIFIED_GAP_TICKET: &str = "duckdb-dialect-100-percent-programme";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The production denominator is well-formed (sorted, unique, pinned count) and the
    /// classifier partitions it: every canonical form in `DUCKDB_STMT_PRODUCTION_FORMS`
    /// maps back to its stated production, and the forms cover every production exactly
    /// once. Schema-independent, so a denominator drift or classifier regression fails the
    /// default conformance lane, not only the oracle sweep.
    #[test]
    fn duckdb_stmt_production_inventory_is_pinned() {
        let productions = duckdb_stmt_productions();
        assert_eq!(
            productions.len(),
            43,
            "DuckDB top-level stmt production count drifted; regenerate stmt-productions.txt \
             from the pinned statements.list",
        );
        // The committed file is exactly what `extract_stmt_productions.py` emits: sorted,
        // de-duped, one production per line.
        let lines: Vec<&str> = DUCKDB_STMT_PRODUCTIONS.lines().collect();
        assert_eq!(
            lines.len(),
            productions.len(),
            "stmt-productions.txt has blank or duplicate lines",
        );
        let mut sorted = lines.clone();
        sorted.sort_unstable();
        assert_eq!(lines, sorted, "stmt-productions.txt is not sorted");

        // The classifier partitions the denominator through the canonical forms.
        let mut covered: BTreeSet<&str> = BTreeSet::new();
        for &(production, sql) in DUCKDB_STMT_PRODUCTION_FORMS {
            assert!(
                productions.contains(production),
                "canonical-form production {production:?} is absent from stmt-productions.txt",
            );
            assert_eq!(
                duckdb_stmt_production(sql),
                Some(production),
                "classifier mapped canonical form {sql:?} to the wrong production",
            );
            assert!(
                covered.insert(production),
                "duplicate canonical-form production: {production}",
            );
        }
        assert_eq!(
            covered, productions,
            "canonical forms must cover every top-level production exactly once",
        );
    }

    #[test]
    fn corpora_are_pinned_and_parse_without_panicking() {
        // Always-on (no oracle needed): the count pins guard the fixtures, and every
        // statement must run through our parser cleanly (a verdict, never a panic).
        let stmts = DUCKDB_STATEMENTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        let docs = DUCKDB_DOCS_EXAMPLES
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        assert_eq!(
            stmts, STATEMENTS_PINNED,
            "statements.sql count changed; if intentional, update STATEMENTS_PINNED",
        );
        assert_eq!(
            docs, DOCS_PINNED,
            "docs_examples.sql count changed; if intentional, update DOCS_PINNED",
        );

        // The grouped setup-driver artifact: anti-vanishing pins + coherence with the
        // flat fixture (both are emitted from one `extract.py` run, so a stale or
        // half-regenerated artifact fails here without the oracle).
        let groups = schema_groups();
        assert_eq!(
            groups.len(),
            GROUPED_FILES_PINNED,
            "statements_with_schema.sql file-group count changed; re-pin GROUPED_FILES_PINNED",
        );
        let ddl: usize = groups.iter().map(|g| g.setup.len()).sum();
        assert_eq!(
            ddl, GROUPED_SETUP_DDL_PINNED,
            "statements_with_schema.sql setup-DDL count changed; re-pin GROUPED_SETUP_DDL_PINNED",
        );
        let grouped_qs: usize = groups.iter().map(|g| g.queries.len()).sum();
        assert_eq!(
            grouped_qs, STATEMENTS_PINNED,
            "grouped query count must equal statements.sql; regenerate both from extract.py",
        );
        let flat: BTreeSet<&str> = DUCKDB_STATEMENTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect();
        let grouped: BTreeSet<&str> = groups
            .iter()
            .flat_map(|g| g.queries.iter().copied())
            .collect();
        assert_eq!(
            flat, grouped,
            "grouped-artifact query set diverged from statements.sql — regenerate both from extract.py",
        );
        assert!(
            groups.iter().all(|g| g.file.starts_with("test/")),
            "every group must name its upstream `test/**/*.test` source file",
        );

        // Track the accept/reject split per corpus label (test-suite vs docs anchors)
        // so a regression in either fixture is legible without the oracle.
        let (mut accept, mut reject) = (0usize, 0usize);
        let (mut test_suite, mut docs_ex) = (0usize, 0usize);
        for entry in entries() {
            let ok = parse_with(entry.sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok();
            if ok {
                accept += 1;
            } else {
                reject += 1;
            }
            match entry.corpus {
                "docs" => docs_ex += usize::from(ok),
                _ => test_suite += usize::from(ok),
            }
        }
        eprintln!(
            "squonk {CORPUS_DIALECT:?} over the DuckDB corpus: {accept} accept / {reject} reject \
             of {} statements (test-suite accepts {test_suite}, docs accepts {docs_ex})",
            accept + reject,
        );
        // The corpus is signature-weighted DuckDB syntax, so the baseline rejects a
        // large share — that rejected set is exactly the coverage-gap surface the oracle
        // sweep quantifies. Assert only that both buckets are non-empty (the parser ran
        // end to end), not a brittle ratio.
        assert!(
            accept > 0 && reject > 0,
            "parser produced a degenerate split"
        );
    }

    #[test]
    fn core_tranche_is_pinned_and_parses_without_panicking() {
        // The broad core-tranche corpus (spec-audit): count pins guard the fixtures,
        // and every accept-corpus and reject-corpus line must run through our parser
        // cleanly (a verdict, never a panic — the P1 class this audit hunts). Measured
        // over v1.5.4: zero panics, zero hangs (slowest line ~0.1 ms).
        let stmts = CORE_STATEMENTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        let rejects = CORE_REJECTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        assert_eq!(
            stmts, CORE_STATEMENTS_PINNED,
            "duckdb-testsuite/statements.sql count changed; re-pin CORE_STATEMENTS_PINNED",
        );
        assert_eq!(
            rejects, CORE_REJECTS_PINNED,
            "duckdb-testsuite/rejects.sql count changed; re-pin CORE_REJECTS_PINNED",
        );

        // Grouped setup-driver artifact: anti-vanishing pins + coherence with both flat
        // fixtures (all three emitted from one `extract_core.py` run).
        let groups = core_schema_groups();
        assert_eq!(
            groups.len(),
            CORE_GROUPED_FILES_PINNED,
            "duckdb-testsuite grouped file-group count changed; re-pin CORE_GROUPED_FILES_PINNED",
        );
        let ddl: usize = groups.iter().map(|g| g.setup.len()).sum();
        assert_eq!(
            ddl, CORE_GROUPED_SETUP_DDL_PINNED,
            "duckdb-testsuite grouped setup-DDL count changed; re-pin CORE_GROUPED_SETUP_DDL_PINNED",
        );
        let grouped_q: BTreeSet<&str> = groups
            .iter()
            .flat_map(|g| g.queries.iter().copied())
            .collect();
        let flat_q: BTreeSet<&str> = CORE_STATEMENTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect();
        assert_eq!(
            flat_q, grouped_q,
            "grouped `# query` set diverged from statements.sql — regenerate both from extract_core.py",
        );
        let grouped_r: BTreeSet<&str> = groups
            .iter()
            .flat_map(|g| g.rejects.iter().copied())
            .collect();
        let flat_r: BTreeSet<&str> = CORE_REJECTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect();
        assert_eq!(
            flat_r, grouped_r,
            "grouped `# reject` set diverged from rejects.sql — regenerate both from extract_core.py",
        );
        assert!(
            groups.iter().all(|g| g.file.starts_with("test/sql/")),
            "every core group must name its upstream `test/sql/**/*.test` source file",
        );

        // Every line parses to a verdict (never a panic). Track the accept/reject split
        // per corpus so a regression is legible without the oracle.
        let (mut acc_a, mut acc_r) = (0usize, 0usize);
        for sql in &flat_q {
            if parse_with(sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok() {
                acc_a += 1;
            } else {
                acc_r += 1;
            }
        }
        let (mut rej_a, mut rej_r) = (0usize, 0usize);
        for sql in &flat_r {
            if parse_with(sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok() {
                rej_a += 1;
            } else {
                rej_r += 1;
            }
        }
        eprintln!(
            "squonk {CORPUS_DIALECT:?} over the core tranche: accepts {acc_a}/{} parse-accept, \
             rejects {rej_a}/{} parse-accept",
            acc_a + acc_r,
            rej_a + rej_r,
        );
        assert!(
            acc_a > 0 && rej_r > 0,
            "parser produced a degenerate split over the core tranche",
        );
    }

    #[test]
    fn tranche2_is_pinned_and_parses_without_panicking() {
        // The tranche-2 corpus (DML/DDL/copy/functions/pivot/prepared/optimizer/pragma/
        // tpc): count pins guard the fixtures, and every accept-corpus and reject-corpus
        // line must run through our parser cleanly (a verdict, never a panic — the P1
        // class this audit hunts). Same shape as the core-tranche always-on test.
        let stmts = TRANCHE2_STATEMENTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        let rejects = TRANCHE2_REJECTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        assert_eq!(
            stmts, TRANCHE2_STATEMENTS_PINNED,
            "duckdb-testsuite-tranche2/statements.sql count changed; re-pin TRANCHE2_STATEMENTS_PINNED",
        );
        assert_eq!(
            rejects, TRANCHE2_REJECTS_PINNED,
            "duckdb-testsuite-tranche2/rejects.sql count changed; re-pin TRANCHE2_REJECTS_PINNED",
        );

        // Grouped setup-driver artifact: anti-vanishing pins + coherence with both flat
        // fixtures (all three emitted from one `extract_tranche2.py` run).
        let groups = tranche2_schema_groups();
        assert_eq!(
            groups.len(),
            TRANCHE2_GROUPED_FILES_PINNED,
            "duckdb-testsuite-tranche2 grouped file-group count changed; re-pin TRANCHE2_GROUPED_FILES_PINNED",
        );
        let ddl: usize = groups.iter().map(|g| g.setup.len()).sum();
        assert_eq!(
            ddl, TRANCHE2_GROUPED_SETUP_DDL_PINNED,
            "duckdb-testsuite-tranche2 grouped setup-DDL count changed; re-pin TRANCHE2_GROUPED_SETUP_DDL_PINNED",
        );
        let grouped_q: BTreeSet<&str> = groups
            .iter()
            .flat_map(|g| g.queries.iter().copied())
            .collect();
        let flat_q: BTreeSet<&str> = TRANCHE2_STATEMENTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect();
        assert_eq!(
            flat_q, grouped_q,
            "grouped `# query` set diverged from statements.sql — regenerate both from extract_tranche2.py",
        );
        let grouped_r: BTreeSet<&str> = groups
            .iter()
            .flat_map(|g| g.rejects.iter().copied())
            .collect();
        let flat_r: BTreeSet<&str> = TRANCHE2_REJECTS
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect();
        assert_eq!(
            flat_r, grouped_r,
            "grouped `# reject` set diverged from rejects.sql — regenerate both from extract_tranche2.py",
        );
        assert!(
            groups.iter().all(|g| g.file.starts_with("test/sql/")),
            "every tranche-2 group must name its upstream `test/sql/**/*.test` source file",
        );

        // Every line parses to a verdict (never a panic). Track the accept/reject split
        // per corpus so a regression is legible without the oracle.
        let (mut acc_a, mut acc_r) = (0usize, 0usize);
        for sql in &flat_q {
            if parse_with(sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok() {
                acc_a += 1;
            } else {
                acc_r += 1;
            }
        }
        let (mut rej_a, mut rej_r) = (0usize, 0usize);
        for sql in &flat_r {
            if parse_with(sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok() {
                rej_a += 1;
            } else {
                rej_r += 1;
            }
        }
        eprintln!(
            "squonk {CORPUS_DIALECT:?} over tranche 2: accepts {acc_a}/{} parse-accept, \
             rejects {rej_a}/{} parse-accept",
            acc_a + acc_r,
            rej_a + rej_r,
        );
        assert!(
            acc_a > 0 && rej_r > 0,
            "parser produced a degenerate split over tranche 2",
        );
    }

    #[test]
    fn accepted_corpus_round_trips_under_duckdb() {
        // Always-on (no oracle): whatever the fitted preset accepts from the vendored
        // corpus must stay render-round-trip-stable in both modes — the same
        // accepted-subset property the Lenient corpus lane checks, sized to the
        // still-growing DuckDB acceptance boundary (each grammar-family child widens
        // it, and this lane gates the widened subset automatically).
        crate::corpus_roundtrip::assert_accepted_lines_round_trip(
            DUCKDB_STATEMENTS,
            CORPUS_DIALECT,
        );
        crate::corpus_roundtrip::assert_accepted_lines_round_trip(
            DUCKDB_DOCS_EXAMPLES,
            CORPUS_DIALECT,
        );
    }

    #[test]
    fn gap_family_and_allowlist_tickets_exist() {
        // Always-on: the coverage-gap routing and the over-acceptance ledger must only
        // ever name real tickets, so the family->ticket map and the allowlist cannot
        // rot even without the oracle feature. The "still diverges" half of the ledger
        // staleness check needs the engine and lives in the oracle sweep.
        for (family, ticket, _pin) in GAP_FAMILIES {
            assert!(
                ticket_exists(ticket),
                "coverage-gap family {family:?} routes to missing ticket {ticket}",
            );
        }
        assert!(
            ticket_exists(UNCLASSIFIED_GAP_TICKET),
            "unclassified-gap ticket {UNCLASSIFIED_GAP_TICKET} must exist",
        );
        assert_entries_are_ticketed(DUCKDB_DIVERGENCE_ALLOWLIST);
    }
}

/// The oracle differential — behind `oracle-engines` (needs `libduckdb`). Drives the
/// per-file setup driver and the allowlist-gated parity assertion, or skips cleanly if
/// the engine is unavailable at construction.
#[cfg(all(test, feature = "oracle-engines"))]
mod oracle_sweep {
    use super::*;
    use crate::duckdb_ffi::Connection;
    use crate::duckdb_structural::{DuckDbMediatedStructuralOracle, DuckDbMediatedVerdict};
    use crate::m2::DuckDbOracle;
    use crate::oracle::{AcceptRejectOracle, OracleUnavailable, OracleVerdict};
    use crate::verdict_harness::{
        Cell, Quadrant, RejectReason, Verdict, assert_entries_still_diverge,
    };
    use std::collections::{BTreeMap, BTreeSet};

    /// Map a DuckDB reject message onto the shared [`RejectReason`] trichotomy, read off
    /// the *bare* probe so binding rejects (a name/type the file's DDL does not cover)
    /// split from syntax rejects (real parser disagreement — schema-independent, so the
    /// ledgered class): a syntax reject is schema-independent, so the bare reason is
    /// authoritative for the split even when DuckDB rejects with schema too. The message
    /// strings are the DuckDB-specific part.
    fn classify_reject(err: &str) -> RejectReason {
        let e = err.to_ascii_lowercase();
        if e.contains("parser error") || e.contains("syntax error") {
            RejectReason::Syntax
        } else if e.contains("catalog error")
            || e.contains("binder error")
            || e.contains("does not exist")
            || e.contains("already exists")
            || e.contains("not found")
            || e.contains("referenced column")
            || e.contains("no function matches")
        {
            RejectReason::Binding
        } else {
            RejectReason::Other
        }
    }

    /// The nine signature families, detected on the same shapes the corpus extractor
    /// weights toward. Used to cross-tabulate coverage gaps so each grammar-family child
    /// ticket gets a real count. A statement can hit several families.
    fn signature_families(sql: &str) -> Vec<&'static str> {
        let upper = sql.to_ascii_uppercase();
        let mut fams = Vec::new();
        let mut push = |cond: bool, name: &'static str| {
            if cond {
                fams.push(name);
            }
        };
        let star_mod = [
            "* EXCLUDE",
            "*EXCLUDE",
            "* REPLACE",
            "*REPLACE",
            "* RENAME",
            "*RENAME",
        ]
        .iter()
        .any(|m| upper.contains(m))
            || upper.contains("COLUMNS(")
            || upper.contains("COLUMNS (");
        push(star_mod, "star-modifiers");
        push(
            upper.contains("GROUP BY ALL") || upper.contains("ORDER BY ALL"),
            "group/order-by-all",
        );
        push(upper.trim_start().starts_with("FROM "), "from-first");
        // DuckDB's bare `FROM VALUES (…) AS t` row-list table factor
        // (`duckdb-from-values-table-factor`), matched by the `FROM VALUES (` phrase. The
        // parenthesized `FROM (VALUES …)` derived table has a `(` before `VALUES`, so it
        // does not match — that form is always-on and never a gap. A leading `FROM VALUES`
        // co-tallies under `from-first` too (it starts with `FROM `); a `CREATE TABLE … AS
        // FROM VALUES` / CTE-bodied one does not, so it tallies here alone.
        push(
            upper.contains("FROM VALUES (") || upper.contains("FROM VALUES("),
            "from-values",
        );
        push(upper.contains("PIVOT"), "pivot/unpivot"); // matches UNPIVOT too
        push(
            sql.contains('[') && sql.contains(']')
                || sql.contains("{'")
                || upper.contains("MAP {")
                || upper.contains("MAP["),
            "collection-literals",
        );
        push(sql.contains("->"), "lambda");
        push(
            upper.contains("ASOF") || upper.contains("POSITIONAL"),
            "nonstandard-joins",
        );
        // Match the `SEMI`/`ANTI JOIN` phrase, not the bare word: `ANTI` alone is a
        // substring of ordinary identifiers (`QUANTITY`), and the phrase is what the
        // family owns. `ASOF SEMI JOIN` matches this *and* `nonstandard-joins` (via
        // ASOF) — both counts are intended (the shared multi-family tallying rule).
        push(
            upper.contains("SEMI JOIN") || upper.contains("ANTI JOIN"),
            "semi-anti-join",
        );
        push(upper.contains("QUALIFY"), "qualify");
        push(upper.contains("BY NAME"), "union-by-name");
        // DuckDB's DESCRIBE/SHOW/SUMMARIZE utility as a parenthesized FROM table source
        // (its `SHOW_REF`; `duckdb-statement-in-query-position`), matched by the leading
        // keyword directly inside a `(`. `DESCRIBE PIVOT …` co-tallies under
        // `pivot/unpivot` too (the shared multi-family rule); the PIVOT/UNPIVOT
        // *query-body* cases (CTE / CREATE VIEW-AS bodies this ticket also lands) tally
        // under `pivot/unpivot` alone, since their pin already owns the `PIVOT` word.
        let paren_show_ref = ["DESCRIBE", "SHOW", "SUMMARIZE"]
            .iter()
            .any(|kw| upper.contains(&format!("({kw}")) || upper.contains(&format!("( {kw}")));
        push(paren_show_ref, "describe-show");
        // The macro DDL family — a `CREATE [OR REPLACE] [TEMP] MACRO`, or a `CREATE
        // FUNCTION … AS` with the live (expr/query) body DuckDB spells as the `FUNCTION`
        // synonym. The trailing space after `MACRO` avoids matching a macro *call* whose
        // function name merely ends in `macro` (`…_macro(…)`).
        push(
            upper.trim_start().starts_with("CREATE")
                && (upper.contains(" MACRO ")
                    || (upper.contains(" FUNCTION ") && upper.contains(" AS "))),
            "macro",
        );
        // The settings & session statements this family owns: the leading `PRAGMA` and
        // `USE` keywords, and the `SET <name> = [ ... ]` bracketed-list value (matched by a
        // leading `SET` carrying a `[`, which co-occurs with the collection-literals
        // detector, as the tallying rule intends).
        let leading = upper.trim_start();
        push(
            leading.starts_with("PRAGMA")
                || leading.starts_with("USE ")
                || (leading.starts_with("SET ") && sql.contains('[')),
            "settings-session",
        );
        // Anonymous composite / nested type syntax (`duckdb-composite-type-syntax`):
        // the `STRUCT`/`ROW`/`UNION`/`MAP` type constructors and `TRY_CAST`. `STRUCT(`
        // is unambiguous (the `struct_pack`/`struct_insert` functions carry the `_`
        // infix, so they never match); the `ROW`/`UNION`/`MAP` keywords double as a
        // value constructor / set operator / value literal, so those are matched only in
        // an unmistakable *type* position (`::`/`AS ` immediately ahead) to avoid tagging
        // the value-side surfaces `union-by-name`/`collection-literals` already own.
        push(
            upper.contains("TRY_CAST")
                || upper.contains("STRUCT(")
                || upper.contains("::ROW(")
                || upper.contains("AS ROW(")
                || upper.contains("::UNION(")
                || upper.contains("AS UNION(")
                || upper.contains("::MAP(")
                || upper.contains("AS MAP("),
            "composite-types",
        );
        // The DuckDB prepared-statement lifecycle + `CALL` — a statement led by one of
        // these keywords. Once the grammar lands most parse (leaving this a gap only when
        // a co-occurring unsupported construct in the body still blocks it, so the
        // statement is tallied under both this family and that construct's).
        push(
            upper.trim_start().starts_with("PREPARE ")
                || upper.trim_start().starts_with("EXECUTE ")
                || upper.trim_start().starts_with("DEALLOCATE ")
                || upper.trim_start().starts_with("CALL "),
            "prepare/execute/call",
        );
        fams
    }

    /// json_serialize_sql over a probe connection. `Ok(None)` = the function is
    /// unavailable (json extension not loaded) — sampling is skipped, not failed.
    fn json_tree(probe: &Connection, sql: &str) -> Result<Option<String>, String> {
        let query = format!("SELECT json_serialize_sql('{}')", sql.replace('\'', "''"));
        match probe.query_string(&query) {
            Ok(json) => Ok(Some(json)),
            Err(err) => {
                let msg = err.0;
                if msg.to_ascii_lowercase().contains("json_serialize_sql") {
                    Ok(None) // scalar function missing -> unavailable
                } else {
                    Err(msg)
                }
            }
        }
    }

    /// Compose a group's setup DDL into one `execute_batch` script. The statements carry
    /// no internal `;` (extract.py enforces it), so `;`-joining is unambiguous.
    fn setup_script(setup: &[&str]) -> String {
        setup
            .iter()
            .map(|s| format!("{s};"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn duckdb_corpus_parity_gated_behind_setup_driver() {
        let bare = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            // Unreachable engine is an infrastructure skip, never a failure (ADR-0015).
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping DuckDB parity gate (oracle unavailable): {reason}");
                return;
            }
        };
        // The bare probe supplies reject *reasons* (the oracle only answers a bool). A
        // syntax reject is schema-independent, so the bare reason settles the
        // syntax/binding split even for statements DuckDB rejects with schema too.
        let bare_probe = Connection::open_in_memory().expect("bare probe connection");

        // The vendored file groups, plus a schema-independent docs-anchor group.
        let mut groups = schema_groups();
        groups.push(SchemaGroup {
            file: "<docs_examples>",
            setup: Vec::new(),
            queries: docs_queries(),
        });
        let total: usize = groups.iter().map(|g| g.queries.len()).sum();

        // --- Quadrant tallies (schema-provisioned) ---
        // The quadrant bookkeeping is shared (`quad`); the routing state below —
        // signature-family map, unclassified/untriaged/provisioning counters — is
        // duckdb's own gate policy.
        let mut quad = Quadrant::default();
        let mut provisioning_failed_files = 0usize; // execute_batch failed for the whole file
        let mut gaps_by_family: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut unclassified_gaps = 0usize;
        let mut untriaged_over_accept: Vec<(&str, &str)> = Vec::new(); // (file, sql)

        for group in &groups {
            // Provision the file's schema once (best-effort). An empty-setup group (docs,
            // or a file with no concrete CREATE) or a file whose DDL fails to provision
            // degrades to the bare comparison — its schema-dependent gaps are lost but no
            // false gate can fire, since over-accept-syntax is schema-independent.
            let schema = if group.setup.is_empty() {
                None
            } else {
                match DuckDbOracle::with_schema(&setup_script(&group.setup)) {
                    Ok(oracle) => Some(oracle),
                    Err(OracleUnavailable(_)) => {
                        provisioning_failed_files += 1;
                        None
                    }
                }
            };

            for &sql in &group.queries {
                let ours = parse_with(sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok();
                let bare_accepts = bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false);
                let schema_accepts = match &schema {
                    Some(oracle) => oracle.verdict(sql).map(|v| v.accepts()).unwrap_or(false),
                    None => bare_accepts, // no schema provisioned -> schema verdict == bare
                };
                let duck = bare_accepts || schema_accepts;
                let bare_reason = if duck {
                    RejectReason::Other // unused when DuckDB accepts
                } else {
                    bare_probe
                        .prepare_err(sql)
                        .map(|e| classify_reject(&e))
                        .unwrap_or(RejectReason::Other)
                };
                let v = Verdict {
                    ours,
                    bare_accepts,
                    schema_accepts,
                    bare_reason,
                };
                // `quad.record` does the shared quadrant bookkeeping; duckdb routes only
                // the two cells it gates (coverage gap -> signature families; syntax
                // over-acceptance -> the untriaged list gated to empty).
                match quad.record(&v) {
                    Cell::CoverageGap => {
                        let fams = signature_families(sql);
                        if fams.is_empty() {
                            unclassified_gaps += 1;
                        }
                        for fam in fams {
                            *gaps_by_family.entry(fam).or_default() += 1;
                        }
                    }
                    Cell::OverAcceptSyntax
                        if !DUCKDB_DIVERGENCE_ALLOWLIST.iter().any(|e| e.sql == sql) =>
                    {
                        untriaged_over_accept.push((group.file, sql));
                    }
                    _ => {}
                }
            }
        }

        let Quadrant {
            agree_accept,
            agree_reject_syntax,
            agree_reject_binding,
            coverage_gap,
            over_accept_syntax,
            over_accept_binding,
            over_accept_other,
            newly_comparable,
            comparable,
        } = quad;
        let residual = total - comparable;

        eprintln!(
            "\n=== DuckDB parity gate ({CORPUS_DIALECT:?} vs DuckDbOracle, per-file setup driver) ==="
        );
        eprintln!("  total statements            {total}");
        eprintln!(
            "  source-file groups          {} ({} provisioning-failed)",
            groups.len() - 1, // minus the synthetic docs group
            provisioning_failed_files,
        );
        eprintln!("  agree accept (A/A)          {agree_accept}");
        eprintln!("  agree reject syntax (R/R)   {agree_reject_syntax}   <- mutual syntax reject");
        eprintln!(
            "  agree reject binding (R/R)  {agree_reject_binding}   <- masked residual (binding)"
        );
        eprintln!("  COVERAGE GAP (R/A)          {coverage_gap}   <- DuckDB syntax we reject");
        eprintln!(
            "  over-accept SYNTAX (A/R)    {over_accept_syntax}   <- REAL over-acceptance (ledgered)"
        );
        eprintln!(
            "  over-accept binding (A/R)   {over_accept_binding}   <- residual (schema miss)"
        );
        eprintln!("  over-accept other  (A/R)    {over_accept_other}");
        eprintln!("  comparable                  {comparable} / {total}  (residual {residual})");
        eprintln!(
            "  newly comparable vs bare    {newly_comparable}   <- setup driver unblinded these"
        );
        eprintln!("\n  coverage gaps by signature family (route to the family child ticket):");
        for (fam, count) in &gaps_by_family {
            let ticket = GAP_FAMILIES
                .iter()
                .find(|(f, _, _)| f == fam)
                .map(|(_, t, _)| *t)
                .unwrap_or("<none>");
            eprintln!("    {fam:22} {count:>4}  [{ticket}]");
        }
        eprintln!(
            "    {:22} {unclassified_gaps:>4}  [{UNCLASSIFIED_GAP_TICKET}]",
            "(unclassified)"
        );

        // --- GATE 1: over-acceptance is fully ledgered ---
        // "We accept / DuckDB syntax-rejects" is a real validator-correctness
        // divergence. Every one must be in the allowlist (with a ticket) or fixed.
        // Phase 0 found zero; this keeps it zero at scale.
        if !untriaged_over_accept.is_empty() {
            eprintln!(
                "\nUNTRIAGED over-acceptances ({}):",
                untriaged_over_accept.len()
            );
            for (file, sql) in &untriaged_over_accept {
                eprintln!("  A/R-syntax [{file}] {sql:?}");
            }
        }
        assert!(
            untriaged_over_accept.is_empty(),
            "{} untriaged DuckDB over-acceptance(s) (we accept, DuckDB syntax-rejects even with the \
             file schema): fix the parser or add a DUCKDB_DIVERGENCE_ALLOWLIST entry (with a ticket)",
            untriaged_over_accept.len(),
        );

        // --- GATE 2: setup-driver efficacy + anti-drift ---
        // The per-file driver removes the binding *noise* class the bare sweep could not
        // see. `over_accept_binding` is not literally zero — the corpus name space is
        // unbounded (templated / cross-file / extension DDL the per-file concrete DDL
        // cannot provision), so a documented, pinned residual is the honest bound (the
        // ticket's STOP fallback), not a green-forcing silence. These pins drift if the
        // schema, corpus, engine, or dialect changes, forcing a reviewed re-baseline.
        assert_eq!(
            (
                newly_comparable,
                over_accept_binding,
                agree_reject_binding,
                residual
            ),
            (
                NEWLY_COMPARABLE_PIN,
                OVER_ACCEPT_BINDING_PIN,
                AGREE_REJECT_BINDING_PIN,
                RESIDUAL_PIN
            ),
            "setup-driver quadrant counts drifted (newly_comparable, over_accept_binding, \
             agree_reject_binding, residual); re-baseline the pins and update the inventory",
        );

        // --- GATE 3: per-family coverage-gap counts match their pins (deliverable e) ---
        // Every family with gaps must be mapped to a child ticket, and its
        // oracle-verified count must match the pinned matrix column. A closed or
        // regressed family drifts here.
        for fam in gaps_by_family.keys() {
            assert!(
                GAP_FAMILIES.iter().any(|(f, _, _)| f == fam),
                "coverage-gap family {fam:?} has no child ticket in GAP_FAMILIES",
            );
        }
        for (fam, ticket, pin) in GAP_FAMILIES {
            let got = gaps_by_family.get(fam).copied().unwrap_or(0);
            assert_eq!(
                got, *pin,
                "coverage-gap count for family {fam:?} ({ticket}) drifted; re-baseline its \
                 GAP_FAMILIES pin and update the child-ticket inventory",
            );
        }
        assert_eq!(
            unclassified_gaps, UNCLASSIFIED_GAP_PIN,
            "unclassified coverage-gap count drifted; re-baseline UNCLASSIFIED_GAP_PIN",
        );
        assert_eq!(
            coverage_gap, COVERAGE_GAP_PIN,
            "total coverage-gap count drifted; re-baseline COVERAGE_GAP_PIN",
        );

        // --- Ledger staleness: every allowlisted over-acceptance still diverges ---
        assert_entries_still_diverge(DUCKDB_DIVERGENCE_ALLOWLIST, |entry| {
            let ours = parse_with(entry.sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok();
            let duck = bare
                .verdict(entry.sql)
                .map(|v| v.accepts())
                .unwrap_or(false);
            let reason = bare_probe
                .prepare_err(entry.sql)
                .map(|e| classify_reject(&e))
                .unwrap_or(RejectReason::Other);
            ours && !duck && reason == RejectReason::Syntax
        });

        // Verdict lifting sanity: the oracle never yields a non-Accept/Reject verdict.
        assert!(matches!(
            bare.verdict("SELECT 1").unwrap(),
            OracleVerdict::Accept | OracleVerdict::Reject,
        ));
    }

    // --- Core-tranche spec-audit measured pins (spec-audit-duckdb-test-suite-corpus) --
    //
    // Baselined against DuckDB 1.5.4 (08e34c447b) + the vendored core-tranche corpus
    // under the fitted `CORPUS_DIALECT`, with the per-file setup driver. Two quadrants
    // are pinned: one over the accept surface (`# query`) and one over the known-reject
    // surface (`# reject`). These are measurement baselines, not zero gates: a drift
    // fails loudly so the inventory is re-read and re-baselined, but nothing is forced to
    // zero and no ticket is required here. The tuples are `(agree_accept, coverage_gap,
    // over_accept_syntax, over_accept_binding, over_accept_other, agree_reject_syntax,
    // agree_reject_binding)`.
    // `duckdb-geometry-type-and-overlaps-operator` closed the GEOMETRY + `&&` family:
    // 9 accept-surface coverage gaps (7 `&&` overlap statements + 2 `GEOMETRY('OGC:CRS…')`
    // string-modifier CREATEs) flipped to agree-accept (coverage_gap 24 → 15, agree_accept
    // 5412 → 5421). On the reject surface, one `type_name('string')` form (a `SET(...)`-shaped
    // constant-modifier cast) our parser now correctly parse-accepts — DuckDB *binder*-rejects
    // it, a non-syntax over-accept — moved from agree_reject_binding to over_accept_other
    // (over_accept_syntax stays 0).
    // `duckdb-testsuite-small-gap-families` closed the FILTER-sans-WHERE family: 3 accept-surface
    // coverage gaps (`SUM(x) FILTER (cond)` without the standard `WHERE`, incl. the two
    // `FILTER (…) OVER (…)` streaming-window forms) flipped to agree-accept
    // (coverage_gap 15 → 12, agree_accept 5421 → 5424) via
    // `AggregateCallSyntax::filter_optional_where`; the keyword omission round-trips through
    // `FunctionCall::filter_where`, so no over-acceptance is introduced (over_accept_syntax
    // stays 0).
    // Accept-surface quadrant. over_accept_syntax is pinned 0: the DuckDb-specific accept forms
    // (the two-word `<expr> NOT NULL` postfix, `DROP MACRO`, short/typeless generated columns,
    // …) round-trip verbatim, so none introduces a syntax over-acceptance.
    const CORE_ACCEPT_QUADRANT: (usize, usize, usize, usize, usize, usize, usize) =
        (5435, 1, 0, 220, 0, 2, 10);
    // Reject-surface quadrant. over_accept_syntax is pinned 26 (the reject-surface syntax
    // over-accept floor); a syntactically-valid body DuckDB rejects only at *bind* time counts in
    // over_accept_binding, the documented never-ledgered noise class, not the syntax floor.
    const CORE_REJECT_QUADRANT: (usize, usize, usize, usize, usize, usize, usize) =
        (513, 1, 26, 472, 27, 70, 10);
    // 6 of 801 source files' DDL cannot provision on a fresh in-memory DB (ATTACH'd
    // databases, extension types, file-backed sources) — degraded to the bare
    // comparison, a counted residual, never a false gate (the STOP fallback).
    const CORE_PROVISIONING_FAILED_PIN: usize = 6;

    #[test]
    fn core_tranche_spec_audit_inventory() {
        let bare = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping core-tranche inventory (oracle unavailable): {reason}");
                return;
            }
        };
        let bare_probe = Connection::open_in_memory().expect("bare probe connection");
        let groups = core_schema_groups();

        let mut q_accept = Quadrant::default();
        let mut q_reject = Quadrant::default();
        let mut provisioning_failed = 0usize;
        let mut gaps_by_family: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut unclassified_gaps = 0usize;
        let mut coverage_gaps: Vec<(&str, &str)> = Vec::new();
        let mut accept_over_syntax: Vec<(&str, &str)> = Vec::new();
        let mut reject_over_syntax: Vec<(&str, &str)> = Vec::new();

        for group in &groups {
            let schema = if group.setup.is_empty() {
                None
            } else {
                match DuckDbOracle::with_schema(&setup_script(&group.setup)) {
                    Ok(oracle) => Some(oracle),
                    Err(OracleUnavailable(_)) => {
                        provisioning_failed += 1;
                        None
                    }
                }
            };
            let verdict_of = |sql: &str| -> Verdict {
                let ours = parse_with(sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok();
                let bare_accepts = bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false);
                let schema_accepts = match &schema {
                    Some(oracle) => oracle.verdict(sql).map(|v| v.accepts()).unwrap_or(false),
                    None => bare_accepts,
                };
                let duck = bare_accepts || schema_accepts;
                let bare_reason = if duck {
                    RejectReason::Other
                } else {
                    bare_probe
                        .prepare_err(sql)
                        .map(|e| classify_reject(&e))
                        .unwrap_or(RejectReason::Other)
                };
                Verdict {
                    ours,
                    bare_accepts,
                    schema_accepts,
                    bare_reason,
                }
            };

            for &sql in &group.queries {
                let v = verdict_of(sql);
                match q_accept.record(&v) {
                    Cell::CoverageGap => {
                        coverage_gaps.push((group.file, sql));
                        let fams = signature_families(sql);
                        if fams.is_empty() {
                            unclassified_gaps += 1;
                        }
                        for fam in fams {
                            *gaps_by_family.entry(fam).or_default() += 1;
                        }
                    }
                    Cell::OverAcceptSyntax => accept_over_syntax.push((group.file, sql)),
                    _ => {}
                }
            }
            for &sql in &group.rejects {
                let v = verdict_of(sql);
                if let Cell::OverAcceptSyntax = q_reject.record(&v) {
                    reject_over_syntax.push((group.file, sql));
                }
            }
        }

        let a = &q_accept;
        let r = &q_reject;
        eprintln!(
            "\n=== DuckDB core-tranche spec-audit inventory ({CORPUS_DIALECT:?} vs DuckDbOracle, \
             v1.5.4, per-file setup driver) ==="
        );
        eprintln!(
            "  source-file groups          {} ({provisioning_failed} provisioning-failed)",
            groups.len(),
        );
        eprintln!("\n  ACCEPT surface (`statement ok` + `query` bodies):");
        eprintln!("    agree accept (A/A)        {}", a.agree_accept);
        eprintln!(
            "    COVERAGE GAP (R/A)        {}   <- DuckDB accepts, we reject",
            a.coverage_gap,
        );
        eprintln!(
            "    over-accept SYNTAX (A/R)  {}   <- we accept, DuckDB syntax-rejects",
            a.over_accept_syntax,
        );
        eprintln!("    over-accept binding (A/R) {}", a.over_accept_binding);
        eprintln!("    over-accept other  (A/R)  {}", a.over_accept_other);
        eprintln!("    agree reject syntax (R/R) {}", a.agree_reject_syntax);
        eprintln!("    agree reject binding(R/R) {}", a.agree_reject_binding);
        eprintln!("\n  REJECT surface (`statement error` bodies — the over-accept differential):");
        eprintln!(
            "    over-accept SYNTAX (A/R)  {}   <- we accept, DuckDB syntax-rejects (REAL over-accept)",
            r.over_accept_syntax,
        );
        eprintln!("    over-accept binding (A/R) {}", r.over_accept_binding);
        eprintln!(
            "    over-accept other  (A/R)  {}   <- we accept, DuckDB rejects (runtime/semantic)",
            r.over_accept_other,
        );
        eprintln!(
            "    agree reject syntax (R/R) {}   <- both reject at parse",
            r.agree_reject_syntax,
        );
        eprintln!("    agree reject binding(R/R) {}", r.agree_reject_binding);
        eprintln!(
            "    agree accept (A/A)        {}   <- DuckDB prepares it (error is runtime-only)",
            r.agree_accept,
        );

        eprintln!("\n  coverage gaps by signature family (accept surface):");
        for (fam, count) in &gaps_by_family {
            eprintln!("    {fam:22} {count:>4}");
        }
        eprintln!("    {:22} {unclassified_gaps:>4}", "(unclassified)");

        eprintln!(
            "\n  --- coverage gaps ({}) [DuckDB accepts, we reject] ---",
            coverage_gaps.len()
        );
        for (file, sql) in &coverage_gaps {
            eprintln!("    R/A [{file}] {sql:?}");
        }
        eprintln!(
            "\n  --- accept-surface over-acceptances ({}) [we accept, DuckDB syntax-rejects] ---",
            accept_over_syntax.len()
        );
        for (file, sql) in &accept_over_syntax {
            eprintln!("    A/R [{file}] {sql:?}");
        }
        eprintln!(
            "\n  --- reject-surface over-acceptances ({}) [we accept, DuckDB syntax-rejects] ---",
            reject_over_syntax.len()
        );
        for (file, sql) in &reject_over_syntax {
            eprintln!("    A/R [{file}] {sql:?}");
        }

        let got_accept = (
            a.agree_accept,
            a.coverage_gap,
            a.over_accept_syntax,
            a.over_accept_binding,
            a.over_accept_other,
            a.agree_reject_syntax,
            a.agree_reject_binding,
        );
        let got_reject = (
            r.agree_accept,
            r.coverage_gap,
            r.over_accept_syntax,
            r.over_accept_binding,
            r.over_accept_other,
            r.agree_reject_syntax,
            r.agree_reject_binding,
        );
        eprintln!("\n  MEASURED accept quadrant tuple: {got_accept:?}");
        eprintln!("  MEASURED reject quadrant tuple: {got_reject:?}");
        eprintln!("  MEASURED provisioning-failed:   {provisioning_failed}");

        // Anti-drift pins (measurement baseline, not a gate). A closed gap, a fresh
        // regression, a schema/corpus/engine change, or a dialect tweak drifts one of
        // these and fails loudly so the inventory is re-read and re-baselined.
        assert_eq!(
            got_accept, CORE_ACCEPT_QUADRANT,
            "core-tranche ACCEPT quadrant drifted; re-baseline CORE_ACCEPT_QUADRANT",
        );
        assert_eq!(
            got_reject, CORE_REJECT_QUADRANT,
            "core-tranche REJECT quadrant drifted; re-baseline CORE_REJECT_QUADRANT",
        );
        assert_eq!(
            provisioning_failed, CORE_PROVISIONING_FAILED_PIN,
            "core-tranche provisioning-failed count drifted; re-baseline CORE_PROVISIONING_FAILED_PIN",
        );
    }

    // --- Tranche-2 spec-audit measured pins (spec-audit-duckdb-remaining-tranches) ----
    //
    // Baselined against DuckDB 1.5.4 (08e34c447b) + the vendored tranche-2 corpus under
    // the fitted `CORPUS_DIALECT`, with the per-file setup driver. Same two-quadrant
    // shape as the tranche-1 `CORE_*_QUADRANT` pins (accept surface over `# query`,
    // reject surface over `# reject`), measured independently so nothing here disturbs
    // the tranche-1 pins. The tuples are `(agree_accept, coverage_gap, over_accept_syntax,
    // over_accept_binding, over_accept_other, agree_reject_syntax, agree_reject_binding)`.
    // ACCEPT surface: 78 coverage gaps (DuckDB accepts, we reject) — the remaining fix-child
    // inventory, one lower after `duckdb-postfix-operator-dimension` closed the `SELECT 10!`
    // postfix gap (it moves into agree_accept). over_accept_syntax = 4: the early-bind
    // `query(...)`/`read_csv(getvariable(...))` string-arg class, already reasoned in
    // `DUCKDB_DIVERGENCE_ALLOWLIST` (DuckDB constant-folds+re-parses a string arg; a non-constant
    // fold → syntax error — not replicable in a parse-only validator). A syntactically-valid body
    // DuckDB rejects only at bind time counts in over_accept_binding, the net-conserved
    // binding-noise class, not the syntax floor.
    const TRANCHE2_ACCEPT_QUADRANT: (usize, usize, usize, usize, usize, usize, usize) =
        (6474, 78, 4, 998, 56, 0, 24);
    // REJECT surface (known DuckDB `statement error` bodies): over_accept_syntax = 19 — not real
    // grammar over-acceptance but argument-value/arity semantic checks DuckDB spells as Parser
    // Errors that a parse-only validator legitimately does not enforce (the early-bind allowlist
    // class), plus dynamic-PIVOT materialization restrictions already in
    // `DUCKDB_DIVERGENCE_ALLOWLIST`. A syntactically-valid body DuckDB rejects only at
    // bind/runtime counts in over_accept_binding/over_accept_other, the net-conserved
    // binding-noise classes.
    const TRANCHE2_REJECT_QUADRANT: (usize, usize, usize, usize, usize, usize, usize) =
        (609, 44, 19, 476, 71, 75, 25);
    // 85 of 1429 source files' DDL cannot provision on a fresh in-memory DB (ATTACH'd /
    // encrypted / file-backed / extension-typed sources — far more than tranche 1's 6, as
    // expected for copy/storage/attach). Degraded to the bare comparison, a counted
    // residual, never a false gate (the STOP fallback).
    const TRANCHE2_PROVISIONING_FAILED_PIN: usize = 85;

    #[test]
    fn tranche2_spec_audit_inventory() {
        let bare = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping tranche-2 inventory (oracle unavailable): {reason}");
                return;
            }
        };
        let bare_probe = Connection::open_in_memory().expect("bare probe connection");
        let groups = tranche2_schema_groups();

        let mut q_accept = Quadrant::default();
        let mut q_reject = Quadrant::default();
        let mut provisioning_failed = 0usize;
        let mut gaps_by_family: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut unclassified_gaps = 0usize;
        let mut coverage_gaps: Vec<(&str, &str)> = Vec::new();
        let mut accept_over_syntax: Vec<(&str, &str)> = Vec::new();
        let mut reject_over_syntax: Vec<(&str, &str)> = Vec::new();

        for group in &groups {
            let schema = if group.setup.is_empty() {
                None
            } else {
                match DuckDbOracle::with_schema(&setup_script(&group.setup)) {
                    Ok(oracle) => Some(oracle),
                    Err(OracleUnavailable(_)) => {
                        provisioning_failed += 1;
                        None
                    }
                }
            };
            let verdict_of = |sql: &str| -> Verdict {
                let ours = parse_with(sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok();
                let bare_accepts = bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false);
                let schema_accepts = match &schema {
                    Some(oracle) => oracle.verdict(sql).map(|v| v.accepts()).unwrap_or(false),
                    None => bare_accepts,
                };
                let duck = bare_accepts || schema_accepts;
                let bare_reason = if duck {
                    RejectReason::Other
                } else {
                    bare_probe
                        .prepare_err(sql)
                        .map(|e| classify_reject(&e))
                        .unwrap_or(RejectReason::Other)
                };
                Verdict {
                    ours,
                    bare_accepts,
                    schema_accepts,
                    bare_reason,
                }
            };

            for &sql in &group.queries {
                let v = verdict_of(sql);
                match q_accept.record(&v) {
                    Cell::CoverageGap => {
                        coverage_gaps.push((group.file, sql));
                        let fams = signature_families(sql);
                        if fams.is_empty() {
                            unclassified_gaps += 1;
                        }
                        for fam in fams {
                            *gaps_by_family.entry(fam).or_default() += 1;
                        }
                    }
                    Cell::OverAcceptSyntax => accept_over_syntax.push((group.file, sql)),
                    _ => {}
                }
            }
            for &sql in &group.rejects {
                let v = verdict_of(sql);
                if let Cell::OverAcceptSyntax = q_reject.record(&v) {
                    reject_over_syntax.push((group.file, sql));
                }
            }
        }

        let a = &q_accept;
        let r = &q_reject;
        eprintln!(
            "\n=== DuckDB tranche-2 spec-audit inventory ({CORPUS_DIALECT:?} vs DuckDbOracle, \
             v1.5.4, per-file setup driver) ==="
        );
        eprintln!(
            "  source-file groups          {} ({provisioning_failed} provisioning-failed)",
            groups.len(),
        );
        eprintln!("\n  ACCEPT surface (`statement ok` + `query` bodies):");
        eprintln!("    agree accept (A/A)        {}", a.agree_accept);
        eprintln!(
            "    COVERAGE GAP (R/A)        {}   <- DuckDB accepts, we reject",
            a.coverage_gap,
        );
        eprintln!(
            "    over-accept SYNTAX (A/R)  {}   <- we accept, DuckDB syntax-rejects",
            a.over_accept_syntax,
        );
        eprintln!("    over-accept binding (A/R) {}", a.over_accept_binding);
        eprintln!("    over-accept other  (A/R)  {}", a.over_accept_other);
        eprintln!("    agree reject syntax (R/R) {}", a.agree_reject_syntax);
        eprintln!("    agree reject binding(R/R) {}", a.agree_reject_binding);
        eprintln!("\n  REJECT surface (`statement error` bodies — the over-accept differential):");
        eprintln!(
            "    over-accept SYNTAX (A/R)  {}   <- we accept, DuckDB syntax-rejects (REAL over-accept)",
            r.over_accept_syntax,
        );
        eprintln!("    over-accept binding (A/R) {}", r.over_accept_binding);
        eprintln!(
            "    over-accept other  (A/R)  {}   <- we accept, DuckDB rejects (runtime/semantic)",
            r.over_accept_other,
        );
        eprintln!(
            "    agree reject syntax (R/R) {}   <- both reject at parse",
            r.agree_reject_syntax,
        );
        eprintln!("    agree reject binding(R/R) {}", r.agree_reject_binding);
        eprintln!(
            "    agree accept (A/A)        {}   <- DuckDB prepares it (error is runtime-only)",
            r.agree_accept,
        );

        eprintln!("\n  coverage gaps by signature family (accept surface):");
        for (fam, count) in &gaps_by_family {
            eprintln!("    {fam:22} {count:>4}");
        }
        eprintln!("    {:22} {unclassified_gaps:>4}", "(unclassified)");

        eprintln!(
            "\n  --- coverage gaps ({}) [DuckDB accepts, we reject] ---",
            coverage_gaps.len()
        );
        for (file, sql) in &coverage_gaps {
            eprintln!("    R/A [{file}] {sql:?}");
        }
        eprintln!(
            "\n  --- accept-surface over-acceptances ({}) [we accept, DuckDB syntax-rejects] ---",
            accept_over_syntax.len()
        );
        for (file, sql) in &accept_over_syntax {
            eprintln!("    A/R [{file}] {sql:?}");
        }
        eprintln!(
            "\n  --- reject-surface over-acceptances ({}) [we accept, DuckDB syntax-rejects] ---",
            reject_over_syntax.len()
        );
        for (file, sql) in &reject_over_syntax {
            eprintln!("    A/R [{file}] {sql:?}");
        }

        let got_accept = (
            a.agree_accept,
            a.coverage_gap,
            a.over_accept_syntax,
            a.over_accept_binding,
            a.over_accept_other,
            a.agree_reject_syntax,
            a.agree_reject_binding,
        );
        let got_reject = (
            r.agree_accept,
            r.coverage_gap,
            r.over_accept_syntax,
            r.over_accept_binding,
            r.over_accept_other,
            r.agree_reject_syntax,
            r.agree_reject_binding,
        );
        eprintln!("\n  MEASURED accept quadrant tuple: {got_accept:?}");
        eprintln!("  MEASURED reject quadrant tuple: {got_reject:?}");
        eprintln!("  MEASURED provisioning-failed:   {provisioning_failed}");

        // Anti-drift pins (measurement baseline, not a gate) — mirror the tranche-1
        // `core_tranche_spec_audit_inventory` pins, independent from them.
        assert_eq!(
            got_accept, TRANCHE2_ACCEPT_QUADRANT,
            "tranche-2 ACCEPT quadrant drifted; re-baseline TRANCHE2_ACCEPT_QUADRANT",
        );
        assert_eq!(
            got_reject, TRANCHE2_REJECT_QUADRANT,
            "tranche-2 REJECT quadrant drifted; re-baseline TRANCHE2_REJECT_QUADRANT",
        );
        assert_eq!(
            provisioning_failed, TRANCHE2_PROVISIONING_FAILED_PIN,
            "tranche-2 provisioning-failed count drifted; re-baseline TRANCHE2_PROVISIONING_FAILED_PIN",
        );
    }

    // --- Measured statement-production coverage (spec-coverage-duckdb-production-inventory) ---
    //
    // Engine-production reach: which of the 43 top-level `stmt` productions the DuckDB
    // oracle reaches over each vendored accept-surface tranche, mapped through the
    // grammar-faithful `duckdb_stmt_production` classifier. A production counts as reached
    // in a tranche only when the oracle ACCEPTS (bare or under the file's provisioned
    // schema) an accept-surface statement that classifies to it — engine reach gated on
    // the real oracle, kept DISTINCT from squonk acceptance (its own set) and from the
    // structural/quadrant sweeps above. Measurement baseline, not a gate: a closed gap or a
    // corpus/engine change drifts a pin and fails loudly for review + re-baseline.

    /// Productions no CORE-tranche accept-surface statement reaches under the engine. The
    /// core tranche is a select/join/subquery/aggregate/window/cte/order/limit/types slice,
    /// so DDL and administrative productions are expected to be absent here. Measured: 26/43.
    const DUCKDB_CORE_UNEXERCISED_PRODUCTIONS: &[&str] = &[
        "AlterDatabaseStmt",
        "AlterObjectSchemaStmt",
        "AlterSeqStmt",
        "AnalyzeStmt",
        "AttachStmt",
        "CommentOnStmt",
        "CopyStmt",
        "CreateSecretStmt",
        "DeallocateStmt",
        "DropSecretStmt",
        "ExecuteStmt",
        "ExportStmt",
        "ImportStmt",
        "IndexStmt",
        "MergeIntoStmt",
        "UpdateExtensionsStmt",
        "VacuumStmt",
    ];
    /// Productions no TRANCHE-2 accept-surface statement reaches under the engine. Tranche 2
    /// adds the DDL/DML/administrative directories (create/alter/insert/merge/copy/pragma/…),
    /// so it reaches most DDL — LoadStmt is the lone production core reaches that it does not.
    /// Measured: 32/43.
    const DUCKDB_TRANCHE2_UNEXERCISED_PRODUCTIONS: &[&str] = &[
        "AlterObjectSchemaStmt",
        "AnalyzeStmt",
        "CommentOnStmt",
        "CreateSecretStmt",
        "DropSecretStmt",
        "ExecuteStmt",
        "ExportStmt",
        "ImportStmt",
        "LoadStmt",
        "UpdateExtensionsStmt",
        "VacuumStmt",
    ];
    /// Productions NEITHER tranche reaches — the measured uncovered-family list (combined
    /// engine reach 33/43). Each is the negative space closed by the authored engine-reach
    /// probes below (`duckdb_uncovered_statement_productions_have_permanent_oracle_probes`).
    const DUCKDB_COMBINED_UNEXERCISED_PRODUCTIONS: &[&str] = &[
        "AlterObjectSchemaStmt",
        "AnalyzeStmt",
        "CommentOnStmt",
        "CreateSecretStmt",
        "DropSecretStmt",
        "ExecuteStmt",
        "ExportStmt",
        "ImportStmt",
        "UpdateExtensionsStmt",
        "VacuumStmt",
    ];
    /// Productions NEITHER tranche reaches under SQUONK — the distinct squonk-reach
    /// column (mirrors the PostgreSQL instrument's `squonk_accepts`). A superset of the
    /// engine-uncovered set: engine reach does not imply squonk parses the production.
    /// This column equals the engine-uncovered set exactly (no extra entries): squonk
    /// parses every corpus production the engine reaches. Measured: squonk reach 33/43
    /// (combined).
    const DUCKDB_COMBINED_SQUONK_UNEXERCISED_PRODUCTIONS: &[&str] = &[
        "AlterObjectSchemaStmt",
        "AnalyzeStmt",
        "CommentOnStmt",
        "CreateSecretStmt",
        "DropSecretStmt",
        "ExecuteStmt",
        "ExportStmt",
        "ImportStmt",
        "UpdateExtensionsStmt",
        "VacuumStmt",
    ];

    #[test]
    fn duckdb_statement_production_coverage_is_measured() {
        let bare = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping DuckDB production coverage (oracle unavailable): {reason}");
                return;
            }
        };
        let productions = duckdb_stmt_productions();

        struct Reach {
            engine: BTreeSet<&'static str>,
            squonk: BTreeSet<&'static str>,
            unmapped: BTreeSet<String>,
            accepted: usize,
        }

        let measure = |groups: &[CoreSchemaGroup]| -> Reach {
            let mut r = Reach {
                engine: BTreeSet::new(),
                squonk: BTreeSet::new(),
                unmapped: BTreeSet::new(),
                accepted: 0,
            };
            for group in groups {
                let schema = if group.setup.is_empty() {
                    None
                } else {
                    DuckDbOracle::with_schema(&setup_script(&group.setup)).ok()
                };
                for &sql in &group.queries {
                    let bare_accepts = bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false);
                    let schema_accepts = schema
                        .as_ref()
                        .is_some_and(|o| o.verdict(sql).map(|v| v.accepts()).unwrap_or(false));
                    if !(bare_accepts || schema_accepts) {
                        continue; // engine rejects (syntax or binding) -> no reach signal
                    }
                    r.accepted += 1;
                    match duckdb_stmt_production(sql) {
                        Some(production) => {
                            r.engine.insert(production);
                            if parse_with(sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok() {
                                r.squonk.insert(production);
                            }
                        }
                        None => {
                            r.unmapped.insert(sql.to_owned());
                        }
                    }
                }
            }
            r
        };

        let core = measure(&core_schema_groups());
        let tranche2 = measure(&tranche2_schema_groups());
        let combined_engine: BTreeSet<&str> =
            core.engine.union(&tranche2.engine).copied().collect();
        let combined_squonk: BTreeSet<&str> =
            core.squonk.union(&tranche2.squonk).copied().collect();

        let report = |label: &str, reach: &Reach| {
            let unexercised: Vec<&str> = productions.difference(&reach.engine).copied().collect();
            eprintln!(
                "\n=== DuckDB {label} statement-production coverage (DuckDbOracle v1.5.4) ==="
            );
            eprintln!(
                "  engine reach:     {}/{} ({:.1}%)",
                reach.engine.len(),
                productions.len(),
                100.0 * reach.engine.len() as f64 / productions.len() as f64,
            );
            eprintln!(
                "  squonk reach: {}/{} ({:.1}%)",
                reach.squonk.len(),
                productions.len(),
                100.0 * reach.squonk.len() as f64 / productions.len() as f64,
            );
            eprintln!(
                "  accept-surface statements engine-accepted: {}",
                reach.accepted,
            );
            eprintln!("  engine-exercised: {:?}", reach.engine);
            eprintln!("  UNEXERCISED:      {unexercised:?}");
            if !reach.unmapped.is_empty() {
                eprintln!(
                    "  UNMAPPED (engine-accepted, unclassified): {}",
                    reach.unmapped.len(),
                );
                for sql in &reach.unmapped {
                    eprintln!("    {sql:?}");
                }
            }
        };
        report("core-tranche", &core);
        report("tranche-2", &tranche2);

        let combined_unexercised: Vec<&str> =
            productions.difference(&combined_engine).copied().collect();
        eprintln!("\n=== DuckDB COMBINED (both tranches) statement-production coverage ===");
        eprintln!(
            "  engine reach:     {}/{} ({:.1}%)",
            combined_engine.len(),
            productions.len(),
            100.0 * combined_engine.len() as f64 / productions.len() as f64,
        );
        eprintln!(
            "  squonk reach: {}/{}",
            combined_squonk.len(),
            productions.len(),
        );
        eprintln!("  UNCOVERED FAMILIES (neither tranche): {combined_unexercised:?}");
        let combined_squonk_unexercised: Vec<&str> =
            productions.difference(&combined_squonk).copied().collect();
        eprintln!("  squonk UNEXERCISED (distinct):    {combined_squonk_unexercised:?}");

        // Every engine-accepted accept-surface statement must classify to a top-level
        // production; a residual here is a classifier hole to close, not a measurement.
        assert!(
            core.unmapped.is_empty() && tranche2.unmapped.is_empty(),
            "engine-accepted accept-surface statements were left unmapped; extend duckdb_stmt_production",
        );

        // Measured pins (baseline, not a gate). The unexercised list and its exercised
        // complement re-baseline together from a fresh oracle run.
        let core_unexercised: Vec<&str> = productions.difference(&core.engine).copied().collect();
        assert_eq!(
            core_unexercised.as_slice(),
            DUCKDB_CORE_UNEXERCISED_PRODUCTIONS,
            "core-tranche production reach drifted; review both halves before re-baselining",
        );
        let tranche2_unexercised: Vec<&str> =
            productions.difference(&tranche2.engine).copied().collect();
        assert_eq!(
            tranche2_unexercised.as_slice(),
            DUCKDB_TRANCHE2_UNEXERCISED_PRODUCTIONS,
            "tranche-2 production reach drifted; review both halves before re-baselining",
        );
        assert_eq!(
            combined_unexercised.as_slice(),
            DUCKDB_COMBINED_UNEXERCISED_PRODUCTIONS,
            "combined production reach drifted; review both halves before re-baselining",
        );
        // The squonk-reach column is pinned separately from engine reach; it is a
        // superset of the engine-uncovered set (a production the engine reaches is not one
        // squonk necessarily parses).
        assert_eq!(
            combined_squonk_unexercised.as_slice(),
            DUCKDB_COMBINED_SQUONK_UNEXERCISED_PRODUCTIONS,
            "combined squonk reach drifted; re-baseline DUCKDB_COMBINED_SQUONK_UNEXERCISED_PRODUCTIONS",
        );
    }

    /// Authored engine-reach probes for the productions no vendored tranche reaches, each
    /// with the minimal setup DDL its `PrepareBind` oracle needs (DuckDB's `prepare` binds,
    /// unlike PostgreSQL's parse-only `pg_query`, so a probe naming a missing object would
    /// bind-reject). `(production, setup_ddl, probe)`; the probe closes the corpus's
    /// negative space by proving the engine reaches the production directly.
    const DUCKDB_UNCOVERED_STMT_PROBES: &[(&str, &str, &str)] = &[
        (
            "AlterObjectSchemaStmt",
            "CREATE TABLE probe_t(a INTEGER); CREATE SCHEMA probe_s",
            "ALTER TABLE probe_t SET SCHEMA probe_s",
        ),
        ("AnalyzeStmt", "", "ANALYZE"),
        (
            "CommentOnStmt",
            "CREATE TABLE probe_t(a INTEGER)",
            "COMMENT ON TABLE probe_t IS 'note'",
        ),
        (
            "CreateSecretStmt",
            "",
            "CREATE SECRET probe_secret (TYPE http)",
        ),
        ("DropSecretStmt", "", "DROP SECRET IF EXISTS probe_secret"),
        (
            "ExecuteStmt",
            "PREPARE probe_p AS SELECT 1",
            "EXECUTE probe_p",
        ),
        (
            "ExportStmt",
            "CREATE TABLE probe_t(a INTEGER)",
            "EXPORT DATABASE 'probe_export'",
        ),
        ("ImportStmt", "", "IMPORT DATABASE 'probe_import'"),
        ("UpdateExtensionsStmt", "", "UPDATE EXTENSIONS"),
        ("VacuumStmt", "", "VACUUM"),
    ];

    /// Uncovered productions the `PrepareBind` engine accepts once minimally provisioned —
    /// the corpus gap is a vendoring gap, not an engine limit. Measured (8/10); distinct
    /// from squonk acceptance.
    const DUCKDB_PROBE_ENGINE_REACHED: &[&str] = &[
        "AnalyzeStmt",
        "CommentOnStmt",
        "CreateSecretStmt",
        "DropSecretStmt",
        "ExecuteStmt",
        "ExportStmt",
        "UpdateExtensionsStmt",
        "VacuumStmt",
    ];
    /// Uncovered productions the `PrepareBind` oracle still rejects even provisioned — the
    /// parser reaches them (proven parse-only via `json_serialize_sql`) but binding needs
    /// execution context this prepare-only harness cannot supply (`ALTER … SET SCHEMA` is
    /// binder-unimplemented in 1.5.4; `IMPORT DATABASE` needs a real exported directory).
    /// Analogous to PostgreSQL's grammar-present, engine-unimplemented `CreateAssertionStmt`.
    /// Measured (2/10).
    const DUCKDB_PROBE_ENGINE_CONDITIONAL: &[&str] = &["AlterObjectSchemaStmt", "ImportStmt"];
    /// Uncovered productions squonk accepts (its own reach column — engine reach does
    /// not imply squonk parses the DDL). Measured (10/10): the secrets DROP
    /// (`create_secret` gate), `EXPORT DATABASE` / `IMPORT DATABASE`
    /// (`export_import_database` gate), `ALTER … SET SCHEMA`
    /// (`alter_object_set_schema` gate — binder-unimplemented but parse-reachable),
    /// `UPDATE EXTENSIONS` (`update_extensions` gate), and the `ANALYZE` / `VACUUM`
    /// maintenance statements (`analyze` / `vacuum_analyze` gates) now parse — the full
    /// measured uncovered set.
    const DUCKDB_PROBE_SQUONK_ACCEPTS: &[&str] = &[
        "AlterObjectSchemaStmt",
        "AnalyzeStmt",
        "CommentOnStmt",
        "CreateSecretStmt",
        "DropSecretStmt",
        "ExecuteStmt",
        "ExportStmt",
        "ImportStmt",
        "UpdateExtensionsStmt",
        "VacuumStmt",
    ];

    #[test]
    fn duckdb_uncovered_statement_productions_have_permanent_oracle_probes() {
        if let Err(OracleUnavailable(reason)) = DuckDbOracle::new() {
            eprintln!("skipping DuckDB uncovered-production probes (oracle unavailable): {reason}");
            return;
        }
        let combined_unexercised: BTreeSet<&str> = DUCKDB_COMBINED_UNEXERCISED_PRODUCTIONS
            .iter()
            .copied()
            .collect();

        let mut engine_reached: BTreeSet<&str> = BTreeSet::new();
        let mut engine_conditional: BTreeSet<&str> = BTreeSet::new();
        let mut squonk_accepts: BTreeSet<&str> = BTreeSet::new();
        let mut parser_unreachable: BTreeSet<&str> = BTreeSet::new();
        let mut probed: BTreeSet<&str> = BTreeSet::new();
        // Parse-only reach: `json_serialize_sql` parses + transforms without binding, so a
        // production that binds-rejects (missing object) still proves parser-reachable here.
        let json_probe = Connection::open_in_memory().expect("json probe connection");
        let mut json_available = true;

        eprintln!("\n=== DuckDB uncovered-production authored probes (DuckDbOracle v1.5.4) ===");
        for &(production, setup, query) in DUCKDB_UNCOVERED_STMT_PROBES {
            assert!(
                combined_unexercised.contains(production),
                "probe production {production:?} is not in the measured uncovered set",
            );
            assert_eq!(
                duckdb_stmt_production(query),
                Some(production),
                "probe query {query:?} classifies to the wrong production",
            );
            assert!(
                probed.insert(production),
                "duplicate probe production {production}"
            );

            let oracle = if setup.is_empty() {
                DuckDbOracle::new()
            } else {
                DuckDbOracle::with_schema(setup)
            };
            let engine_accepts = match &oracle {
                Ok(o) => o.verdict(query).map(|v| v.accepts()).unwrap_or(false),
                Err(OracleUnavailable(_)) => false, // provisioning failed -> not bind-reached
            };
            let ours = parse_with(query, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok();
            let parses = match json_tree(&json_probe, query) {
                Ok(Some(_)) => true,
                Ok(None) => {
                    json_available = false;
                    true // json extension absent -> cannot disprove parse-reach; do not flag
                }
                Err(_) => false,
            };
            if !parses {
                parser_unreachable.insert(production);
            }
            // A bind reject (parser reaches it, binder does not) is characterized by the
            // oracle's own message, captured off a same-setup connection.
            let reason = if engine_accepts {
                String::new()
            } else {
                let probe = Connection::open_in_memory().expect("reason probe connection");
                if !setup.is_empty() {
                    let _ = probe.execute_batch(setup);
                }
                probe.prepare_err(query).unwrap_or_default()
            };
            eprintln!(
                "  {production:22} engine={engine_accepts:<5} parses={parses:<5} squonk={ours:<5} {query:?}{}",
                if reason.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n      bind-reject: {}",
                        reason.lines().next().unwrap_or(&reason)
                    )
                },
            );
            if engine_accepts {
                engine_reached.insert(production);
            } else {
                engine_conditional.insert(production);
            }
            if ours {
                squonk_accepts.insert(production);
            }
        }

        // Every authored probe must be parser-reachable — the denominator names no phantom
        // production (skipped only if the json extension is unavailable).
        if json_available {
            assert!(
                parser_unreachable.is_empty(),
                "authored probes unreachable by the DuckDB parser (json_serialize_sql): {parser_unreachable:?}",
            );
        }

        assert_eq!(
            probed, combined_unexercised,
            "authored probes must cover exactly the measured uncovered productions",
        );

        let corpus_reach = duckdb_stmt_productions().len() - combined_unexercised.len();
        let combined_engine_reach = corpus_reach + engine_reached.len();
        eprintln!("  engine-reached by probe:  {engine_reached:?}");
        eprintln!("  engine-conditional:       {engine_conditional:?}");
        eprintln!("  squonk accepts:       {squonk_accepts:?}");
        eprintln!(
            "  TOTAL engine reach (corpus {corpus_reach} + probes {}): {combined_engine_reach}/{}",
            engine_reached.len(),
            duckdb_stmt_productions().len(),
        );

        // Measured pins (baseline). The reached / conditional split partitions the probes.
        let reached_pin: BTreeSet<&str> = DUCKDB_PROBE_ENGINE_REACHED.iter().copied().collect();
        let conditional_pin: BTreeSet<&str> =
            DUCKDB_PROBE_ENGINE_CONDITIONAL.iter().copied().collect();
        let squonk_pin: BTreeSet<&str> = DUCKDB_PROBE_SQUONK_ACCEPTS.iter().copied().collect();
        assert_eq!(
            engine_reached, reached_pin,
            "probe engine-reach drifted; re-baseline DUCKDB_PROBE_ENGINE_REACHED",
        );
        assert_eq!(
            engine_conditional, conditional_pin,
            "probe engine-conditional set drifted; re-baseline DUCKDB_PROBE_ENGINE_CONDITIONAL",
        );
        assert_eq!(
            squonk_accepts, squonk_pin,
            "probe squonk acceptance drifted; re-baseline DUCKDB_PROBE_SQUONK_ACCEPTS",
        );
        assert_eq!(
            engine_reached.len() + engine_conditional.len(),
            combined_unexercised.len(),
            "probe reached/conditional split must partition the uncovered productions",
        );
    }

    #[test]
    fn json_serialize_sql_sample_sizes_structural_child() {
        let oracle = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping json_serialize_sql sample (oracle unavailable): {reason}");
                return;
            }
        };
        let probe = Connection::open_in_memory().expect("json probe connection");

        eprintln!("\n=== json_serialize_sql sample (structural-oracle sizing) ===");
        let mut json_dumps = 0usize;
        let mut json_available = true;
        for entry in entries() {
            if json_dumps >= 4 {
                break;
            }
            let trimmed = entry.sql.trim_start().to_ascii_uppercase();
            let is_select = trimmed.starts_with("SELECT") || trimmed.starts_with("FROM");
            let both_accept = oracle
                .verdict(entry.sql)
                .map(|v| v.accepts())
                .unwrap_or(false)
                && parse_with(entry.sql, squonk::ParseConfig::new(CORPUS_DIALECT)).is_ok();
            if !(is_select && both_accept) {
                continue;
            }
            match json_tree(&probe, entry.sql) {
                Ok(Some(json)) => {
                    let shown: String = json.chars().take(900).collect();
                    eprintln!("  SQL: {}", entry.sql);
                    eprintln!("  JSON: {shown}\n");
                    json_dumps += 1;
                }
                Ok(None) => {
                    json_available = false;
                    eprintln!(
                        "  json_serialize_sql unavailable (json extension not loaded); skipping sample"
                    );
                    break;
                }
                Err(err) => panic!("json_serialize_sql probe failed unexpectedly: {err}"),
            }
        }
        // The structural lever is the whole reason DuckDB can reach PG-class parity;
        // prove it is live in this environment (or explicitly recorded unavailable).
        if json_available {
            assert!(
                json_dumps > 0,
                "expected at least one json_serialize_sql tree from a both-accept SELECT",
            );
        }
    }

    // --- Oracle-mediated structural lane (conformance-mediated-structural-lane-duckdb) --

    /// The comparable both-accept subset size for the mediated lane — statements our
    /// [`CORPUS_DIALECT`] preset parses AND `json_serialize_sql` serializes (the latter
    /// requires DuckDB to parse them AND them to be a SELECT, since `json_serialize_sql`
    /// refuses every non-SELECT). This is the denominator the lane compares over; pinned
    /// as an anti-vanishing guard like the per-corpus counts (the DuckDB analogue of PG's
    /// `PG_MEDIATED_BOTH_ACCEPT_PINNED`).
    // +1 vs the prior baseline: `duckdb-geometry-type-and-overlaps-operator` made a
    // `&&`-overlap SELECT parse on our side, so it joins the both-accept structural subset.
    // +1 more: `duckdb-pg-operator-spelling-under-acceptance` armed the general symbolic
    // operator surface, so one further generic-`Op` SELECT now parses and mediates.
    const DUCKDB_MEDIATED_BOTH_ACCEPT_PINNED: usize = 1052;

    /// Known mediated structural divergences, knowingly tolerated with a ticket — the same
    /// exact-SQL, staleness-enforced ledger discipline as [`DUCKDB_DIVERGENCE_ALLOWLIST`]
    /// and `pg::PG_MEDIATED_DIVERGENCE_ALLOWLIST`. A mediated mismatch implicates the
    /// parser OR the renderer, so an entry parked here (rather than fixed) must record
    /// which. Currently empty: measured 0 mismatch / 0 unparseable over the both-accept
    /// subset.
    const DUCKDB_MEDIATED_DIVERGENCE_ALLOWLIST: &[DivergenceEntry] = &[];

    /// Whether `sql` is named in the mediated divergence ledger.
    fn mediated_allowlisted(sql: &str) -> bool {
        DUCKDB_MEDIATED_DIVERGENCE_ALLOWLIST
            .iter()
            .any(|entry| entry.sql == sql)
    }

    /// The `json_serialize_sql`-mediated structural lane over the DuckDB both-accept corpus
    /// (conformance-mediated-structural-lane-duckdb): for every SELECT our DuckDb preset
    /// and DuckDB both accept, our canonical render must serialize to the SAME
    /// `json_serialize_sql` tree as the original (`query_location` stripped). A NEW
    /// mismatch (or an unparseable render) drifts the pin and fails.
    ///
    /// This is the COMMODITY structural check; the hand-written
    /// [`DuckDbStructuralOracle`](crate::duckdb_structural::DuckDbStructuralOracle)
    /// (neutral-shape mapper) stays as the PREMIUM tier — it encodes the sensitivity to the
    /// serializer-erased distinctions this lane is blind to (the `RECURSIVE` keyword, the
    /// comma/cross-join split, the CTE materialization hint, literal-vs-explicit-
    /// constructor spelling, and the folded operator/boolean/count_star/lambda forms). A
    /// green here is NOT full structural coverage.
    #[test]
    fn duckdb_corpus_mediated_structural_lane_holds() {
        let oracle = match DuckDbMediatedStructuralOracle::new() {
            Ok(oracle) => oracle,
            // Unreachable engine is an infrastructure skip, never a failure (ADR-0015).
            Err(OracleUnavailable(reason)) => {
                eprintln!(
                    "skipping DuckDB mediated structural lane (oracle unavailable): {reason}"
                );
                return;
            }
        };
        // `json_serialize_sql` needs the json extension; probe once so an environment
        // without it is a clean infra skip (never a false green), mirroring the sweep's
        // json-unavailable guard. `SELECT 1` is a serializable SELECT that must Match.
        match oracle.verdict("SELECT 1") {
            Ok(DuckDbMediatedVerdict::Match) => {}
            Ok(other) => {
                panic!("json_serialize_sql sanity probe on `SELECT 1` did not Match: {other:?}")
            }
            Err(OracleUnavailable(reason)) => {
                eprintln!(
                    "skipping DuckDB mediated structural lane (json_serialize_sql unavailable): {reason}"
                );
                return;
            }
        }

        // Ledger staleness (mirrors the accept/reject and PG-mediated ledgers): every entry
        // names a real ticket and must STILL diverge, so a fixed or fallen-out entry fails
        // until deleted.
        assert_entries_are_ticketed(DUCKDB_MEDIATED_DIVERGENCE_ALLOWLIST);
        assert_entries_still_diverge(DUCKDB_MEDIATED_DIVERGENCE_ALLOWLIST, |entry| {
            matches!(
                oracle.verdict(entry.sql),
                Ok(DuckDbMediatedVerdict::Mismatch { .. }
                    | DuckDbMediatedVerdict::RenderUnparseable(_)),
            )
        });

        let mut both_accept = 0usize;
        let mut matched = 0usize;
        let mut mismatch = 0usize;
        let mut unparseable = 0usize;
        // Non-allowlisted divergences (mismatch or unparseable) — the failing set.
        let mut untriaged: Vec<(&str, &str, DuckDbMediatedVerdict)> = Vec::new();

        for entry in entries() {
            // A statement outside the comparable subset (our parser rejects it, or DuckDB
            // cannot serialize it — a non-SELECT or a DuckDB syntax reject) is a Skip: the
            // accept/reject sweep above owns those.
            let verdict = oracle
                .verdict(entry.sql)
                .expect("json_serialize_sql transport (probed live above)");
            match verdict {
                DuckDbMediatedVerdict::Skip(_) => continue,
                DuckDbMediatedVerdict::Match => {
                    both_accept += 1;
                    matched += 1;
                }
                verdict @ DuckDbMediatedVerdict::Mismatch { .. } => {
                    both_accept += 1;
                    mismatch += 1;
                    if !mediated_allowlisted(entry.sql) {
                        untriaged.push((entry.corpus, entry.sql, verdict));
                    }
                }
                verdict @ DuckDbMediatedVerdict::RenderUnparseable(_) => {
                    both_accept += 1;
                    unparseable += 1;
                    if !mediated_allowlisted(entry.sql) {
                        untriaged.push((entry.corpus, entry.sql, verdict));
                    }
                }
            }
        }

        // Printed always, so a green run documents the distribution and a drift shows fresh
        // counts to triage (mirrors the accept/reject sweep's always-print block).
        eprintln!(
            "DuckDB json_serialize_sql-mediated structural lane over the both-accept corpus:\n  \
             both-accept {both_accept}  match {matched}  mismatch {mismatch}  \
             unparseable {unparseable}  ({} untriaged)",
            untriaged.len(),
        );

        assert_eq!(
            both_accept, DUCKDB_MEDIATED_BOTH_ACCEPT_PINNED,
            "DuckDB both-accept subset size for the mediated lane changed \
             ({both_accept} vs pinned {DUCKDB_MEDIATED_BOTH_ACCEPT_PINNED}); if a \
             corpus/parser change is intentional, re-measure and update \
             DUCKDB_MEDIATED_BOTH_ACCEPT_PINNED",
        );

        if !untriaged.is_empty() {
            eprintln!(
                "\nUNTRIAGED mediated structural divergences ({}):",
                untriaged.len()
            );
            for (corpus, sql, verdict) in &untriaged {
                eprintln!("  [{corpus}] {sql:?}\n    {verdict:?}");
            }
        }
        assert!(
            untriaged.is_empty(),
            "{} untriaged DuckDB mediated structural divergence(s): our canonical render \
             serializes to a different json_serialize_sql tree than the original (or DuckDB \
             rejects our render). A mismatch implicates the PARSER (wrong tree) OR the \
             RENDERER (wrong canonical form) — triage against the ADR-0014 render round-trip \
             gates (`assert_roundtrips` / `assert_roundtrips_parenthesized`, and the \
             `corpus_roundtrip` differential reparse) to localize which, then FIX it or add \
             an exact-SQL, ticketed entry to DUCKDB_MEDIATED_DIVERGENCE_ALLOWLIST",
            untriaged.len(),
        );
    }

    // --- Setup-driver reclassification pins ------------------------------------------
    //
    // Baselined against DuckDB 1.5.4 + the vendored signature-surface corpus under the
    // fitted `DuckDb` `CORPUS_DIALECT`, with the per-file setup driver. These pins guard
    // the provisioned subset and residual classes produced by the setup driver. They are
    // measurement baselines, not zero gates: a drift fails loudly so the inventory is
    // re-read and re-baselined.
    const NEWLY_COMPARABLE_PIN: usize = 600;
    const COVERAGE_GAP_PIN: usize = 0;
    // +1 over_accept_binding / -1 agree_reject_binding: `duckdb-tranche2-small-gaps`
    // added parser support for one setup-driver statement that DuckDB syntax accepts and
    // rejects only after binding.
    // +1 more: `duckdb-pg-operator-spelling-under-acceptance` armed the general symbolic
    // operator surface for DuckDb, so one further statement that DuckDB parse-accepts and
    // bind-rejects (a generic `Op` run) now parses on our side too.
    const OVER_ACCEPT_BINDING_PIN: usize = 277;
    // -1 vs the prior baseline: `duckdb-geometry-type-and-overlaps-operator` made the
    // malformed-CRS `GEOMETRY('GEOGCRS[…')` query parse on our side (DuckDB's PARSER accepts
    // it too — the "Invalid Input" is a CRS-validation reject at bind, a non-syntax
    // over-accept), so it left the both-reject class for over_accept_other.
    // -1: `duckdb-tranche2-small-gaps` moved one syntax-accepted/bind-rejected statement
    // to over_accept_binding.
    // -1 more: `duckdb-pg-operator-spelling-under-acceptance` moved one further
    // parse-accept/bind-reject statement (a generic `Op` run) out of the both-reject class.
    const AGREE_REJECT_BINDING_PIN: usize = 4;
    const RESIDUAL_PIN: usize = 308;
    const UNCLASSIFIED_GAP_PIN: usize = 0;

    // ------------------------------------------------------------------
    // Flag-aware generative differential (oracle-parity-duckdb)
    // ------------------------------------------------------------------

    use crate::properties::{DUCKDB_FEATURE_PROBES, DUCKDB_FEATURE_SEEDS, arb_feature_statement};
    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
    use squonk::Dialect;
    use squonk::dialect::DuckDb;

    const DUCKDB_FEATURE_SCHEMA: &str = "CREATE TABLE t (a INTEGER PRIMARY KEY, b TEXT, c INTEGER)";

    fn duckdb_generative_divergence(sql: &str, oracle: &DuckDbOracle) -> Option<String> {
        let ours = squonk::parse_with(sql, squonk::ParseConfig::new(DuckDb)).is_ok();
        let theirs = match oracle.verdict(sql) {
            Ok(OracleVerdict::Accept) => true,
            Ok(OracleVerdict::Reject) => false,
            Err(_) => return None,
        };
        if ours == theirs {
            return None;
        }
        Some(if ours {
            format!("over-accept: we accept, DuckDB rejects: {sql:?}")
        } else {
            format!("coverage gap: DuckDB accepts, we reject: {sql:?}")
        })
    }

    #[test]
    fn duckdb_feature_generative_differential_replays_committed_seeds() {
        let Ok(oracle) = DuckDbOracle::with_schema(DUCKDB_FEATURE_SCHEMA) else {
            eprintln!("skip duckdb feature generative seeds: oracle unavailable");
            return;
        };
        let divergences: Vec<String> = DUCKDB_FEATURE_SEEDS
            .iter()
            .filter_map(|&sql| duckdb_generative_divergence(sql, &oracle))
            .collect();
        assert!(
            divergences.is_empty(),
            "flag-aware DuckDB generative differential found {} un-ledgered divergence(s) over seeds:\n  {}",
            divergences.len(),
            divergences.join("\n  "),
        );
    }

    #[test]
    fn duckdb_feature_generative_differential_explores_flag_aware_surface() {
        let Ok(oracle) = DuckDbOracle::with_schema(DUCKDB_FEATURE_SCHEMA) else {
            eprintln!("skip duckdb feature generative explore: oracle unavailable");
            return;
        };
        let mut runner = TestRunner::new_with_rng(
            Config {
                cases: 256,
                ..Config::default()
            },
            TestRng::from_seed(RngAlgorithm::ChaCha, &[0xD8; 32]),
        );
        let strategy = arb_feature_statement(DuckDb.features(), DUCKDB_FEATURE_PROBES);
        for _ in 0..256 {
            let tree = strategy.new_tree(&mut runner).expect("strategy ok");
            let (_family, sql) = tree.current();
            if let Some(detail) = duckdb_generative_divergence(&sql, &oracle) {
                panic!("flag-aware DuckDB generative differential: {detail}");
            }
        }
    }
}
