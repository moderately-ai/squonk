# Platform support policy

This policy defines which operating systems and Rust targets the project supports at the `v1.0.0` stable release, and how strongly. It covers the three published surfaces — the Rust crates (`squonk`, `squonk-ast`), the Python wheel (`squonk-python`), and the wasm32 npm package (`squonk-wasm`) — because each ships to a different registry with a different platform contract. It is the companion to [stable-api.md](./stable-api.md): that document governs the *shape* of the API across versions; this one governs *where* the code is built and tested. Both are enforced by CI, not by convention.

The rule this policy exists to keep: a registry (crates.io / PyPI / npm) or a README must never advertise a platform that CI does not at least compile. Every promised tier below maps to a concrete, cheapest-representative check in `.github/workflows/`.

## Support tiers

- **Tier 1 — supported and tested.** CI builds *and runs the test suite* on this target on every landing. A test regression on a Tier 1 target blocks the release. This is the strongest guarantee.
- **Tier 2 — supported, compile-verified.** CI *compiles* the crates on this target on every landing, but does not run the full test suite there. The code is expected to work and is a supported platform, but platform-specific *runtime* behaviour is not asserted by CI. Bugs are accepted and fixed; they are just not caught pre-merge.
- **Tier 3 — best-effort.** No CI coverage. The code is portable pure Rust (one micro-dependency, `thin-vec`; no C toolchain — see ADR-0017), so these targets are expected to work, but nothing verifies it and no support is guaranteed.
- **Unsupported (v1).** Explicitly out of scope for the first stable release. Listed so the boundary is deliberate rather than accidental.

Promotion between tiers is a release-notes-worthy change and requires the higher tier's CI lane to be green first (see [Evolution](#evolution)).

## The matrix

| Surface | Target triple | OS / runner | Tier | CI check (cheapest representative) | Workflow · cadence |
|---|---|---|---|---|---|
| Rust crates | `x86_64-unknown-linux-gnu` | `ubuntu-latest` | 1 — tested | `cargo xtask preflight` (fmt→tidy→clippy→nextest→doc) | `ci.yml` · every push to `main` + every PR |
| Rust crates | `aarch64-apple-darwin` | `macos-latest` | 1 — tested | `cargo nextest run` on the core crates | `platform.yml` · push to `main` + weekly + dispatch |
| Rust crates | `x86_64-pc-windows-msvc` | `windows-latest` | 1 — tested | `cargo nextest run` on the core crates | `platform.yml` · push to `main` + weekly + dispatch |
| Rust crates | other 64-bit host targets (e.g. `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`) | — | 3 — best-effort | none | — |
| Rust crates | 32-bit / big-endian targets | — | Unsupported v1 | none | — |
| Python wheel | Linux / macOS / Windows pyo3 boundary | `ubuntu-latest` / `macos-latest` / `windows-latest` | compile-verified here; wheel tier owned by the distribution workflow | `cargo check -p squonk-python` (extension-module boundary compiles) | `ci.yml` (Linux, via preflight binding lane) + `platform.yml` (macOS/Windows) |
| npm bindings | Node-API 8 on macOS/Linux/Windows x64+arm64; `wasm32-unknown-unknown` on Node 22+, Bun, Deno, browsers, and Workers | native OS/architecture runners + `ubuntu-24.04` | 1 — built and packaged | eight native packages; eight facade packages; Node/Bun/Deno/workerd/browser smokes | `release-npm.yml` |

"Core crates" in the checks above means `squonk`, `squonk-ast`, `squonk-sourcegen`, `squonk-node`, and `squonk-wasm` (its native mirror; the `#[wasm_bindgen]` layer is `cfg(target_arch = "wasm32")`-gated, so a native build never needs a wasm toolchain). The conformance, bench, and oracle crates are excluded from the cross-OS lanes because they need a system `libduckdb` and a MySQL server, which stay nightly-only on Linux (`oracle-nightly.yml`).

### Why the size assertions are portable

The generated AST size budgets (`crates/squonk-ast/src/generated/size_asserts.rs`) are `#[cfg(target_pointer_width = "64")]`-gated, not architecture-gated, so they compile and pass identically on every 64-bit Tier 1 host (Linux x86_64, macOS arm64, Windows x86_64) and are simply absent on 32-bit `wasm32`. This is why the Linux, macOS, and Windows nextest lanes assert the same byte layout with no per-OS pin. The only architecture-branched pins in the repo are the allocation-count checks under `bench/`, which are not part of the published surface and run only where they are measured.

## Python wheel: coordinated with the distribution workflow

This policy compile-verifies the pyo3 extension boundary (`cargo check -p squonk-python`) on Linux, macOS, and Windows so a platform-specific break in the binding code is caught before the wheel job ever runs. It deliberately does **not** build, install, or import-smoke a wheel here — that, plus the wheel platform set, the `abi3` floor, sdist verification, and the non-alpha metadata bump, are documented in the Python distribution runbook, whose authoritative matrix, build/verify/publish procedure, and rollback policy live in [`docs/release/python-distribution.md`](./release/python-distribution.md). The v1 wheel set the release workflow builds and install-smokes on native GitHub runners is Linux `x86_64` (manylinux2014), macOS `arm64` (macos-14), macOS `x86_64` (macos-15-intel), and Windows `x86_64`, plus the sdist — all `cp311-abi3`; Linux `aarch64`/musllinux are its documented Tier-2 growth path (deferred because they would need an emulated smoke). The wheel build + install-smoke lanes live in `.github/workflows/release-python.yml`, gated behind a protected environment for publish. Accordingly `pyproject.toml` now carries the three `Operating System ::` classifiers matching those built wheels and `Development Status :: 5 - Production/Stable`.

