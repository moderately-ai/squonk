// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The multi-claimant statement-head ledger — one enumerable record of every leading
//! keyword two or more features grammar-claim, and how the permissive `LENIENT` union
//! resolves each.
//!
//! # Why this exists
//!
//! The three self-consistency registries in the sibling `conflict` module catch the
//! contentions that have *no* defined resolution (`LexicalConflict` for a shared tokenizer
//! trigger, `FeatureDependencyViolation` for a base-flag gap, `GrammarConflict` for an
//! undefined parser-position shadow). Their contract is the mirror image of this table's:
//! a multi-claimant head that a preset *does* union with a documented, deterministic
//! resolution — a lookahead split, a fixed dispatch precedence, or a deliberate one-reading
//! exclusion — is conflict-*free* and gets **no** registry variant. Those resolutions used
//! to live only in scattered per-arm code comments across the parser and in the `LENIENT`
//! preset's field docs, so no reader could enumerate them or test them against each other.
//! This ledger is that missing artifact: the single enumerable source of every contested
//! statement head and the reading `LENIENT` picks.
//!
//! It is dialect **data** — a `const` slice of `&'static str` pointers, not runtime
//! machinery. Enforcement (the union-property invariant, a parse-entry assert, an xtask
//! lint) is other tickets' work; this module ships the data and one consistency test that
//! keeps the `LENIENT` exclusion columns honest against `FeatureSet::LENIENT`.
//!
//! # The exception source for the Lenient union property
//!
//! `oracle-parity-lenient` specifies Lenient's union property — *any statement
//! accepted by any enabled preset must be accepted by Lenient, except where a deliberate
//! exclusion is documented*. The [`OneReadingExclusion`](HeadResolution::OneReadingExclusion)
//! and [`Route`](HeadResolution::Route) entries here are the sanctioned statement-head
//! exclusions that property consumes: a new head-level exclusion requires a row here, not
//! an allowlist line. (Lexical-trigger sacrifices — the `"` / `[` / `$` conflicts — are the
//! separate concern of `LexicalConflict` and the `LENIENT` module's rules 1-8.)

/// How the permissive union resolves a statement head that two or more features claim.
///
/// MECE over the multi-claimant heads: a head is a clean lookahead split
/// ([`MeceLookahead`](Self::MeceLookahead)), a precedence-resolved overlap
/// ([`DispatchOrderUnion`](Self::DispatchOrderUnion)), a forgone rival reading
/// ([`OneReadingExclusion`](Self::OneReadingExclusion)), or a forgone wholesale alternative
/// grammar ([`Route`](Self::Route)). The first two keep *both* claimants enabled under
/// `LENIENT`; the last two turn one claimant *off* — the two exclusion kinds, whose forgone
/// flags are the union property's sanctioned head-level exceptions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadResolution {
    /// The claimants share a leading keyword but split cleanly on a lookahead token (a
    /// second keyword or a follow token), so no input is ambiguous and the union keeps
    /// **both** readings. `LOAD DATA` vs `LOAD <extension>` (split on the second word) is
    /// the archetype.
    MeceLookahead,
    /// The claimants overlap on some input with no lookahead to separate them, but a fixed
    /// dispatch order / precedence picks one reading for the overlap while still admitting
    /// the other's distinct tail — so the union keeps **both** grammars, one of which wins
    /// the shared prefix. `VACUUM <name>` (the DuckDB qualified-name tail takes precedence,
    /// the SQLite `INTO` tail still admitted after) is the archetype.
    DispatchOrderUnion,
    /// The claimants genuinely collide on some input with no lookahead or precedence able
    /// to keep both readable, so the union turns one claimant **off** and forgoes that
    /// reading. `DO` (MySQL `DO <expr-list>` collides with the PostgreSQL code block on
    /// `DO 'x'`) is the archetype.
    OneReadingExclusion,
    /// One claimant is a wholesale alternative grammar — a *route* that structurally
    /// replaces rather than extends the other — so the union forgoes the route to keep the
    /// richer / more permissive grammar. The MySQL account-based `GRANT` route displacing
    /// the extended standard/PostgreSQL grant grammar is the archetype. (A route is an
    /// exclusion by whole-grammar rather than by a single overlapping input; both this and
    /// [`OneReadingExclusion`](Self::OneReadingExclusion) turn a claimant off under `LENIENT`.)
    Route,
}

