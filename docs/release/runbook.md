<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# crates.io release runbook

Operational, no-surprises procedure for publishing the two Rust crates to crates.io. Every real `cargo publish` is individually human-gated: the maintainer confirms each upload. Nothing in this file should be automated end-to-end.

## What publishes where

| Crate | Registry | Publishable | Notes |
| --- | --- | --- | --- |
| `squonk-ast` | crates.io | yes | The dialect-agnostic AST. Published **first** — `squonk` depends on it. |
| `squonk` | crates.io | yes | The tokenizer/parser. Published **second**, after `squonk-ast` is live (or coordinated in one `cargo publish` invocation, see below). |
| `squonk-python` | PyPI (via `maturin`) | no (`publish = false`) | Python wheel surface, not a crates.io crate. |
| `squonk-wasm` | npm package family | no (`publish = false`) | Six `@squonk/*` dialect packages plus the `squonk` umbrella; not a crates.io crate. |
| `squonk-sourcegen` | — | no (`publish = false`) | Internal dev codegen tool; never shipped. |

Only `squonk-ast` and `squonk` go to crates.io. `release/semver-baseline.toml` and `xtask/src/semver.rs` already encode exactly this published set.

## Publish order (load-bearing)

`squonk` declares `squonk-ast = { path = "…", version = "1.0.0" }`. crates.io resolves that `version` against the registry at upload time, so **`squonk-ast` must exist on crates.io before `squonk` uploads.** Publishing `squonk` first fails with `no matching package named 'squonk-ast' found`.

Two supported ways to satisfy the order:

1. **Coordinated workspace publish (preferred, cargo ≥ 1.66).** `cargo publish -p squonk-ast -p squonk` packages both, verifies `squonk` against a locally-staged `squonk-ast`, then uploads `squonk-ast`, waits for the index, and uploads `squonk`. One command, correct order, no manual wait.
2. **Sequential.** Publish `squonk-ast`, wait for it to appear in the sparse index, then publish `squonk` separately. Use this if the two uploads need separate maintainer gates on separate days.

## Pre-publish checklist (do all before the first upload)

- [ ] **Repository metadata.** Confirm shipped metadata points at `https://github.com/moderately-ai/squonk` and public branch URLs use `main`.
- [ ] **Version.** `1.0.0` is ratified and the workspace is already bumped (see "Version number" below). Confirm the `v1.0.0` git tag has been created at the release commit — the semver gate requires it once the major is ≥ 1.
- [ ] **Names.** Per the ratified decision, there are **no placeholder reservations** — the first real `1.0.0` publish is what claims `squonk` and `squonk-ast` on crates.io. Re-verify both names are free the same day (the availability re-check below) and that the publishing account will own them.
- [ ] **Token.** `cargo login <token>` with a token scoped to publish, owned by the account that owns both names.
- [ ] **Green tree.** On the exact release commit: `cargo fmt --all --check` clean, `cargo xtask preflight` green, working tree clean (no `--allow-dirty` on the real publish).
- [ ] **Same-day availability re-check.** Re-confirm the names/versions are still free the day of release:
  ```sh
  # 404 == not yet published (expected for a first release of this version)
  curl -sI https://index.crates.io/sq/uo/squonk-ast
  curl -sI https://index.crates.io/sq/uo/squonk
  # Or the JSON API (404 body == name unclaimed):
  curl -s https://crates.io/api/v1/crates/squonk-ast
  curl -s https://crates.io/api/v1/crates/squonk
  ```

## Dry run (no upload — safe to run anytime)

This is the honest pre-flight; it packages, verifies the dependent against the staged dependency, and aborts before upload. On a clean tree drop `--allow-dirty`.

```sh
cargo publish --dry-run -p squonk-ast -p squonk
```

Expected tail:

```
Packaged 65 files, 5.0MiB (…KiB compressed)   # squonk-ast
Packaged 92 files, 4.2MiB (…KiB compressed)   # squonk
Verifying squonk-ast v…
Verifying squonk v…
Uploading squonk-ast v…
warning: aborting upload due to dry run
Uploading squonk v…
warning: aborting upload due to dry run
```

To review the exact shipped file inventory without building:

```sh
cargo package --list -p squonk-ast
cargo package --list -p squonk
```

The shipped set is a deliberate `include` allowlist in each `Cargo.toml` (library sources, examples for `squonk`, integration tests, README, and a crate-local `LICENSE`). Test corpora and build artifacts do not ship.

## Real publish (each upload is a separate maintainer gate)

