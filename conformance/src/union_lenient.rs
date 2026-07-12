// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The `Lenient` union property as a standing generative invariant (oracle-parity-lenient).
//!
//! # The property
//!
//! `Lenient` has no engine by construction — its ground truth is the **union property**:
//! any statement accepted by any enabled shipped preset (ANSI, PostgreSQL, MySQL, SQLite,
//! DuckDB) must also be accepted by `Lenient`, *except* where a deliberate exclusion is
//! documented. This lane drives the cheaply-available accepted surface of each shipped
//! preset — the per-dialect committed seeds and the flag-aware generative probes in
//! [`crate::properties`] — through `Lenient` and classifies every reject: a reject that is
//! not a sanctioned exception is a real union gap and fails loudly.
//!
//! # The three sanctioned exception sources
//!
//! The exception ledger has **three** parts, not just the head ledger + "rules 1-8". A
//! `Lenient` reject of a preset-accepted
//! statement is sanctioned iff it is attributable to one of:
//!
//! 1. **Head-contention ledger** — the six `lenient_excludes` flags in
//!    [`MULTI_CLAIMANT_STATEMENT_HEADS`]: `variable_assignment` (SET), `do_expression_list`
//!    (DO), `prepared_statements_from` (PREPARE/EXECUTE/DEALLOCATE), `drop_database` +
//!    `index_drop_on_table` (DROP), `access_control_account_grants` (GRANT/REVOKE). Each is
//!    a MySQL-side one-reading/route exclusion; this lane consumes the ledger rows directly
//!    ([`head_ledger_exclusion`]) rather than keeping a private allowlist, so a new
//!    head-level exclusion requires a ledger row.
//! 2. **Reserved-identifier model (lenient.rs rule 5)** — a word the source dialect frees
//!    but ANSI (hence `Lenient`) reserves, so it cannot be a bare identifier under `Lenient`
//!    (`ANALYZE`, `INT`, `BINARY`, `STRAIGHT_JOIN`, …; quote to recover). Detected
//!    *algorithmically and self-provingly* by [`reserved_model_recovers`]: re-parsing under
//!    a `Lenient` featureset that borrows only the source preset's four reserved-keyword
//!    sets recovers acceptance, so the reject is attributable purely to the reserved model.
//! 3. **Lexical-trigger / grammar exclusions (lenient.rs rules 1-4, 8 + the documented
//!    off-flags)** — a shared tokenizer trigger `Lenient` resolves the other way (`[`/`{`
//!    as bracket identifier vs array/collection punctuation, rule 2; `$`+digit as parameter
//!    vs money, rule 3; …) or a documented deliberate off-flag (`indexed_by`,
//!    `table_json_path`). These are genuine [`LexicalConflict`](squonk::ast::dialect::LexicalConflict)s:
//!    no *consistent* featureset keeps `Lenient`'s reading and the rival's at once (the
//!    parser's own lexical-consistency validator enforces this), so they are attributed by
//!    the trigger the source-on/`Lenient`-off flag visibly uses ([`lexical_trigger_reason`]).
//!    This attribution is a content heuristic, not a proof — see its precision note.
//!
//! # The superset asymmetry (only one direction is claimed)
//!
//! The union property is **one-directional**: `Lenient` accepting a statement *no* shipped
//! preset accepts is allowed and expected — `Lenient` is a permissive *superset*, so it
//! carries forms (e.g. `TABLE(<expr>)`, `MATCH_RECOGNIZE`) no oracle-backed preset enables.
//! This lane therefore never asserts the inverse ("`Lenient`-accepted ⟹ some-preset-accepts");
//! it asserts only "preset-accepted ⟹ `Lenient`-accepted-or-sanctioned-exception".
//!
//! # The VACUUM residual
//!
//! `VACUUM ANALYZE INTO 'f'` (SQLite accepts, `Lenient` rejects) is resolved here **by
//! measurement** as a *rule-5 reserved-identifier-model* sacrifice, **not** a head-ledger
//! row: `ANALYZE`
//! is reserved under `Lenient`'s ANSI model, the VACUUM head is a clean `DispatchOrderUnion`
//! keeping both tails (its `lenient_excludes` is empty), and `VACUUM "ANALYZE" INTO 'f'`
//! recovers. [`vacuum_analyze_into_is_a_reserved_model_sacrifice_not_a_ledger_row`] pins
//! this so the residual can never silently drift into a violation or a spurious ledger row.