/// One contested statement head: the leading keyword(s), the features that claim it, how
/// the union resolves them, the reading `LENIENT` picks, and where the resolution is
/// implemented.
///
/// A row is dialect data with `&'static str` columns; the parser-code `doc` pointer is
/// prose (the parser lives in the `squonk` crate, which this crate cannot intra-doc
/// link into), while the flag names in `claimants` / `lenient_excludes` match the
/// `FeatureSet` fields verbatim so the consistency test can cross-check them.
#[derive(Clone, Copy, Debug)]
pub struct MultiClaimantHead {
    /// The leading keyword(s) that dispatch this head (e.g. `["DO"]`, `["ALTER", "VIEW"]`).
    pub heads: &'static [&'static str],
    /// The `FeatureSet` flag names whose grammars claim this head, spelled exactly as the
    /// struct fields (e.g. `"do_statement"`, `"do_expression_list"`).
    pub claimants: &'static [&'static str],
    /// How the union resolves the contention.
    pub resolution: HeadResolution,
    /// The subset of `claimants` that `FeatureSet::LENIENT` turns **off** — empty for the
    /// union kinds ([`MeceLookahead`](HeadResolution::MeceLookahead) /
    /// [`DispatchOrderUnion`](HeadResolution::DispatchOrderUnion), which keep every
    /// claimant), non-empty for the exclusion kinds
    /// ([`OneReadingExclusion`](HeadResolution::OneReadingExclusion) /
    /// [`Route`](HeadResolution::Route)). This is the union property's exception column.
    pub lenient_excludes: &'static [&'static str],
    /// The reading `LENIENT` accepts for this head, in prose.
    pub lenient_reading: &'static str,
    /// Where the resolution is implemented — a prose pointer to the parser function(s) that
    /// carry the per-arm comment the ledger summarizes.
    pub doc: &'static str,
}

