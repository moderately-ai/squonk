// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Seeded, replayable generated-SQL corpus (`prod-corpus-generators`).
//!
//! Beyond the hand-authored and vendored corpora, this module manufactures SQL by
//! the cheapest correct route: draw a legal AST from the existing round-trip
//! strategy ([`arb_statement`]) and [`render`](render_generated) it to text. The
//! string is therefore parseable *by construction*, and re-parsing it is a real
//! differential oracle (ADR-0014) — no from-scratch SQL-text grammar is involved,
//! and the generated cases come from the same source of truth as the property
//! layer (ADR-0015) rather than a second, divergent opinion of "legal SQL".
//!
//! ## The determinism guarantee — what makes a seed reproducible
//!
//! The acceptance turns on this: the same `u64` seed must yield an identical corpus
//! on every run, machine, and process, so a failing case is reproduced by re-running
//! its seed rather than chased as a flake. That holds because every input to
//! generation is a pure function of the seed:
//!
//! 1. [`TestRng::from_seed`] with [`RngAlgorithm::ChaCha`] is a deterministic
//!    ChaCha20 stream — identical seed bytes produce an identical byte stream
//!    everywhere. [`seed_bytes`] expands the `u64` into ChaCha's 32-byte seed by a
//!    fixed, total rule, so the `u64` fully determines the stream.
//! 2. [`arb_statement`] and its sub-strategies are pure `proptest` combinators
//!    (`prop_oneof`, `collection::vec`, `option::of`, `prop_recursive`, `Just`,
//!    `any::<bool>`). Their `new_tree` draws *only* from the runner's RNG — there is
//!    no clock, thread id, address, or `HashMap`-iteration order in the strategy
//!    module to leak nondeterminism.
//! 3. We pull each tree with [`Strategy::new_tree`] + [`ValueTree::current`] in a
//!    fixed `0..count` loop and never call [`TestRunner::run`]. So `proptest`'s
//!    [`Config`] (case count, forking, failure persistence, the `PROPTEST_*`
//!    environment) is *never consulted* — only the seeded RNG drives generation.
//! 4. [`render_generated`] renders against the constant [`GENERATED_RESOLVER`]
//!    index→name table, so identifier spelling is a pure function of the AST; no
//!    live interner allocation order enters the rendered text.
//!
//! Replay is likewise order-independent: re-parsing allocates fresh symbol ids, so
//! the structural check remaps both trees through one shared interner
//! ([`shared_interner`]) before comparing — exactly as the property oracle does.
//!
//! ## What runs by default vs. on demand
//!
//! The fixed-seed smoke + determinism tests run in `cargo nextest run`; they are
//! fully deterministic and so cannot flake CI. The broad random sweep is `#[ignore]`
//! (opt in with `--run-ignored`); when its seed is not pinned via the environment it
//! draws one from entropy and *prints* it, so any failure still names an exact seed
//! to replay.
//!
//! ## Minimizing a failing case (`prod-corpus-minimize-shrink-failing-cases`)
//!
//! A large failing draw is not handed back verbatim: [`shrink_failing_roundtrip`]
//! re-drives the *same* [`arb_statement`] strategy through [`TestRunner::run`] with
//! the replay oracle ([`replay_case`]) as the predicate, so `proptest` simplifies the
//! failing AST to a minimal one that still fails. The broad sweep reports that
//! minimized repro (with the exact `(seed, sql)` tuple to commit), and a discovered
//! bug is persisted as a deterministic regression in [`SHRUNK_REGRESSIONS`]. The
//! shrink search is pinned to the same seed-as-sole-input determinism the corpus
//! makes (see [`shrink_failing_statement`]).

use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{
    Config, RngAlgorithm, TestCaseError, TestCaseResult, TestError, TestRng, TestRunner,
};
use squonk::dialect::Ansi;
use squonk::parse_with;
use squonk_ast::render::RenderMode;
use squonk_ast::{NoExt, Statement};

use crate::properties::{GENERATED_RESOLVER, arb_statement, normalize_statement, render_generated};
use crate::shared_interner;

/// One generated corpus case: the AST a seed produced and its canonical SQL text.
///
/// The `sql` is the corpus artefact a replay re-parses; the `statement` is the
/// ground truth the re-parsed tree is compared against (so the oracle catches a
/// renderer that drops or mis-binds structure, not merely round-trips a string).
pub(crate) struct GeneratedCase {
    /// Canonical-mode rendering of [`statement`](Self::statement); parseable by
    /// construction.
    pub(crate) sql: String,
    /// The AST the seed drew, in [`GENERATED_RESOLVER`]'s symbol space.
    pub(crate) statement: Statement<NoExt>,
}

