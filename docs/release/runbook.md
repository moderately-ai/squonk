<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# crates.io release runbook

Operational, no-surprises procedure for publishing the two Rust crates to crates.io.
The protected `release-crates.yml` workflow is the primary publisher. It verifies
freely, but registry writes require both an explicit `publish: true` dispatch from the
exact version tag and approval of the `crates-io` GitHub environment.

Version 1.0.0 completed this procedure on 2026-07-12. The current coordinated release
candidate is **2.0.0**; `v1.0.0` remains the historical 1.x baseline and
`v2.0.0` becomes the 2.x baseline at release.

## What publishes where

| Crate | Registry | Publishable | Notes |
| --- | --- | --- | --- |
| `squonk-ast` | crates.io | yes | The dialect-agnostic AST. Published **first** — `squonk` depends on it. |
| `squonk` | crates.io | yes | The tokenizer/parser. Published **second**, after `squonk-ast` is live (or coordinated in one `cargo publish` invocation, see below). |
| `squonk-python` | PyPI (via `maturin`) | no (`publish = false`) | Python wheel surface, not a crates.io crate. |
| `squonk-wasm` | npm package family | no (`publish = false`) | Seven `@squonk-sql/*` dialect packages plus the `squonk` umbrella; not a crates.io crate. |
| `squonk-sourcegen` | — | no (`publish = false`) | Internal dev codegen tool; never shipped. |

Only `squonk-ast` and `squonk` go to crates.io. `release/semver-baseline.toml` and `xtask/src/semver.rs` already encode exactly this published set.

## Publish order (load-bearing)

`squonk` declares `squonk-ast = { path = "…", version = "2.0.0" }`. crates.io resolves that `version` against the registry at upload time, so **`squonk-ast` 2.0.0 must exist on crates.io before `squonk` 2.0.0 uploads.** Publishing `squonk` first fails with `no matching package named 'squonk-ast' found`.

The workflow publishes sequentially: it publishes `squonk-ast`, waits until the exact
version and checksum appear in the sparse index, performs a normal registry-backed
dry-run of `squonk`, and only then publishes `squonk`. Re-running a partially completed
release is safe: an existing version is skipped only when its immutable registry
checksum matches the artifact rebuilt from the approved tag.

Cargo does **not** locally stage one unpublished workspace package for another during
`cargo publish --dry-run -p squonk-ast -p squonk`. Before `squonk-ast` 2.0.0 exists in
the registry, verification of the parser must use an explicit `[patch.crates-io]` path
to the candidate AST. The workflow does exactly that; after the AST reaches the index,
it repeats the parser dry-run without a patch.

## Pre-publish checklist (do all before the first upload)

- [ ] **Repository metadata.** Confirm shipped metadata points at `https://github.com/moderately-ai/squonk` and public branch URLs use `main`.
- [ ] **Version.** The workspace, Python, and npm build manifests all resolve to `2.0.0`. Confirm the historical `v1.0.0` baseline tag exists and create `v2.0.0` only from the final release commit.
- [ ] **Ownership.** Confirm the publishing account still owns `squonk` and `squonk-ast`, and that version `2.0.0` does not already exist.
- [ ] **Protected environment.** The repository has a `crates-io` GitHub environment
  with a required reviewer. Store a crates.io API token as its
  `CARGO_REGISTRY_TOKEN` environment secret. Scope the token to publishing `squonk`
  and `squonk-ast`; do not store it as a repository-wide secret.
- [ ] **Green tree.** On the exact release commit: `cargo fmt --all --check` clean, `cargo xtask preflight` green, working tree clean (no `--allow-dirty` on the real publish).
- [ ] **Same-day version re-check.** Inspect the registry records and confirm `2.0.0` is absent:
  ```sh
  # The JSON must list 1.0.0 but not 2.0.0 before this release.
  curl -s https://crates.io/api/v1/crates/squonk-ast
  curl -s https://crates.io/api/v1/crates/squonk
  ```

## Dry run (no upload — safe to run anytime)

The release workflow's `publish: false` dispatch is the authoritative rehearsal. The
equivalent local commands are:

```sh
cargo publish --dry-run -p squonk-ast
cargo publish --dry-run -p squonk \
  --config "patch.crates-io.squonk-ast.path='$PWD/crates/squonk-ast'"
```

The first command verifies the exact AST tarball. The second verifies the exact parser
source tarball against the candidate AST API. A final unpatched parser dry-run occurs
inside the publish job after the AST checksum is visible in crates.io.

To review the exact shipped file inventory without building:

```sh
cargo package --list -p squonk-ast
cargo package --list -p squonk
```

The shipped set is a deliberate `include` allowlist in each `Cargo.toml` (library sources, examples for `squonk`, integration tests, README, and a crate-local `LICENSE`). Test corpora and build artifacts do not ship.

## Real publish

1. Create the annotated `v2.0.0` tag at the reviewed release commit and push it.
2. Confirm the tag-triggered `release-crates` verification run is green and review its
   `crates-io-candidates` artifact and file inventories.
3. Dispatch `release-crates.yml` from ref `v2.0.0` with `publish: true`.
4. Approve the protected `crates-io` environment only after confirming the run SHA and
   version. The job publishes in dependency order and verifies both index checksums.

The sequential local commands remain an emergency fallback, but CI is the recorded,
reviewed, least-privilege release path.

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

## Version lineage

The first stable version is **`1.0.0`** and the current release candidate is **`2.0.0`**.
The workspace, Python wheel, seven scoped npm facades, npm umbrella, and eight native npm
platform packages move in lockstep. The `1.0.0` baseline is a hard commitment—no breaking
public-API change is permitted before `2.0.0`.

### Semver baseline rotation (at 2.0.0)

Each major line is checked against its own first release. The workspace major and
`release/semver-baseline.toml` must agree: major 2 requires `v2.0.0`. This prevents
future 2.x checks from comparing against `v1.0.0`, which would classify every change
as a permitted major change and skip the compatibility lints.

The `.0.0` release necessarily establishes rather than consumes its baseline. Use
this ordering:

1. Land the reviewed `2.0.0` release commit with `baseline_tag = "v2.0.0"`.
2. Create the annotated `v2.0.0` tag at exactly that commit.
3. Run `cargo xtask semver`. On the release commit this is an identity comparison;
   on every later 2.x commit it checks both published crates against that immutable
   tag with all features.
4. Publish only after the tag-backed gate passes.

Before step 2 the command fails because `v2.0.0` does not exist. That is the expected
pre-tag bootstrap state, not permission to publish. `cargo xtask preflight` remains
independent so the release commit can land before its tag is created. At the next
major, rotate the manifest to `v3.0.0` in the 3.0 release commit and repeat this
procedure. Historical baseline tags are never moved or deleted.