/// Every statement head two or more features grammar-claim, with the permissive union's
/// resolution for each — the enumerable replacement for the scattered per-arm comments.
///
/// Verified entry-by-entry against the cited parser code. The exclusion
/// rows ([`OneReadingExclusion`](HeadResolution::OneReadingExclusion) /
/// [`Route`](HeadResolution::Route)) are exactly the head-level flags `FeatureSet::LENIENT`
/// turns off; the consistency test (`lenient_exclusions_match_the_ledger`) proves the
/// correspondence in both directions.
pub const MULTI_CLAIMANT_STATEMENT_HEADS: &[MultiClaimantHead] = &[
    MultiClaimantHead {
        heads: &["DO"],
        claimants: &["do_statement", "do_expression_list"],
        resolution: HeadResolution::OneReadingExclusion,
        lenient_excludes: &["do_expression_list"],
        lenient_reading: "PostgreSQL's `DO [LANGUAGE <lang>] $$…$$` anonymous code block; \
            MySQL's `DO <expr-list>` collides on inputs like `DO 'x'` and cannot express the \
            `LANGUAGE` clause, so it is forgone.",
        doc: "squonk/src/parser/query.rs `DO` dispatch arms; \
            util.rs `parse_do_statement` / `parse_do_expressions_statement`.",
    },
    MultiClaimantHead {
        heads: &["PREPARE", "EXECUTE", "DEALLOCATE"],
        claimants: &["prepared_statements", "prepared_statements_from"],
        resolution: HeadResolution::OneReadingExclusion,
        lenient_excludes: &["prepared_statements_from"],
        lenient_reading: "DuckDB's typed-`AS` lifecycle (`PREPARE p AS <stmt>`, \
            `EXECUTE name(<args>)`, `DEALLOCATE`); MySQL's `PREPARE … FROM` / \
            `EXECUTE … USING @var` is a different grammar on the same three keywords with no \
            positional-argument spelling, so it is forgone.",
        doc: "squonk/src/parser/query.rs `PREPARE`/`EXECUTE`/`DEALLOCATE` dispatch arms; \
            util.rs `parse_prepare_statement` / `parse_prepare_from_statement` / \
            `parse_deallocate_statement`.",
    },
    MultiClaimantHead {
        heads: &["GRANT", "REVOKE"],
        claimants: &[
            "access_control_account_grants",
            "access_control_extended_objects",
        ],
        resolution: HeadResolution::Route,
        lenient_excludes: &["access_control_account_grants"],
        lenient_reading: "the extended standard/PostgreSQL object-and-role grammar (schema \
            objects, `GRANTED BY`, `CASCADE`, routine signatures, the `{GRANT|ADMIN} OPTION \
            FOR` REVOKE prefix); MySQL's account-based grant route structurally replaces (does \
            not extend) it, so the route is forgone.",
        doc: "squonk/src/parser/dcl.rs `parse_grant_kind` / `parse_revoke_kind` \
            (the `access_control_account_grants` route branches to \
            `parse_account_grant` / `parse_account_revoke`).",
    },
    MultiClaimantHead {
        heads: &["SET"],
        claimants: &["variable_assignment"],
        resolution: HeadResolution::Route,
        lenient_excludes: &["variable_assignment"],
        lenient_reading: "the generic `SET [SESSION|LOCAL] <name> {=|TO} <value>` session \
            grammar; MySQL's `variable_assignment` comma-list route (heterogeneous \
            assignments over full expressions, plus `:=`) structurally replaces it and relies \
            on the `@name`-as-user-variable read that `LENIENT` does not take, so it is forgone.",
        doc: "squonk/src/parser/dcl.rs `parse_set` (the `variable_assignment` route \
            branches to `parse_mysql_set_variables`).",
    },
    MultiClaimantHead {
        heads: &["LOCK", "UNLOCK"],
        claimants: &["lock_tables", "lock_instance"],
        resolution: HeadResolution::MeceLookahead,
        lenient_excludes: &[],
        lenient_reading: "both MySQL grammars — `LOCK/UNLOCK {TABLES|TABLE}` and `LOCK \
            INSTANCE FOR BACKUP` / `UNLOCK INSTANCE` — kept, split on the second word. The \
            (unimplemented) PostgreSQL statement-level mode-list reading of `LOCK` will take \
            its own future gate, owing the same one-reading decision `DO` got; that gate does \
            not exist yet, so nothing is resolved away today.",
        doc: "squonk/src/parser/query.rs `LOCK`/`UNLOCK` dispatch arms \
            (`peek_nth_starts_table_or_tables` vs `INSTANCE` second-word split).",
    },
    MultiClaimantHead {
        heads: &["LOAD"],
        claimants: &["load_data", "load_extension", "key_cache_statements"],
        resolution: HeadResolution::MeceLookahead,
        lenient_excludes: &[],
        lenient_reading: "all three readings kept, split on the follow token: MySQL `LOAD \
            {DATA|XML}` (second word), MySQL `LOAD INDEX INTO CACHE` (the `key_cache_statements` \
            `LOAD INDEX` two-token lookahead), and PostgreSQL/DuckDB `LOAD <extension>` (the \
            fall-through bare `LOAD`).",
        doc: "squonk/src/parser/query.rs `LOAD DATA` / `LOAD INDEX` / `LOAD` dispatch arms \
            (ordered so the two-word forms are checked before the bare `load_extension` arm); \
            util.rs `parse_load_statement`.",
    },
    MultiClaimantHead {
        heads: &["VACUUM"],
        claimants: &["vacuum", "vacuum_analyze"],
        resolution: HeadResolution::DispatchOrderUnion,
        lenient_excludes: &[],
        lenient_reading: "both tails kept; on the overlapping `VACUUM <name>` operand the \
            DuckDB `vacuum_analyze` reading takes precedence (a *qualified* table name via \
            `parse_object_name`) over the SQLite `vacuum` bare-schema `parse_ident`, and the \
            SQLite `INTO <expr>` clause is still admitted afterwards. This is a one-reading \
            precedence on the shared operand, NOT a pure addition.",
        doc: "squonk/src/parser/util.rs `parse_vacuum_statement` (the `duck`-branch \
            precedence on the name operand; see the in-function comment).",
    },
    MultiClaimantHead {
        heads: &["ANALYZE"],
        claimants: &["analyze", "table_maintenance"],
        resolution: HeadResolution::MeceLookahead,
        lenient_excludes: &[],
        lenient_reading: "both kept — MySQL `ANALYZE {TABLE|TABLES} …` (the \
            `table_maintenance` verb family, which always requires the `TABLE`/`TABLES` \
            lookahead) and the SQLite/DuckDB bare `ANALYZE [<table> [(<cols>)]]`; a bare \
            `ANALYZE` falls through to the SQLite/DuckDB reading.",
        doc: "squonk/src/parser/query.rs `table_maintenance` arm placed before the bare \
            `ANALYZE` arm (`peek_starts_table_maintenance` insists on `TABLE`/`TABLES`).",
    },
    MultiClaimantHead {
        heads: &["UPDATE"],
        claimants: &["update_extensions"],
        resolution: HeadResolution::MeceLookahead,
        lenient_excludes: &[],
        lenient_reading: "both kept — DuckDB's `UPDATE EXTENSIONS [(names)]` claims the head \
            only when `EXTENSIONS` is followed by `(` or statement end, otherwise the DML \
            `UPDATE <target> SET …` wins (an `UPDATE extensions SET …` still targets a table \
            named `extensions`).",
        doc: "squonk/src/parser/util.rs `peek_starts_update_extensions` / \
            `parse_update_extensions_statement`.",
    },
    MultiClaimantHead {
        heads: &["ALTER", "VIEW"],
        claimants: &["view_definition_options", "alter_object_set_schema"],
        resolution: HeadResolution::MeceLookahead,
        lenient_excludes: &[],
        lenient_reading: "both kept — MySQL view redefinition (`ALGORITHM`/`SQL SECURITY` \
            prefix, or a `(cols)` / `AS <query>` tail) vs DuckDB's `SET SCHEMA` relocation, \
            split by lookahead on the bare `ALTER VIEW <name>` head (an `IF EXISTS` guard or a \
            `SET SCHEMA` tail routes to the relocation; a `(`/`AS` tail to the redefinition).",
        doc: "squonk/src/parser/ddl.rs `parse_alter` `view_definition_options` block \
            (the `relocates` lookahead split when `alter_object_set_schema` is also on).",
    },
    MultiClaimantHead {
        heads: &["ALTER", "DATABASE", "SCHEMA"],
        claimants: &["alter_database", "alter_database_options"],
        resolution: HeadResolution::MeceLookahead,
        lenient_excludes: &[],
        lenient_reading: "both kept — DuckDB's `SET ALIAS TO` relocation vs MySQL's option \
            list, split by lookahead on the shared `ALTER {DATABASE|SCHEMA}` head (an `IF \
            EXISTS` guard or a `SET` tail after the name is the relocation; an option keyword \
            leading with no name, or a name followed by a non-`SET` tail, is the option list).",
        doc: "squonk/src/parser/ddl.rs `parse_alter_database_head` (the both-on \
            disambiguator).",
    },
    MultiClaimantHead {
        heads: &["DROP"],
        claimants: &["drop_database", "index_drop_on_table"],
        resolution: HeadResolution::OneReadingExclusion,
        lenient_excludes: &["drop_database", "index_drop_on_table"],
        lenient_reading: "the shared name-list drop grammar kept for both sub-heads: `DROP \
            {DATABASE|SCHEMA}` stays the PostgreSQL/DuckDB name-list-plus-`CASCADE` form \
            (MySQL's single-name `drop_database` synonym would displace it), and `DROP INDEX \
            <name>[, …]` stays the bare-name form (MySQL's mandatory-`ON` `index_drop_on_table` \
            would displace it). Both MySQL displacements are forgone.",
        doc: "squonk/src/parser/ddl.rs `parse_drop` dispatch (the `drop_database` and \
            `index_drop_on_table` interceptors before `parse_drop_object_kind`).",
    },
    MultiClaimantHead {
        heads: &["IMPORT"],
        claimants: &["import_table", "export_import_database"],
        resolution: HeadResolution::MeceLookahead,
        lenient_excludes: &[],
        lenient_reading: "both kept — MySQL `IMPORT TABLE FROM …` vs DuckDB `IMPORT DATABASE`, \
            split on the second keyword (`TABLE` vs `DATABASE`), the `import_table` arm checked \
            before the `export_import_database` arm.",
        doc: "squonk/src/parser/query.rs `IMPORT TABLE` / `IMPORT` dispatch arms.",
    },
    MultiClaimantHead {
        heads: &["CACHE", "LOAD INDEX"],
        claimants: &["key_cache_statements"],
        resolution: HeadResolution::MeceLookahead,
        lenient_excludes: &[],
        lenient_reading: "kept — MySQL's `CACHE INDEX` (an uncontended leading `CACHE`) and \
            `LOAD INDEX INTO CACHE` key-cache pair; the `LOAD INDEX` two-token lookahead keeps \
            the second head MECE against the `LOAD <extension>` statement (see the `LOAD` row).",
        doc: "squonk/src/parser/query.rs `CACHE` / `LOAD INDEX` dispatch arms; \
            util.rs `parse_cache_index_statement` / `parse_load_index_statement`.",
    },
];