/// Expand a `u64` seed into `proptest`'s 32-byte ChaCha seed.
///
/// The rule is arbitrary but fixed and total: the determinism guarantee only needs
/// `seed_bytes` to be a pure function of `seed`, which repeating the little-endian
/// `u64` across all four 8-byte lanes plainly is. (ChaCha diffuses a one-bit seed
/// change across the whole stream, so the expansion shape is not load-bearing.)
fn seed_bytes(seed: u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    let lane = seed.to_le_bytes();
    for chunk in bytes.chunks_mut(lane.len()) {
        chunk.copy_from_slice(&lane);
    }
    bytes
}

/// Deterministically generate `count` corpus cases from `seed`.
///
/// Same `(seed, count)` ⇒ identical cases in identical order — see the module-level
/// determinism guarantee. The strategy is built once and drawn `count` times from a
/// single seeded runner, so the cases form the seed's reproducible prefix: a longer
/// `count` extends an existing corpus rather than reshuffling it.
pub(crate) fn generate_corpus(seed: u64, count: usize) -> Vec<GeneratedCase> {
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &seed_bytes(seed));
    // `Config` is irrelevant here: we draw trees directly and never call
    // `runner.run`, so only the seeded RNG above influences the output.
    let mut runner = TestRunner::new_with_rng(Config::default(), rng);
    let strategy = arb_statement();

    (0..count)
        .map(|_| {
            let statement = strategy
                .new_tree(&mut runner)
                .expect("arb_statement is infallible to instantiate")
                .current();
            let sql = render_generated(&statement, RenderMode::Canonical);
            GeneratedCase { sql, statement }
        })
        .collect()
}