## wasm / npm: coordinated with the distribution workflow

The wasm32 target is compile-verified as the focused ANSI shape and the all-14-dialect `dialects-full` shape. Every package always includes document rendering. The release workflow builds seven scoped packages plus the `squonk` umbrella and eight script-free Node-API platform packages. It tests native Node/Bun, Node `--no-addons`, permissionless Deno, workerd, asynchronous browser factories, ESM/CommonJS consumers, size budgets, and clean packed installs. GNU addons target the Node-supported glibc 2.28 floor; musl ships separately.

## Windows: Tier 1 (tested)

Windows x86_64 runs the full core-crate test suite on every landing. The `cross-os` job in `platform.yml` matrixes over operating system and task, so each macOS/Windows core test and pyo3 compile check has an independent runner. The 64-bit size assertions run and pass in the core-test lanes (they are `target_pointer_width`-gated, not architecture-gated).

The conformance, bench, and oracle crates are excluded from this lane, exactly as on the macOS lane: they need a build-time `libduckdb` and a running MySQL server, so no oracle engine is compiled or skipped cross-OS. That differential coverage stays nightly-only on Linux (`oracle-nightly.yml`), which keeps the cross-OS runners free of a C toolchain and a database service.

Two Windows-specific test hazards that a nextest lane would otherwise trip are handled so the suite runs clean on Windows:

1. **`canonicalize()` UNC prefix.** `squonk-sourcegen`'s test-only `TempTree::new` canonicalizes the temp dir (on Unix this resolves the `/tmp` → `/private/tmp` symlink). On Windows `canonicalize()` returns a `\\?\` verbatim-prefixed path; the sourcegen test helper `strip_verbatim_prefix` removes that prefix (`\\?\C:\x` → `C:\x`, `\\?\UNC\server\share` → `\\server\share`) before the substring path assertions, and is a no-op on the Unix path form, so the assertions hold on every host.
2. **CRLF drift.** The sourcegen drift test compares freshly generated bytes against the checked-in generated files via `read_to_string`. The repo's `.gitattributes` (`* text=auto eol=lf`) forces LF in the working tree on every platform, so a default `core.autocrlf=true` Windows checkout no longer rewrites the LF generated files to CRLF; the drift comparison additionally folds CRLF → LF as a content check, so it stays correct even if a pre-`.gitattributes` checkout or a user `core.autocrlf` override slips CRLF into the tree.

## CI enforcement and cadence

Two workflows enforce this matrix:

- **`ci.yml`** — the lean everyday gate on `ubuntu-latest` (runner cost multiplier 1×), on every push to `main` and every PR. It is the single source of Tier 1 Linux coverage and, through the preflight binding lane, the Linux pyo3 boundary check.
- **`platform.yml`** — the cross-platform matrices. Its two cheap `wasm32` feature-shape jobs run on `ubuntu-latest` on every push/PR (the wasm-only code path is worth catching per-PR at 1× cost). Its cross-OS matrix gives the core tests and pyo3 compile check separate macOS and Windows runners; those expensive lanes (`macos-latest` at 10×, `windows-latest` at 2×) are skipped on pull requests and run on push to `main` (every landing), a weekly schedule (a backstop against toolchain drift with no intervening landing), and manual dispatch.

The cadence split is a deliberate cost decision: paying the 10× macOS multiplier on every push of every PR iteration is not justified when the lean Linux gate already covers the common case and cross-OS regressions are caught at merge to `main`. This mirrors the repo's existing separation of everyday gates (`ci.yml`) from the heavier nightly soaks (`fuzz-nightly.yml`, `oracle-nightly.yml`).

## Evolution

- Adding a target at any tier is a minor, release-notes-worthy change. Removing a target or demoting its tier is reviewed like a breaking change, because downstream builds depend on it.
- A target reaches **Tier 1** only once a `cargo nextest` lane for it is committed and green; a target reaches **Tier 2** only once a `cargo check` lane for it is committed and green. A tier is never advertised (in a README, a registry classifier, or an npm field) ahead of its lane existing — that inversion is exactly what this policy forbids.
- The workspace `rust-version` (MSRV) interacts with this policy: the CI lanes use the repository's pinned stable toolchain and do not rebuild under older toolchains. Raising the MSRV follows the [stable-api.md](./stable-api.md) rule.

## No over-promising: registry metadata audit

Audited at the time of writing against this matrix:

- **`README.md`** — the install snippets (`cargo add`, `pip install`, `npm install`) carry no OS qualification, so a "Platform support" pointer to this document was added to contextualize them. No claim advertises an OS the matrix never builds.
- **`crates/squonk-python/pyproject.toml`** — the three `Operating System ::` classifiers (Linux/macOS/Windows) match exactly the wheels `release-python.yml` builds and install-smokes, so nothing over-promises a wheel platform. `Development Status :: 5 - Production/Stable` matches the 1.0 release.
- **npm package manifests** — the checked-in build manifest is private; staged publish manifests declare Node 22+ and expose a separate browser entrypoint. Publishing metadata is described in the npm distribution runbook.