#[cfg(test)]
mod tests {
    use super::super::FeatureSet;
    use super::*;

    /// One statement-head claimant gate, valued across the five core presets.
    ///
    /// `get` reads the *live* preset (all struct spreads resolved), so an inherited value is
    /// measured through the spread, not mirrored from a base const — the whole point of the
    /// value pin below.
    struct HeadGateValueRow {
        /// A `FeatureSet` sub-flag that appears as a `claimants` entry in the ledger.
        flag: &'static str,
        /// Reads the flag from a resolved preset.
        get: fn(&FeatureSet) -> bool,
        /// The pinned value in each preset, index-aligned with `CORE_PRESETS`.
        expected: [bool; 5],
    }

    const T: bool = true;
    const F: bool = false;

    /// The five presets the value pin ranges over, in the column order the `expected`
    /// arrays use.
    const CORE_PRESETS: [(&str, FeatureSet); 5] = [
        ("ANSI", FeatureSet::ANSI),
        ("POSTGRES", FeatureSet::POSTGRES),
        ("SQLITE", FeatureSet::SQLITE),
        ("MYSQL", FeatureSet::MYSQL),
        ("DUCKDB", FeatureSet::DUCKDB),
    ];

    /// Every ledger claimant flag, valued by preset — `[ANSI, POSTGRES, SQLITE, MYSQL,
    /// DUCKDB]`. The pinned cells are the measured `FeatureSet` truth, re-derived by running
    /// this test; `value_pinned_gates_cover_every_ledger_claimant` proves this table's flag
    /// set is exactly the ledger's claimants, so a new multi-claimant head forces a new row.
    const STATEMENT_HEAD_GATE_VALUES: &[HeadGateValueRow] = &[
        // DO — PostgreSQL's anonymous code block vs MySQL's `DO <expr-list>`.
        HeadGateValueRow {
            flag: "do_statement",
            get: |f| f.utility_syntax.do_statement,
            expected: [F, T, F, F, F],
        },
        HeadGateValueRow {
            flag: "do_expression_list",
            get: |f| f.utility_syntax.do_expression_list,
            expected: [F, F, F, T, F],
        },
        // PREPARE/EXECUTE/DEALLOCATE — DuckDB/PostgreSQL typed-`AS` lifecycle vs MySQL `FROM`.
        HeadGateValueRow {
            flag: "prepared_statements",
            get: |f| f.utility_syntax.prepared_statements,
            expected: [F, T, F, F, T],
        },
        HeadGateValueRow {
            flag: "prepared_statements_from",
            get: |f| f.utility_syntax.prepared_statements_from,
            expected: [F, F, F, T, F],
        },
        // GRANT/REVOKE — the extended standard grammar (on by default, off in MySQL/SQLite)
        // vs MySQL's account-based route.
        HeadGateValueRow {
            flag: "access_control_account_grants",
            get: |f| f.access_control_syntax.access_control_account_grants,
            expected: [F, F, F, T, F],
        },
        HeadGateValueRow {
            flag: "access_control_extended_objects",
            get: |f| f.access_control_syntax.access_control_extended_objects,
            expected: [T, T, F, F, T],
        },
        // SET — the generic session grammar vs MySQL's `variable_assignment` comma-list route.
        HeadGateValueRow {
            flag: "variable_assignment",
            get: |f| f.session_variables.variable_assignment,
            expected: [F, F, F, T, F],
        },
        // LOCK/UNLOCK — MySQL-only.
        HeadGateValueRow {
            flag: "lock_tables",
            get: |f| f.utility_syntax.lock_tables,
            expected: [F, F, F, T, F],
        },
        HeadGateValueRow {
            flag: "lock_instance",
            get: |f| f.utility_syntax.lock_instance,
            expected: [F, F, F, T, F],
        },
        // LOAD — MySQL `LOAD DATA` / key-cache vs PostgreSQL/DuckDB `LOAD <extension>`.
        HeadGateValueRow {
            flag: "load_data",
            get: |f| f.utility_syntax.load_data,
            expected: [F, F, F, T, F],
        },
        HeadGateValueRow {
            flag: "load_extension",
            get: |f| f.utility_syntax.load_extension,
            expected: [F, T, F, F, T],
        },
        HeadGateValueRow {
            flag: "key_cache_statements",
            get: |f| f.utility_syntax.key_cache_statements,
            expected: [F, F, F, T, F],
        },
        // VACUUM — SQLite `VACUUM … INTO` vs DuckDB `VACUUM [ANALYZE]`.
        HeadGateValueRow {
            flag: "vacuum",
            get: |f| f.maintenance_syntax.vacuum,
            expected: [F, F, T, F, F],
        },
        HeadGateValueRow {
            flag: "vacuum_analyze",
            get: |f| f.maintenance_syntax.vacuum_analyze,
            expected: [F, F, F, F, T],
        },
        // ANALYZE — SQLite/DuckDB bare `ANALYZE` vs MySQL's `ANALYZE TABLE` verb family.
        HeadGateValueRow {
            flag: "analyze",
            get: |f| f.maintenance_syntax.analyze,
            expected: [F, F, T, F, T],
        },
        HeadGateValueRow {
            flag: "table_maintenance",
            get: |f| f.maintenance_syntax.table_maintenance,
            expected: [F, F, F, T, F],
        },
        // UPDATE — DuckDB's `UPDATE EXTENSIONS`.
        HeadGateValueRow {
            flag: "update_extensions",
            get: |f| f.utility_syntax.update_extensions,
            expected: [F, F, F, F, T],
        },
        // ALTER VIEW — MySQL redefinition vs DuckDB `SET SCHEMA` relocation.
        HeadGateValueRow {
            flag: "view_definition_options",
            get: |f| f.statement_ddl_gates.view_definition_options,
            expected: [F, F, F, T, F],
        },
        HeadGateValueRow {
            flag: "alter_object_set_schema",
            get: |f| f.statement_ddl_gates.alter_object_set_schema,
            expected: [F, F, F, F, T],
        },
        // ALTER DATABASE/SCHEMA — DuckDB `SET ALIAS TO` relocation vs MySQL option list.
        HeadGateValueRow {
            flag: "alter_database",
            get: |f| f.statement_ddl_gates.alter_database,
            expected: [F, F, F, F, T],
        },
        HeadGateValueRow {
            flag: "alter_database_options",
            get: |f| f.statement_ddl_gates.alter_database_options,
            expected: [F, F, F, T, F],
        },
        // DROP — MySQL's `drop_database` synonym / mandatory-`ON` index drop displacements.
        HeadGateValueRow {
            flag: "drop_database",
            get: |f| f.statement_ddl_gates.drop_database,
            expected: [F, F, F, T, F],
        },
        HeadGateValueRow {
            flag: "index_drop_on_table",
            get: |f| f.index_alter_syntax.index_drop_on_table,
            expected: [F, F, F, T, F],
        },
        // IMPORT — MySQL `IMPORT TABLE` vs DuckDB `IMPORT DATABASE`.
        HeadGateValueRow {
            flag: "import_table",
            get: |f| f.utility_syntax.import_table,
            expected: [F, F, F, T, F],
        },
        HeadGateValueRow {
            flag: "export_import_database",
            get: |f| f.utility_syntax.export_import_database,
            expected: [F, F, F, F, T],
        },
    ];