use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use squonk::ast::NoExt;
use squonk::ast::dialect::{FeatureSet, MULTI_CLAIMANT_STATEMENT_HEADS, MultiClaimantHead};
use squonk::dialect::{Ansi, DuckDb, Lenient, MySql, Postgres, Sqlite};
use squonk::{Dialect, parse_with};

use crate::properties::dialect_features::ANSI_ROUNDTRIP_SEEDS;
use crate::properties::{
    DUCKDB_FEATURE_PROBES, DUCKDB_FEATURE_SEEDS, FeatureProbe, MYSQL_FEATURE_PROBES,
    MYSQL_FEATURE_SEEDS, POSTGRES_FEATURE_PROBES, POSTGRES_FEATURE_SEEDS, SQLITE_FEATURE_PROBES,
    SQLITE_MISFEATURE_SEEDS, arb_feature_statement,
};

/// A `Dialect` over an arbitrary [`FeatureSet`], for the reserved-model recovery probe.
struct AdHocFeatures(FeatureSet);

impl Dialect for AdHocFeatures {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &self.0
    }
}

fn accepts<D: Dialect + Copy>(sql: &str, dialect: D) -> bool {
    parse_with(sql, dialect).is_ok()
}

// ---------------------------------------------------------------------------
// (1) Head-contention ledger consumption
// ---------------------------------------------------------------------------

/// One head-ledger `lenient_excludes` flag, with a live accessor onto a [`FeatureSet`].
///
/// The `name` matches a `lenient_excludes` entry in [`MULTI_CLAIMANT_STATEMENT_HEADS`]
/// verbatim; [`excluded_flag_names_match_the_ledger`] proves this table's name set equals
/// the ledger's exclusion columns, so a new head exclusion forces a new accessor here (the
/// ledger stays the single source of truth — no private allowlist).
struct ExcludedFlag {
    name: &'static str,
    is_on: fn(&FeatureSet) -> bool,
}

const LENIENT_EXCLUDED_FLAGS: &[ExcludedFlag] = &[
    ExcludedFlag {
        name: "variable_assignment",
        is_on: |f| f.session_variables.variable_assignment,
    },
    ExcludedFlag {
        name: "do_expression_list",
        is_on: |f| f.utility_syntax.do_expression_list,
    },
    ExcludedFlag {
        name: "prepared_statements_from",
        is_on: |f| f.utility_syntax.prepared_statements_from,
    },
    ExcludedFlag {
        name: "drop_database",
        is_on: |f| f.statement_ddl_gates.drop_database,
    },
    ExcludedFlag {
        name: "index_drop_on_table",
        is_on: |f| f.index_alter_syntax.index_drop_on_table,
    },
    ExcludedFlag {
        name: "access_control_account_grants",
        is_on: |f| f.access_control_syntax.access_control_account_grants,
    },
];

/// The leading word tokens of `sql`, upper-cased (alphanumeric/underscore runs).
fn word_tokens(sql: &str) -> Vec<String> {
    sql.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_uppercase())
        .collect()
}

/// Whether a ledger `heads` entry (possibly multi-word, e.g. `"LOAD INDEX"`) is a prefix
/// of `tokens`.
fn head_entry_matches(entry: &str, tokens: &[String]) -> bool {
    let head_words: Vec<String> = entry
        .split_whitespace()
        .map(|w| w.to_ascii_uppercase())
        .collect();
    head_words.len() <= tokens.len() && head_words.iter().zip(tokens).all(|(h, t)| h == t)
}