> **GATE.** The maintainer explicitly confirms before each `cargo publish` without `--dry-run`. Do not chain these.

Preferred — coordinated:

```sh
# MAINTAINER CONFIRMS → then run:
cargo publish -p squonk-ast -p squonk
```

Sequential fallback (separate gates):

```sh
# MAINTAINER CONFIRMS squonk-ast → then run:
cargo publish -p squonk-ast

# Wait for it to land in the index (poll until 200):
until curl -sfI https://index.crates.io/sq/uo/squonk-ast >/dev/null; do sleep 5; done

# MAINTAINER CONFIRMS squonk → then run:
cargo publish -p squonk
```

## Post-publish verification

1. **Index presence:** `curl -sI https://index.crates.io/sq/uo/squonk-ast` and `.../squonk` return `200`.
2. **docs.rs build:** watch `https://docs.rs/crate/squonk-ast/<version>/builds` and `https://docs.rs/crate/squonk/<version>/builds`. Both crates set `[package.metadata.docs.rs] all-features = true` + `--cfg docsrs`, so the feature-gate banners must render and the all-features build must succeed. A red docs.rs build is fixable with a patch release; it does not require a yank.
3. **Install smoke test:** run `docs/release/smoke-test.sh` (below). It only works **after** the crates are live on crates.io.

## Install smoke test

`docs/release/smoke-test.sh` creates a throwaway project, `cargo add squonk`, and compiles+runs a parse. It resolves `squonk` (and transitively `squonk-ast`) from crates.io, proving the published artifacts install cleanly from a clean cache. It fails until both crates are published.

## Rollback / yank

crates.io publishes are **immutable** — a version can never be overwritten or deleted, only *yanked*. Yank stops new dependents from selecting the version; it does not break existing `Cargo.lock`s.

```sh
# Stop new selections of a bad release:
cargo yank --version <version> squonk
cargo yank --version <version> squonk-ast   # yank the dependency too if it is the fault

# Undo a yank (re-allow selection):
cargo yank --version <version> --undo squonk
```

Recovery from a bad publish is **a new patch version**, not an edit: yank the bad one, fix, bump the patch, republish. Yank `squonk` before `squonk-ast` (unblock the dependency last) so no window leaves `squonk` selectable against a yanked `squonk-ast`.

## Version number — ratified: `1.0.0`

The first stable version is **`1.0.0`**. The workspace, Python wheel, six scoped npm packages, and npm umbrella move in lockstep. `1.0.0` is a hard commitment — no breaking public-API change without a `2.0`.

### Semver gate bootstrap (at 1.0.0)

The stable-release infrastructure is hard-wired to `v1.0.0`, and the gate has a deliberate bootstrap ordering:

- `xtask/src/semver.rs` is **inert below 1.0.0** — at major `0` it prints `pre-stable workspace` and returns success. At major ≥ 1 it *activates*: it requires the `v1.0.0` baseline git tag and `cargo-semver-checks`, then compares each published crate against the baseline.
- `release/semver-baseline.toml` pins `baseline_tag = "v1.0.0"` (the xtask rejects any other baseline).
- `docs/stable-api.md`: the SemVer policy starts at the `v1.0.0` tag.

**The chicken-and-egg, and the resolution.** At `1.0.0` *before* the tag exists, `cargo xtask semver` exits non-zero with `stable workspace requires baseline tag v1.0.0` (observed: exit 2). This is expected and not a blocker, because **`semver` is not a preflight step** — it never runs in `cargo xtask preflight`, `ci.yml`, or the everyday gate stack, so a missing baseline tag does not block landing the 1.0.0 bump or any subsequent PR. It is a dedicated release-gating command. The bootstrap order is therefore:

1. **Land the `1.0.0` version bump.** The gate now *reports as active* but has no baseline to compare against yet — expected, non-blocking.
2. **At cutover, create the annotated `v1.0.0` tag** at the release commit. This tag *is* the immutable API baseline.
3. From that point, `cargo xtask semver` compares the published crates (`squonk-ast`, `squonk`) against `v1.0.0`. On the release commit itself the comparison is a no-op identity check (HEAD == the tag), so it passes once `cargo-semver-checks` is installed (`cargo install cargo-semver-checks --locked`).
4. Every future `1.x` release runs the gate against the frozen `v1.0.0` baseline; a breaking API change fails it and demands a `2.0`.

So the very first release establishes the baseline rather than being checked against one — there is nothing earlier to compare to. The gate protects `1.0.1` onward.