    /// Pin every ledger claimant gate's *value* in each core preset.
    ///
    /// The full-struct divergence tests in `duckdb.rs` assert `duck.utility_syntax ==
    /// UtilitySyntax { …deltas…, ..pg.utility_syntax }` — they pin the divergence *set*, so a
    /// base-preset flip (say `POSTGRES.utility_syntax.load_data` armed by mistake) flows into
    /// both the actual DuckDB preset and the expected struct through the same `..pg` spread
    /// and the test still passes. This test reads each flag by absolute value across all five
    /// presets, so any base flip that a `..UtilitySyntax::POSTGRES` / `..UtilitySyntax::ANSI`
    /// (etc.) spread silently inherits changes a pinned cell and fails here. The sharpest
    /// guarded case is a MySQL-only gate riding an all-false ANSI spread (`load_data`,
    /// `lock_tables`, …): were ANSI to arm it, MySQL would inherit `true` with no local edit.
    #[test]
    fn statement_head_gate_values_are_pinned_per_preset() {
        for row in STATEMENT_HEAD_GATE_VALUES {
            for ((name, preset), &expected) in CORE_PRESETS.iter().zip(row.expected.iter()) {
                assert_eq!(
                    (row.get)(preset),
                    expected,
                    "{}: expected {expected} in {name} — a statement-head gate value drifted \
                     (a base-preset flip may have silently propagated through a struct spread)",
                    row.flag,
                );
            }
        }
    }