/// The head-ledger exclusion row that explains a `Lenient` reject of `sql` under `src`, if
/// any: an exclusion row (`lenient_excludes` non-empty) whose head leads `sql` and at least
/// one of whose excluded flags is enabled on the source preset. `None` for statements no
/// exclusion row claims.
fn head_ledger_exclusion(sql: &str, src: &FeatureSet) -> Option<&'static MultiClaimantHead> {
    let tokens = word_tokens(sql);
    MULTI_CLAIMANT_STATEMENT_HEADS.iter().find(|row| {
        !row.lenient_excludes.is_empty()
            && row.heads.iter().any(|h| head_entry_matches(h, &tokens))
            && row.lenient_excludes.iter().any(|excluded| {
                LENIENT_EXCLUDED_FLAGS
                    .iter()
                    .find(|f| f.name == *excluded)
                    .is_some_and(|f| (f.is_on)(src))
            })
    })
}

// ---------------------------------------------------------------------------
// (2) Reserved-identifier model (rule 5) — algorithmic, self-proving
// ---------------------------------------------------------------------------

/// `FeatureSet::LENIENT` with only the four reserved-keyword sets replaced by `src`'s.
///
/// The reserved sets are not lexical triggers, so this borrow is always a *consistent*
/// featureset (unlike borrowing the `[`/`$` tokenizer axes). If the reject recovers under
/// it, the reject is attributable purely to rule 5 — the ANSI reserved-identifier model
/// `Lenient` keeps where the source dialect frees the word.
fn lenient_with_reserved_of(src: &FeatureSet) -> FeatureSet {
    let mut features = FeatureSet::LENIENT.clone();
    features.reserved_column_name = src.reserved_column_name;
    features.reserved_function_name = src.reserved_function_name;
    features.reserved_type_name = src.reserved_type_name;
    features.reserved_bare_alias = src.reserved_bare_alias;
    features
}

/// Whether the `Lenient` reject of `sql` recovers when `Lenient` borrows `src`'s reserved
/// model — the algorithmic proof that the reject is a rule-5 reserved-identifier sacrifice.
fn reserved_model_recovers(sql: &str, src: &FeatureSet) -> bool {
    parse_with(sql, AdHocFeatures(lenient_with_reserved_of(src))).is_ok()
}

// ---------------------------------------------------------------------------
// (3) Lexical-trigger / grammar exclusions — content attribution
// ---------------------------------------------------------------------------

/// The documented lexical-trigger or deliberate-grammar-exclusion reason for a `Lenient`
/// reject of `sql`, attributed by the source-on / `Lenient`-off flag whose trigger the
/// statement visibly uses.
///
/// # Precision
///
/// This is a *content heuristic*, not a proof: unlike [`reserved_model_recovers`], these
/// rejects are genuine [`LexicalConflict`](squonk::ast::dialect::LexicalConflict)s, so
/// no consistent featureset borrow can recover them (the parser's own lexical-consistency
/// validator rejects the borrow — that inconsistency *is* the conflict). Attribution here
/// requires the trigger flag on in `src`, off in `Lenient`, and the statement to carry the
/// trigger token. It can therefore mask a hypothetical real violation that coincidentally
/// carries the same trigger; the measured seed surface exhibits no such case (every
/// bracket/`INDEXED` reject is the documented sacrifice), and the algorithmic checks (1)/(2)
/// run first, so only rejects those two decline reach here.
fn lexical_trigger_reason(sql: &str, src: &FeatureSet) -> Option<&'static str> {
    let upper = sql.to_ascii_uppercase();

    // Rule 2: `[` is a bracket identifier quote under Lenient (subscript / array_constructor
    // / collection_literals off), so `[`/`{` array & collection punctuation and the `ARRAY`
    // constructor keyword (whose subquery form rides the same off flag) cannot parse.
    let src_has_bracket_forms = src.expression_syntax.subscript
        || src.expression_syntax.array_constructor
        || src.expression_syntax.multidim_array_literals
        || src.expression_syntax.collection_literals;
    if src_has_bracket_forms && (sql.contains('[') || sql.contains('{') || upper.contains("ARRAY"))
    {
        return Some("rule-2 bracket/brace trigger (`[`/`{` claimed as identifier quote)");
    }

    // Documented deliberate grammar exclusion: `INDEXED BY` / `NOT INDEXED` is off so a bare
    // `INDEXED` stays a correlation alias (lenient.rs TableExpressionSyntax::indexed_by).
    if src.table_expressions.indexed_by && upper.contains("INDEXED") {
        return Some("indexed_by grammar exclusion (bare INDEXED kept as an alias)");
    }

    // Rule 3: `$`+digit is a positional parameter, not a T-SQL money literal.
    if src.numeric_literals.money_literals
        && sql.contains('$')
        && sql.bytes().any(|b| b.is_ascii_digit())
    {
        return Some("rule-3 `$`+digit trigger (positional parameter, not money)");
    }

    None
}

