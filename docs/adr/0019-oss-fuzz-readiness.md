# ADR-0019: OSS-Fuzz onboarding — deferred (no-go for now)

- **Status:** Accepted (2026-06-29) — *records a deliberate deferral with an explicit revisit trigger*
- **Atoms:** — (resolves the OSS-Fuzz spike deferred by [ADR-0014](0014-testing-strategy.md))

## Context

[ADR-0014](0014-testing-strategy.md) stood up the fuzz layer and deliberately deferred OSS-Fuzz — *"OSS-Fuzz is deferred (needs a user base)."* That deferral has to remain a **decision**, not an oversight: once the libFuzzer targets matured, the readiness criteria must be re-checked and a go/no-go written down. The prerequisite targets now exist and this ADR is that decision.

[OSS-Fuzz](https://github.com/google/oss-fuzz) is Google's continuous-fuzzing service: it builds a project's fuzz targets in its own images, fuzzes them on Google infrastructure, and files bugs with a **90-day disclosure deadline** that the project's maintainers are expected to triage. Its acceptance bar favours software that is open-source and *"critical to the global IT infrastructure"* or otherwise widely depended upon.

### Current fuzz state (what is already in place)

The harness is mature and is the integration point OSS-Fuzz would consume — there is no missing fuzzing capability, only missing packaging:

- **Three libFuzzer targets** in `conformance/fuzz/fuzz_targets/`: `parse_no_panic`, `roundtrip`, and `differential`. Each is a thin `fuzz_target!` shell over a shared body in `conformance/src/fuzz.rs`, so the stable Bolero test checks and the nightly libFuzzer binaries run **one harness on two engines** (ADR-0014/0015).
- **Deliberately standalone:** `conformance/fuzz/` declares its own `[workspace]` and is nightly-only (`libfuzzer-sys`), so the published crates and `cargo nextest` stay on stable and never build it (`conformance/fuzz/Cargo.toml`).
- **Crash → replay workflow** documented in `conformance/fuzz/README.md`: `cargo +nightly fuzz tmin`, then commit the minimized bytes into the `PARSE_NO_PANIC_REPLAYS` / `ROUNDTRIP_REPLAYS` / `DIFFERENTIAL_REPLAYS` constants so the stable `*_replays_committed_inputs` tests replay them without nightly (`prod-fuzz-crash-corpus-workflow`, done).
- **Differential oracle:** the `differential` target renders generated SQL and compares accept/reject and structural parity against the real PostgreSQL parser via `pg_query` 6.1, gated by a divergence allowlist that is **currently empty** — no known live divergences (`prod-fuzz-differential-loop`, done).
- The generated libFuzzer `corpus/` and `artifacts/` are git-ignored (`conformance/fuzz/.gitignore`); the committed seeds today are inline byte constants in `conformance/src/fuzz.rs`, not an on-disk seed corpus.

## Decision

**No-go: do not onboard OSS-Fuzz now. Defer, with the explicit revisit trigger below.**

The fuzzing *itself* is ready in design; the blockers are about audience and ownership, not the harness. Two of the four ADR-0014 criteria are green (licensing, target design), and two are not met (a demonstrated crash-free record, and — decisively — an external user base plus a committed triage owner). For an alpha (`v0.1.0`) parser with a single internal consumer, the cost of a continuously-running external service that files deadline-bearing bugs exceeds the benefit, especially while the same targets can already be fuzzed locally and (once `prod-fuzz-ci-nightly-soak` lands) in CI. This keeps the ADR-0014 deferral deliberate and visible rather than forgotten.

## Readiness criteria (ADR-0014) → current state

| Criterion | Current state | Verdict |
|-----------|---------------|---------|
| **Target stability / non-flaky** | Stable *by construction*: bodies are deterministic, reject non-UTF-8 and oversized inputs, treat parse errors as success, and the structured targets generate an `arbitrary` legal subset, so a failure is a real bug. But there is **no evidence of a sustained crash-free run** — no committed soak record; the corpora are seed/replay constants, not the product of CPU-days of fuzzing. Stable in design, unproven by soak. | **Partial** |
| **Dependency licensing** | The fuzz build closure is uniformly permissive: workspace MIT; `libfuzzer-sys`, `arbitrary`, `thin-vec` MIT/Apache-2.0; `pg_query`/libpg_query is BSD-family and bundles PostgreSQL parser source under the PostgreSQL Licence — all on this repo's own permissive allowlist (`xtask/src/lib.rs` `ALLOWED_SPDX_LICENSES` lists BSD-3-Clause and PostgreSQL). **No GPL/copyleft is compiled into the fuzz crate**: the GPL MySQL/MariaDB suites are run as *external oracles*, never vendored or linked (ADR-0015; `xtask` `DISALLOWED_CORPUS_SOURCES`). OSS-Fuzz imposes no licence bar beyond buildable open source. | **Met** |
| **Maintenance capacity + user base** | Alpha, `v0.1.0`; libs pre-1.0, `conformance` is `publish = false`. The sole known downstream is an internal consumer (not yet public); **no external/public user base**. No maintainer has committed to triaging OSS-Fuzz reports inside the 90-day disclosure window — and the `differential` target in particular would generate ongoing triage load. This is the decisive factor and matches ADR-0014's stated reason. | **Not met** |
| **Harness / packaging changes** | None of the OSS-Fuzz integration files exist (`project.yaml`, `Dockerfile`, `build.sh`); seeds would need materializing to on-disk per-target dirs. Small, well-trodden for cargo-fuzz, but net-new and externally maintained. See *Packaging gap* below. | **Not met (work, not blocker)** |

## Revisit trigger

Re-open this decision (flip to a GO assessment) when **all** of the following hold:

1. **A real external user base exists** — e.g. the `squonk` libraries are published to crates.io with at least one external consumer, **or** the downstream consumer has shipped to production users; **and**
2. **A crash-free record exists** — the `parse_no_panic` and `roundtrip` targets have run **≥ 48 CPU-hours** (or a recurring CI nightly soak, `prod-fuzz-ci-nightly-soak`) crash-free on current `main`; **and**
3. **A named maintainer commits** to triaging OSS-Fuzz reports within the 90-day disclosure window.

Intermediate step (does not require GO): if continuous coverage is wanted before OSS-Fuzz, stand up the **in-repo CI nightly bounded fuzz run** first (`prod-fuzz-ci-nightly-soak`). It is cheaper, has no external dependency or disclosure clock, and is what produces the criterion-2 evidence — so it de-risks the eventual GO regardless.

## Packaging gap (what a GO would cost)

A GO is a small, conventional lift on top of the existing cargo-fuzz crate — already tracked as the remaining scope of `prod-adr-bolero-differential-libfuzzer` ("the libFuzzer wrapper binary + OSS-fuzz build manifest"), which is blocked on this decision. For the record, it is:

- a `project.yaml` (`language: rust`, `fuzzing_engines: [libfuzzer]`, sanitizers, a primary-contact email);
- a `Dockerfile` `FROM gcr.io/oss-fuzz-base/base-builder-rust` that clones the repo;
- a `build.sh` that runs `cargo +nightly fuzz build` and copies the target binaries into `$OUT`, plus per-target `$OUT/<target>_seed_corpus.zip` packaged from on-disk seeds (today the seeds are inline byte constants in `conformance/src/fuzz.rs`);
- a scoping call on `differential`: it needs `pg_query`/libpg_query (a native C build) and an external oracle, so an initial submission should ship only the two pure-Rust targets (`parse_no_panic`, `roundtrip`) and hold `differential` back until the native build and its triage load are justified.

## Consequences

- The ADR-0014 OSS-Fuzz deferral is now a recorded decision with a concrete, conjunctive revisit trigger — it cannot be silently forgotten.
- No new packaging files are added now; the existing cargo-fuzz targets remain the integration point and stay runnable locally on nightly.
- `prod-adr-bolero-differential-libfuzzer` stays blocked on a future GO for the OSS-Fuzz manifest half of its scope; its in-repo Bolero/libFuzzer work is unaffected.
- One follow-up is filed: `prod-fuzz-ci-nightly-soak` (the in-repo nightly soak that produces the crash-free evidence the revisit trigger needs).

## Interconnects

- code: `conformance/fuzz/fuzz_targets/parse_no_panic.rs`, `conformance/fuzz/fuzz_targets/differential.rs`, `conformance/fuzz/fuzz_targets/roundtrip.rs` — the cargo-fuzz targets a GO would onboard
- invariant: the fuzz targets exist but no OSS-Fuzz manifest (`project.yaml`/`Dockerfile`/`build.sh`) is present; if one appears, this ADR must flip to GO.
- xtask: none — absence guard on the OSS-Fuzz packaging files.

## References

ADR-0014 (fuzz layer + the original deferral), ADR-0015 (differential oracle + corpus licensing), ADR-0017 (dependency minimalism, local-runnable checks). Harness: `conformance/fuzz/`, `conformance/src/fuzz.rs`. OSS-Fuzz: <https://github.com/google/oss-fuzz>.