    /// The value pin covers exactly the ledger's claimants: a new multi-claimant head forces
    /// a new valued row (with its own measured per-preset cells), so the value-level coverage
    /// cannot silently fall behind the ledger. Kept separate from the value assertions so the
    /// flag *set* is gated even if a value cell is later, deliberately, changed.
    #[test]
    fn value_pinned_gates_cover_every_ledger_claimant() {
        let mut from_ledger: Vec<&str> = MULTI_CLAIMANT_STATEMENT_HEADS
            .iter()
            .flat_map(|head| head.claimants.iter().copied())
            .collect();
        from_ledger.sort_unstable();
        from_ledger.dedup();

        let mut valued: Vec<&str> = STATEMENT_HEAD_GATE_VALUES
            .iter()
            .map(|row| row.flag)
            .collect();
        valued.sort_unstable();
        valued.dedup();

        assert_eq!(
            from_ledger, valued,
            "the value pin must value exactly the ledger's claimant flags — add or remove a \
             row in STATEMENT_HEAD_GATE_VALUES to match a ledger change",
        );
    }

    /// Every ledger row is internally coherent: a resolution kind that keeps both
    /// claimants excludes nothing, and an exclusion kind excludes at least one *listed*
    /// claimant. This runs in the default (ANSI-only) build, so the ledger's shape is
    /// gated even without the `lenient` feature.
    #[test]
    fn ledger_rows_are_internally_coherent() {
        for head in MULTI_CLAIMANT_STATEMENT_HEADS {
            match head.resolution {
                HeadResolution::MeceLookahead | HeadResolution::DispatchOrderUnion => {
                    assert!(
                        head.lenient_excludes.is_empty(),
                        "{:?}: union kinds keep every claimant, so lenient_excludes must be empty",
                        head.heads,
                    );
                }
                HeadResolution::OneReadingExclusion | HeadResolution::Route => {
                    assert!(
                        !head.lenient_excludes.is_empty(),
                        "{:?}: exclusion kinds must forgo at least one claimant",
                        head.heads,
                    );
                }
            }
            for excluded in head.lenient_excludes {
                assert!(
                    head.claimants.contains(excluded),
                    "{:?}: excluded flag {excluded:?} is not among the row's claimants",
                    head.heads,
                );
            }
        }
    }