// ---------------------------------------------------------------------------
// The unified classifier
// ---------------------------------------------------------------------------

/// Why a `Lenient` reject of a preset-accepted statement is sanctioned (or that it is not).
#[derive(Debug)]
enum Sanction {
    /// A head-contention ledger exclusion row explains the reject.
    HeadLedger(&'static [&'static str]),
    /// The reject recovers under `Lenient`+source-reserved-model (rule 5).
    ReservedModel,
    /// A documented lexical-trigger / grammar exclusion, with its reason.
    LexicalTrigger(&'static str),
}

/// Classify a `Lenient` reject of `sql` that the source preset `src` accepts. `None` means
/// the reject is a real union violation — no sanctioned exception explains it.
fn sanctioned_reject(sql: &str, src: &FeatureSet) -> Option<Sanction> {
    if let Some(row) = head_ledger_exclusion(sql, src) {
        return Some(Sanction::HeadLedger(row.heads));
    }
    if reserved_model_recovers(sql, src) {
        return Some(Sanction::ReservedModel);
    }
    lexical_trigger_reason(sql, src).map(Sanction::LexicalTrigger)
}

/// Drive one shipped preset's accepted surface through `Lenient`, failing on any reject that
/// is not a sanctioned exception.
fn assert_union_over_seeds<D: Dialect + Copy>(preset: &str, dialect: D, seeds: &[&str]) {
    let src = dialect.features();
    for &sql in seeds {
        assert!(
            accepts(sql, dialect),
            "{preset} seed no longer parses under its own preset (accepted-surface guard \
             broke): {sql:?}",
        );
        if accepts(sql, Lenient) {
            continue; // union holds
        }
        assert!(
            sanctioned_reject(sql, src).is_some(),
            "UNION VIOLATION: {preset} accepts but Lenient rejects with no sanctioned \
             exception — {sql:?}: {:?}",
            parse_with(sql, Lenient).err(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The union property over the five shipped presets' cheap accepted-surface seeds.
    #[test]
    fn union_property_holds_over_the_shipped_seed_surface() {
        assert_union_over_seeds("ANSI", Ansi, ANSI_ROUNDTRIP_SEEDS);
        assert_union_over_seeds("PostgreSQL", Postgres, POSTGRES_FEATURE_SEEDS);
        assert_union_over_seeds("SQLite", Sqlite, SQLITE_MISFEATURE_SEEDS);
        assert_union_over_seeds("MySQL", MySql, MYSQL_FEATURE_SEEDS);
        assert_union_over_seeds("DuckDB", DuckDb, DUCKDB_FEATURE_SEEDS);
    }

    /// My six excluded-flag accessors are exactly the ledger's `lenient_excludes` union —
    /// the tie that keeps [`MULTI_CLAIMANT_STATEMENT_HEADS`] the single source of truth.
    #[test]
    fn excluded_flag_names_match_the_ledger() {
        let mut from_ledger: Vec<&str> = MULTI_CLAIMANT_STATEMENT_HEADS
            .iter()
            .flat_map(|row| row.lenient_excludes.iter().copied())
            .collect();
        from_ledger.sort_unstable();
        from_ledger.dedup();

        let mut mine: Vec<&str> = LENIENT_EXCLUDED_FLAGS.iter().map(|f| f.name).collect();
        mine.sort_unstable();
        mine.dedup();

        assert_eq!(
            mine, from_ledger,
            "the excluded-flag accessor table must equal the ledger's lenient_excludes columns \
             — add/remove an ExcludedFlag to match a ledger change",
        );
    }

    /// The head-ledger exclusions have teeth: each of the six is a genuine
    /// MySQL-accept / Lenient-reject acceptance gap, and the lane attributes it to the
    /// ledger row (not a private allowlist). Measured under the fitted `MySql` preset (all
    /// six excluded flags are MySQL-exclusive among the shipped presets).
    #[test]
    fn head_ledger_exclusions_are_genuine_gaps_and_consumed() {
        // (statement, the ledger head-keyword it must be attributed to).
        let cases: &[(&str, &str)] = &[
            ("SET a = 1, b = 2", "SET"),                       // variable_assignment
            ("SET @v := 1", "SET"),                            // variable_assignment (`:=`)
            ("DO 1", "DO"),                                    // do_expression_list
            ("DO 1, 2", "DO"),                                 // do_expression_list
            ("PREPARE s FROM 'SELECT 1'", "PREPARE"),          // prepared_statements_from
            ("EXECUTE s USING @a", "EXECUTE"),                 // prepared_statements_from
            ("DROP DATABASE d", "DROP"),                       // drop_database
            ("DROP INDEX ix ON t", "DROP"),                    // index_drop_on_table
            ("GRANT ALL ON db.* TO 'u'@'localhost'", "GRANT"), // access_control_account_grants
        ];
        let src = MySql.features();
        for &(sql, head) in cases {
            assert!(
                accepts(sql, MySql),
                "MySQL must accept the excluded form {sql:?}"
            );
            assert!(
                !accepts(sql, Lenient),
                "the ledger exclusion {sql:?} must be a real Lenient acceptance gap",
            );
            let Some(Sanction::HeadLedger(heads)) = sanctioned_reject(sql, src) else {
                panic!("the excluded form {sql:?} must classify as a HeadLedger exclusion")
            };
            assert!(
                heads.iter().any(|h| h.eq_ignore_ascii_case(head)),
                "{sql:?} attributed to row {heads:?}, expected head {head:?}",
            );
        }
    }

    /// The lexical-trigger / grammar exclusions (category 3) classify with a reason: the
    /// `[`/`{` array-punctuation forms (rule 2) and the `INDEXED BY` grammar exclusion. Each
    /// is a genuine preset-accept / Lenient-reject gap attributed to its documented trigger.
    #[test]
    fn lexical_trigger_sacrifices_classify_with_a_reason() {
        // (statement, source preset features).
        let pg = Postgres.features();
        let duck = DuckDb.features();
        let sqlite = Sqlite.features();
        let cases: &[(&str, &FeatureSet)] = &[
            ("SELECT ARRAY[1, 2, 3]", pg),
            ("SELECT ARRAY(SELECT a FROM t)", pg),
            ("SELECT {'x': 1, 'y': 2}", duck),
            ("SELECT * FROM t INDEXED BY ix", sqlite),
            ("SELECT * FROM t NOT INDEXED", sqlite),
        ];
        for &(sql, src) in cases {
            assert!(!accepts(sql, Lenient), "{sql:?} should be a Lenient reject");
            assert!(
                head_ledger_exclusion(sql, src).is_none() && !reserved_model_recovers(sql, src),
                "{sql:?} is a lexical/grammar trigger, not a head-ledger or reserved-model case",
            );
            let Some(Sanction::LexicalTrigger(reason)) = sanctioned_reject(sql, src) else {
                panic!("{sql:?} must classify as a LexicalTrigger sacrifice")
            };
            assert!(!reason.is_empty(), "the trigger reason must be recorded");
        }
    }

    /// The reserved-identifier-model (rule 5) rejects — a word the source frees but Lenient
    /// reserves — classify algorithmically and recover by quoting. One representative per
    /// distinct reserved word met on the seed surface, plus the STRAIGHT_JOIN alias case.
    #[test]
    fn reserved_model_sacrifices_classify_and_recover() {
        // (statement, source preset features, quote/alias recovery that Lenient accepts).
        let sqlite = Sqlite.features();
        let mysql = MySql.features();
        let cases: &[(&str, &FeatureSet, &str)] = &[
            (
                "CREATE TABLE t (a UNSIGNED BIG INT)",
                sqlite,
                "CREATE TABLE t (a UNSIGNED BIG \"INT\")",
            ),
            (
                "CREATE TABLE t (a LONG INTEGER)",
                sqlite,
                "CREATE TABLE t (a LONG \"INTEGER\")",
            ),
            (
                "CREATE TABLE t (a, b, UNIQUE(b COLLATE binary ASC))",
                sqlite,
                "CREATE TABLE t (a, b, UNIQUE(b COLLATE \"binary\" ASC))",
            ),
            (
                "SELECT * FROM t STRAIGHT_JOIN t AS x ON t.a = x.a",
                mysql,
                "SELECT * FROM t AS a STRAIGHT_JOIN t AS x ON a.a = x.a",
            ),
        ];
        for &(sql, src, recovery) in cases {
            assert!(!accepts(sql, Lenient), "{sql:?} should be a Lenient reject");
            assert!(
                head_ledger_exclusion(sql, src).is_none(),
                "{sql:?} is a reserved-model sacrifice, not a head-ledger exclusion",
            );
            assert!(
                reserved_model_recovers(sql, src),
                "{sql:?} must recover under Lenient+source-reserved-model (rule 5)",
            );
            assert!(
                accepts(recovery, Lenient),
                "quote/alias recovery must parse under Lenient: {recovery:?}",
            );
            assert!(
                matches!(sanctioned_reject(sql, src), Some(Sanction::ReservedModel)),
                "{sql:?} must classify as ReservedModel",
            );
        }
    }

    /// The VACUUM residual decision, pinned. `VACUUM ANALYZE INTO 'f'` is a rule-5
    /// reserved-identifier-model sacrifice (`ANALYZE` reserved under Lenient), NOT a
    /// head-ledger row — VACUUM's ledger row is a `DispatchOrderUnion` that keeps both
    /// tails (empty `lenient_excludes`).
    #[test]
    fn vacuum_analyze_into_is_a_reserved_model_sacrifice_not_a_ledger_row() {
        let sql = "VACUUM ANALYZE INTO 'f'";
        let sqlite = Sqlite.features();
        assert!(accepts(sql, Sqlite), "SQLite accepts {sql:?}");
        assert!(!accepts(sql, Lenient), "Lenient rejects {sql:?}");
        assert!(
            head_ledger_exclusion(sql, sqlite).is_none(),
            "VACUUM is a DispatchOrderUnion (no head-level exclusion), so no ledger row \
             claims this residual",
        );
        // The VACUUM ledger row keeps both claimants (its exclusion column is empty).
        let vacuum_row = MULTI_CLAIMANT_STATEMENT_HEADS
            .iter()
            .find(|r| r.heads == ["VACUUM"])
            .expect("the ledger carries a VACUUM row");
        assert!(
            vacuum_row.lenient_excludes.is_empty(),
            "the VACUUM row keeps both tails; the residual is lexical (rule 5), not a head \
             exclusion",
        );
        assert!(
            reserved_model_recovers(sql, sqlite),
            "the residual recovers under the SQLite reserved model — it is the ANALYZE-as-\
             reserved-word sacrifice",
        );
        assert!(
            accepts("VACUUM \"ANALYZE\" INTO 'f'", Lenient),
            "quoting ANALYZE recovers the statement under Lenient",
        );
        assert!(matches!(
            sanctioned_reject(sql, sqlite),
            Some(Sanction::ReservedModel)
        ));
    }

    /// Standing generative invariant: fixed-RNG exploration of each preset's flag-aware
    /// probes, every preset-accepted draw replayed under Lenient and classified. A draw
    /// that Lenient rejects with no sanctioned exception fails — the generative analogue of
    /// the seed lane. Deterministic (seeded ChaCha), so a preset tightening or a fresh
    /// divergence trips it reproducibly rather than via a rare random draw.
    #[test]
    fn generative_flag_aware_probes_expose_no_new_union_violation() {
        let presets: &[(&str, &FeatureSet, &'static [FeatureProbe])] = &[
            ("SQLite", Sqlite.features(), SQLITE_FEATURE_PROBES),
            ("MySQL", MySql.features(), MYSQL_FEATURE_PROBES),
            ("PostgreSQL", Postgres.features(), POSTGRES_FEATURE_PROBES),
            ("DuckDB", DuckDb.features(), DUCKDB_FEATURE_PROBES),
        ];
        for &(preset, features, probes) in presets {
            let mut runner = TestRunner::new_with_rng(
                Config {
                    cases: 256,
                    ..Config::default()
                },
                TestRng::from_seed(RngAlgorithm::ChaCha, &[0x1e; 32]),
            );
            let strategy = arb_feature_statement(features, probes);
            for _ in 0..256 {
                let (_family, sql) = strategy
                    .new_tree(&mut runner)
                    .expect("arb_feature_statement is infallible to instantiate")
                    .current();
                // Only the preset-accepted surface is in scope for the union property.
                if parse_with(&sql, AdHocFeatures((*features).clone())).is_err() {
                    continue;
                }
                if accepts(&sql, Lenient) {
                    continue;
                }
                assert!(
                    sanctioned_reject(&sql, features).is_some(),
                    "UNION VIOLATION ({preset} generative): preset accepts but Lenient rejects \
                     with no sanctioned exception — {sql:?}: {:?}",
                    parse_with(&sql, Lenient).err(),
                );
            }
        }
    }

    /// The superset asymmetry is real and only the forward direction is claimed: Lenient
    /// accepts forms no shipped preset accepts. A guard so the module's asymmetry claim is
    /// not merely documentation.
    #[test]
    fn lenient_is_a_strict_superset_the_inverse_is_not_claimed() {
        // `TABLE(<expr>)` (the Snowflake/Oracle table-expression factor, Lenient-only) and the
        // three-at-once identifier quote styles (`"` + `` ` `` + `[`) — the headline Lenient
        // capability — are forms no shipped preset accepts. The second witness pins the quote
        // triple in a table position: `` `tbl name` `` is a spaced backtick identifier, which
        // DuckDb's backtick-as-operator reading (`custom_operators` + `postfix_operators`)
        // cannot parse as a table factor, so DuckDb rejects it where Lenient reads it as a
        // quoted name. (A bare projection `` `b` `` alone is now a valid DuckDb operator
        // expression — engine-confirmed on 1.5.4 — so the discriminator moved to `FROM`.)
        for sql in [
            "SELECT * FROM TABLE(my_func(1))",
            "SELECT \"x\"::text, `y`, [z] FROM `tbl name`",
        ] {
            assert!(
                accepts(sql, Lenient),
                "Lenient accepts the superset form {sql:?}"
            );
            let no_preset_accepts = [
                accepts(sql, Ansi),
                accepts(sql, Postgres),
                accepts(sql, Sqlite),
                accepts(sql, MySql),
                accepts(sql, DuckDb),
            ]
            .iter()
            .all(|ok| !ok);
            assert!(
                no_preset_accepts,
                "{sql:?} is meant to be a Lenient-only superset form; if a shipped preset now \
                 accepts it, pick a different asymmetry witness",
            );
        }
    }
}