/// Re-parse one case's SQL and assert it round-trips to the AST the seed produced.
///
/// This is the replay oracle: it confirms the generated string parses, parses to a
/// single statement, and re-parses to a structurally equal tree. A parse failure or
/// a structural mismatch panics with the rendered SQL and a normalized diff, so the
/// owning seed (printed by the sweep, or pinned by the smoke test) is enough to
/// reproduce the failure.
///
/// # Panics
///
/// If `case.sql` fails to parse, parses to other than one statement, or re-parses to
/// a tree that is not structurally equal to `case.statement`.
pub(crate) fn replay_case(case: &GeneratedCase) {
    let reparsed = parse_with(&case.sql, Ansi)
        .unwrap_or_else(|err| panic!("generated SQL did not parse: {:?}: {err:?}", case.sql));

    let [reparsed_statement] = reparsed.statements() else {
        panic!(
            "generated SQL should parse to exactly one statement: {:?}",
            case.sql
        );
    };

    let comparison = shared_interner::compare_statement_with_shared_symbols(
        &case.statement,
        &GENERATED_RESOLVER,
        reparsed_statement,
        reparsed.resolver(),
    );
    if !comparison.structurally_equal() {
        let left = normalize_statement(&case.statement, &GENERATED_RESOLVER);
        let right = normalize_statement(reparsed_statement, reparsed.resolver());
        panic!(
            "{}",
            comparison.failure_message(
                "generated corpus round-trip structural mismatch",
                &[("generated SQL", case.sql.as_str())],
                Some((&left, &right)),
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// Shrinking a failing case to a minimal repro
// ---------------------------------------------------------------------------

/// A committed minimized round-trip regression: the originating `u64` seed paired
/// with the minimal SQL the shrinker distilled the failure to.
///
/// Stored as a bare `(seed, sql)` tuple (mirroring `fuzz`'s replay slices and
/// `fuzz`'s `EXCLUDED_DIVERGENCE_CLASSES`) so the slice needs no constructor and
/// stays dead-code-clean while empty. `sql` is the artefact each replay re-checks;
/// `seed` is provenance — re-running [`shrink_failing_roundtrip`] with it re-derives
/// the case, keeping the repro reproducible per the module's determinism guarantee.
type ShrunkRegression = (u64, &'static str);

/// Committed minimized round-trip regressions, replayed by
/// `committed_shrunk_regressions_round_trip`.
///
/// Empty today: the parser is mature and the generated corpus is all-green, so the
/// shrinker finds nothing to commit (the empty-but-ready slot mirrors
/// [`fuzz::DIFFERENTIAL_REPLAYS`](crate::fuzz::DIFFERENTIAL_REPLAYS)). When a sweep
/// finds a failure, [`shrink_failing_roundtrip`] minimizes it and the `(seed, sql)`
/// pair is pasted here as a permanent, seed-tagged regression alongside the vendored
/// `corpus_*` replayers.
const SHRUNK_REGRESSIONS: &[ShrunkRegression] = &[];

/// Cases the shrink driver draws looking for a failure before giving up, pinned (not
/// `PROPTEST_CASES`) so the search is a pure function of the seed. The default-on
/// synthetic shrink tests stay well inside the unit-test budget at this width, which
/// also matches `proptest`'s own default case count.
const SHRINK_CASES: u32 = 256;

/// Cap on `simplify` iterations, pinned to a concrete, generous bound rather than
/// `u32::MAX`. `u32::MAX` is `proptest`'s "automatic" sentinel, which resolves to
/// `cases * 4` (1024 here) — too few to fully reduce a large initial draw, leaving a
/// non-minimal repro. This bound is well above the natural convergence point of the
/// bounded [`arb_statement`] trees (shrinking stops on its own when neither
/// `simplify` nor `complicate` makes progress), so it caps runaway, not the result,
/// and stays env-immune for determinism.
const SHRINK_MAX_ITERS: u32 = 1_000_000;

/// Drive `proptest`'s shrinker over [`arb_statement`], returning the minimal
/// statement for which `fails` reports a failure, or `None` if no drawn case fails.
///
/// This is the minimization counterpart to [`generate_corpus`]: where that draws
/// trees directly and never consults the runner, this hands the same strategy to
/// [`TestRunner::run`], which draws up to `cases` trees from the seeded RNG until
/// `fails` rejects one, then repeatedly [`ValueTree::simplify`]s that failing tree —
/// re-running `fails` on each candidate — and returns the smallest tree that still
/// fails ([`TestError::Fail`]'s minimal value). `fails` signals a failure either by
/// returning `Err(`[`TestCaseError::Fail`]`)` or by panicking (`run` catches the
/// panic), so the panicking replay oracle can be reused verbatim as the predicate.
///
/// ## Determinism — the same guarantee [`generate_corpus`] makes
///
/// A minimized repro must be reproducible from its seed, so every input to the
/// search is pinned to a pure function of `seed`:
///
/// 1. the RNG is [`TestRng::from_seed`] over [`seed_bytes`], identical to
///    [`generate_corpus`];
/// 2. `cases`, `max_shrink_iters`, and `max_shrink_time` are pinned on the [`Config`]
///    so the `PROPTEST_*` environment cannot re-steer case selection, truncate the
///    shrink, or make it wall-clock-dependent (which would diverge across machines);
/// 3. `failure_persistence` is `None` and `fork` is off, so the search neither reads
///    nor writes `proptest-regressions/` files — the synthetic shrink tests fail by
///    design on every run — and never forks; it consults *only* the seeded RNG.
fn shrink_failing_statement(
    seed: u64,
    cases: u32,
    fails: impl Fn(&Statement<NoExt>) -> TestCaseResult,
) -> Option<Statement<NoExt>> {
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &seed_bytes(seed));
    let config = Config {
        // Pure function of the seed: never load or persist a `proptest-regressions`
        // file (the synthetic shrink tests fail by design every run), and pin the
        // env-steered knobs so `PROPTEST_*` cannot perturb which case is found or how
        // far it shrinks: a concrete [`SHRINK_MAX_ITERS`] cap (not the `u32::MAX`
        // "automatic" sentinel, which under-shrinks) and `max_shrink_time = 0` (no
        // wall-clock limit), so shrinking reaches the same fixed minimum everywhere.
        // `fork` off: in-process shrinking only.
        failure_persistence: None,
        cases,
        max_shrink_iters: SHRINK_MAX_ITERS,
        max_shrink_time: 0,
        fork: false,
        ..Config::default()
    };
    let mut runner = TestRunner::new_with_rng(config, rng);

    match runner.run(&arb_statement(), |statement| fails(&statement)) {
        Ok(()) => None,
        Err(TestError::Fail(_, minimal)) => Some(minimal),
        // `Abort` covers the strategy failing to instantiate or too many rejects;
        // `arb_statement` is infallible and the predicate never rejects, so this is
        // unreachable in practice — surface it loudly rather than swallow it.
        Err(TestError::Abort(reason)) => {
            panic!("shrink driver aborted before reaching a failing case: {reason}")
        }
    }
}

/// Minimize a round-trip failure in `seed`'s generated corpus to a minimal failing
/// [`GeneratedCase`], or `None` when every drawn case round-trips.
///
/// The predicate is the replay oracle itself ([`replay_case`]): each drawn — and each
/// simplified — statement is wrapped in the same Canonical-rendered [`GeneratedCase`]
/// a replay sees, and `replay_case` panics on a parse failure or structural mismatch,
/// which `run` catches and shrinks. Reusing the replay oracle verbatim keeps one
/// source of truth for "round-trips"; the per-candidate `clone` is the cost of that
/// reuse and is paid only on the opt-in shrink path, never in the default replay.
fn shrink_failing_roundtrip(seed: u64, cases: u32) -> Option<GeneratedCase> {
    let statement = shrink_failing_statement(seed, cases, |statement| {
        replay_case(&GeneratedCase {
            sql: render_generated(statement, RenderMode::Canonical),
            statement: statement.clone(),
        });
        Ok(())
    })?;
    let sql = render_generated(&statement, RenderMode::Canonical);
    Some(GeneratedCase { sql, statement })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seed pinned into CI. The smoke corpus is regenerated from it every run, so a
    /// regression in the generator, renderer, or parser fails here deterministically
    /// rather than waiting for a lucky random draw.
    const SMOKE_SEED: u64 = 0x5751_4C50_4152_5345; // ASCII "SQLPARSE": arbitrary, fixed.

    /// Drawn per run but small enough to stay well inside the unit-test budget; it
    /// matches `proptest`'s default case count so the smoke corpus is comparably wide.
    const SMOKE_COUNT: usize = 256;

    /// Environment override pinning the broad sweep's seed for deterministic replay.
    const SEED_ENV: &str = "SQUONK_CORPUS_SEED";
    /// Environment override for the broad sweep's case count.
    const COUNT_ENV: &str = "SQUONK_CORPUS_COUNT";

    /// Default-on: the fixed-seed corpus parses and round-trips. Deterministic, so it
    /// never flakes `cargo nextest run`.
    #[test]
    fn fixed_seed_corpus_replays() {
        let corpus = generate_corpus(SMOKE_SEED, SMOKE_COUNT);
        assert_eq!(corpus.len(), SMOKE_COUNT);
        for case in &corpus {
            assert!(
                !case.sql.is_empty(),
                "a generated case rendered to empty SQL"
            );
            replay_case(case);
        }
    }

    /// Default-on determinism proof: the same seed yields a byte-identical corpus.
    /// This is the property the acceptance turns on — no clock, address, or iteration
    /// -order nondeterminism leaks into generation.
    #[test]
    fn same_seed_is_byte_identical() {
        let first: Vec<String> = generate_corpus(SMOKE_SEED, SMOKE_COUNT)
            .into_iter()
            .map(|case| case.sql)
            .collect();
        let second: Vec<String> = generate_corpus(SMOKE_SEED, SMOKE_COUNT)
            .into_iter()
            .map(|case| case.sql)
            .collect();
        assert_eq!(
            first, second,
            "an identical seed must produce an identical corpus"
        );
    }

    /// Default-on: a longer `count` extends the corpus rather than reshuffling it, so
    /// the seed names a stable, growable prefix (relied on by replay-by-seed).
    #[test]
    fn longer_count_extends_the_same_prefix() {
        let short: Vec<String> = generate_corpus(SMOKE_SEED, 32)
            .into_iter()
            .map(|case| case.sql)
            .collect();
        let long: Vec<String> = generate_corpus(SMOKE_SEED, 64)
            .into_iter()
            .map(|case| case.sql)
            .collect();
        assert_eq!(&long[..short.len()], short.as_slice());
    }

    /// Default-on: distinct seeds diverge. Guards against an accidentally seed-blind
    /// generator, which would make the whole seed→replay workflow meaningless.
    #[test]
    fn distinct_seeds_diverge() {
        let a: Vec<String> = generate_corpus(1, 64)
            .into_iter()
            .map(|case| case.sql)
            .collect();
        let b: Vec<String> = generate_corpus(2, 64)
            .into_iter()
            .map(|case| case.sql)
            .collect();
        assert_ne!(a, b, "distinct seeds should produce different corpora");
    }

    /// Seed for the synthetic-predicate shrink tests. Fixed so the shrink path is
    /// byte-identical across runs (the same guarantee the corpus determinism tests
    /// make), and verified to find a failing case within [`SHRINK_CASES`] draws under
    /// both synthetic predicates below.
    const SHRINK_SEED: u64 = SMOKE_SEED;

    /// Default-on: the shrinker reduces a synthetic failure to a *minimal* failing
    /// case. There is no real round-trip bug today (the parser is mature), so the
    /// machinery — `TestRunner::run` + `ValueTree::simplify` + the minimal-value
    /// extraction — is proven against a synthetic predicate: "the rendered SQL
    /// contains a JOIN". `run` finds a statement containing a join, then `simplify`
    /// drives it down, stopping at the boundary where dropping more would lose the
    /// join (proptest *complicates* back when a simplification stops failing). The
    /// result is therefore small yet still satisfies the predicate — exactly the
    /// "minimal reproducing case" the ticket asks the real round-trip oracle to yield.
    #[test]
    fn shrink_driver_minimizes_a_synthetic_join_failure() {
        let minimal = shrink_failing_statement(SHRINK_SEED, SHRINK_CASES, |statement| {
            if render_generated(statement, RenderMode::Canonical).contains("JOIN") {
                Err(TestCaseError::fail("rendered SQL contains a JOIN"))
            } else {
                Ok(())
            }
        })
        .expect("the JOIN predicate finds a failing case within SHRINK_CASES draws");

        let sql = render_generated(&minimal, RenderMode::Canonical);
        // Still fails the predicate: shrinking stopped *at* the boundary, not past it.
        assert!(
            sql.contains("JOIN"),
            "the shrunk case must still contain a JOIN: {sql:?}"
        );
        // ...and is genuinely minimized: a minimal join is a single `SELECT ... JOIN`,
        // far shorter than a typical multi-clause generated draw (which run into the
        // hundreds of characters).
        assert!(
            sql.len() < 96,
            "the shrunk JOIN case should be minimal, got {} chars: {sql:?}",
            sql.len(),
        );
    }

    /// Default-on: the shrinker is deterministic *and* seeks a minimum. The same seed
    /// shrinks a length-boundary failure ("rendered SQL longer than 24 chars") to a
    /// byte-identical minimal case across runs — extending the module's determinism
    /// guarantee to the shrink path, so a committed minimized repro stays reproducible
    /// from its seed. The minimal case still exceeds the bound (it sits just past the
    /// boundary), proving `simplify` shrank toward it rather than returning the draw.
    #[test]
    fn shrink_driver_is_deterministic_and_seeks_a_minimum() {
        // Non-capturing, so `Copy` — the same predicate drives two independent runs.
        let longer_than_bound = |statement: &Statement<NoExt>| {
            if render_generated(statement, RenderMode::Canonical).len() > 24 {
                Err(TestCaseError::fail("rendered SQL longer than 24 chars"))
            } else {
                Ok(())
            }
        };

        let render = |statement| render_generated(&statement, RenderMode::Canonical);
        let first =
            shrink_failing_statement(SHRINK_SEED, SHRINK_CASES, longer_than_bound).map(render);
        let second =
            shrink_failing_statement(SHRINK_SEED, SHRINK_CASES, longer_than_bound).map(render);

        assert_eq!(
            first, second,
            "the same seed must shrink to the same minimal case"
        );
        let minimal = first.expect("the length predicate finds a failing case");
        assert!(
            minimal.len() > 24,
            "the shrunk case must still exceed the length bound: {minimal:?}"
        );
        assert!(
            minimal.len() < 96,
            "the shrunk case should sit near the boundary, got {} chars: {minimal:?}",
            minimal.len(),
        );
    }

    /// Default-on: the production path stays green on a mature parser. Driving the
    /// shrinker with the *replay oracle* as the predicate over the smoke seed finds no
    /// failing case, so the shrinker never fabricates a repro from a corpus that
    /// round-trips. (If a real round-trip bug landed, this flips and
    /// [`shrink_failing_roundtrip`] would hand back the minimized statement to commit
    /// into `SHRUNK_REGRESSIONS`.)
    #[test]
    fn shrink_driver_finds_no_failure_in_the_green_corpus() {
        assert!(
            shrink_failing_roundtrip(SMOKE_SEED, SMOKE_COUNT as u32).is_none(),
            "the smoke-seed corpus round-trips, so the shrinker must find nothing"
        );
    }

    /// Default-on: every committed minimized regression still parses and round-trips.
    /// This is the regression guard the shrinker feeds — a re-introduced bug fails
    /// here on the small shrunk SQL, not on a large generated statement. Vacuously
    /// green while [`SHRUNK_REGRESSIONS`] is empty; the assertion gains teeth the
    /// moment a case lands.
    #[test]
    fn committed_shrunk_regressions_round_trip() {
        for &(seed, sql) in SHRUNK_REGRESSIONS {
            // `seed` is the recorded provenance (re-run `shrink_failing_roundtrip(seed)`
            // to re-derive `sql`); print it so a resurfaced regression names its
            // originating seed. Reuse the crate's canonical string round-trip oracle —
            // not a second oracle — as the actual guard.
            eprintln!("replaying shrunk regression (seed {seed}): {sql:?}");
            crate::assert_roundtrips(sql);
            crate::assert_roundtrips_parenthesized(sql);
        }
    }

    /// Broad replay sweep — GATED behind `#[ignore]` so it never runs in the default
    /// `cargo nextest run` and so cannot flake CI. Opt in with:
    ///
    /// ```text
    /// cargo nextest run -p squonk-conformance --run-ignored all \
    ///     -E 'test(corpus_generated::tests::broad_sweep_replays)'
    /// ```
    ///
    /// Pin `SQUONK_CORPUS_SEED` (and optionally `SQUONK_CORPUS_COUNT`) to
    /// replay a specific run; left unset, the sweep draws an entropy seed and prints
    /// it, so any failure still names the exact seed to reproduce.
    #[test]
    #[ignore = "broad random corpus sweep; opt in with --run-ignored (prints its seed for replay)"]
    fn broad_sweep_replays() {
        let seed = sweep_seed();
        let count = sweep_count();
        // Printed (not asserted) so a failing sweep tells the operator how to replay
        // it deterministically: re-run with SQUONK_CORPUS_SEED set to this value.
        eprintln!(
            "corpus broad sweep: seed={seed} count={count} \
             (replay with {SEED_ENV}={seed} {COUNT_ENV}={count})"
        );
        // Drive the shrinker rather than a raw replay loop: on a round-trip failure
        // it hands back the *minimized* statement, so the operator gets a small repro
        // (and the exact `(seed, sql)` tuple to paste into `SHRUNK_REGRESSIONS`)
        // instead of a large generated draw. `cases` is the sweep count, clamped to
        // proptest's `u32` budget. `None` means the whole sample round-trips.
        let cases = u32::try_from(count).unwrap_or(u32::MAX);
        if let Some(minimal) = shrink_failing_roundtrip(seed, cases) {
            panic!(
                "broad sweep found a round-trip failure under seed {seed}; minimized \
                 repro — commit to SHRUNK_REGRESSIONS as ({seed}, {sql:?}):\n{sql}",
                sql = minimal.sql,
            );
        }
    }

    /// The sweep seed: the pinned `SQUONK_CORPUS_SEED` if set and parseable, else
    /// one drawn from entropy (printed by the caller for replay).
    fn sweep_seed() -> u64 {
        std::env::var(SEED_ENV)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(entropy_seed)
    }

    /// The sweep case count: `SQUONK_CORPUS_COUNT` if set and parseable, else a
    /// default wide enough to be worth the opt-in.
    fn sweep_count() -> usize {
        std::env::var(COUNT_ENV)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(4096)
    }

    /// A per-process entropy seed for an unpinned sweep. Only the `#[ignore]` sweep
    /// uses it, and it prints the result, so this is the one intentionally
    /// nondeterministic input — every default test path takes a fixed seed instead.
    fn entropy_seed() -> u64 {
        use std::hash::{BuildHasher as _, Hasher as _};
        use std::time::{SystemTime, UNIX_EPOCH};

        // `RandomState` is OS-seeded per construction; mixing in the wall clock makes
        // the result distinct per call as well as per process.
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos() as u64)
            .unwrap_or(0);
        hasher.write_u64(nanos);
        hasher.finish()
    }
}