    /// The exclusion columns match `FeatureSet::LENIENT` in both directions: every
    /// statement-head flag Lenient turns off appears as an excluded claimant in exactly one
    /// row, and every excluded claimant in the ledger is genuinely off in Lenient. This is
    /// the artifact `oracle-parity-lenient` consumes — a new head-level exclusion needs a
    /// ledger row, not an allowlist line.
    ///
    /// Gated on the `lenient` feature because `FeatureSet::LENIENT` compiles only there;
    /// the scope is the statement-head exclusions, NOT every `false` flag in the preset
    /// (lexical sacrifices — `double_quoted_strings`, `subscript`, `money_literals`, … — are
    /// `LexicalConflict`'s concern, and restriction flags default off as pure permissiveness).
    #[cfg(feature = "lenient")]
    #[test]
    fn lenient_exclusions_match_the_ledger() {
        let lenient = FeatureSet::LENIENT;
        // The statement-head claimants Lenient documents as "stays off" (lenient.rs), paired
        // with their live flag value so the assertion reads the real preset, not a mirror.
        let stays_off: [(&str, bool); 6] = [
            (
                "variable_assignment",
                lenient.session_variables.variable_assignment,
            ),
            (
                "do_expression_list",
                lenient.utility_syntax.do_expression_list,
            ),
            (
                "prepared_statements_from",
                lenient.utility_syntax.prepared_statements_from,
            ),
            ("drop_database", lenient.statement_ddl_gates.drop_database),
            (
                "index_drop_on_table",
                lenient.index_alter_syntax.index_drop_on_table,
            ),
            (
                "access_control_account_grants",
                lenient.access_control_syntax.access_control_account_grants,
            ),
        ];
        for (name, is_on) in stays_off {
            assert!(
                !is_on,
                "Lenient documents {name:?} as off, but the preset has it on"
            );
        }

        let mut from_ledger: Vec<&str> = MULTI_CLAIMANT_STATEMENT_HEADS
            .iter()
            .flat_map(|head| head.lenient_excludes.iter().copied())
            .collect();
        from_ledger.sort_unstable();
        // No head-level exclusion is recorded twice.
        let deduped = {
            let mut d = from_ledger.clone();
            d.dedup();
            d
        };
        assert_eq!(
            from_ledger, deduped,
            "a flag is excluded by two ledger rows"
        );

        let mut expected: Vec<&str> = stays_off.iter().map(|(name, _)| *name).collect();
        expected.sort_unstable();
        assert_eq!(
            from_ledger, expected,
            "the ledger's exclusion columns must be exactly the statement-head flags Lenient turns off",
        );
    }
}
