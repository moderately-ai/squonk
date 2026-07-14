# Python distribution release runbook (`squonk` on PyPI)

The step-by-step procedure to cut a Python release of `squonk` (crate `squonk-python`, module `squonk._native`). Every publish step is individually maintainer-gated: the automation builds and install-smokes artifacts freely, but nothing reaches an index without an explicit human action. This runbook is the companion to [`docs/platform-support.md`](../platform-support.md) (which owns the platform tiers) and is exercised by [`.github/workflows/release-python.yml`](../../.github/workflows/release-python.yml) and the smoke script [`docs/release/smoke_python.py`](./smoke_python.py).

## Distribution facts

- **Distribution name:** `squonk` (PyPI project). **Import name:** `squonk`. **Native module:** `squonk._native` (an `abi3` extension).
- **ABI:** `cp311-abi3` — one stable-ABI wheel per platform, installable on CPython >= 3.11. Configured by the PyO3 `abi3`/`abi3-py311` features in `crates/squonk-python/Cargo.toml`; do not switch to per-version wheels without a reason.
- **Minimum Python:** 3.11 (`requires-python = ">=3.11"`).
- **License:** MIT, declared as a PEP 639 `License-Expression` with the file bundled from `crates/squonk-python/LICENSE`.

## Wheel matrix (authoritative v1 set)

Every wheel is `cp311-abi3` and is built **and** install-smoked on a native GitHub-hosted runner — no shipped wheel relies on emulation.

| Wheel platform tag | Target triple | Runner | Tier | Notes |
|---|---|---|---|---|
| `manylinux2014_x86_64` | `x86_64-unknown-linux-gnu` | `ubuntu-latest` | 1 — built + native smoke | glibc 2.17 floor (manylinux2014 container) |
| `macosx_11_0_arm64` | `aarch64-apple-darwin` | `macos-14` | 1 — built + native smoke | Apple silicon |
| `macosx_10_12_x86_64` | `x86_64-apple-darwin` | `macos-15-intel` | 1 — built + native smoke | Intel Mac |
| `win_amd64` | `x86_64-pc-windows-msvc` | `windows-latest` | 1 — built + native smoke | |
| sdist (`.tar.gz`) | — | `ubuntu-latest` | source fallback | compiled from source on install; needs a Rust toolchain on the target |

**Deferred (Tier-2 growth path, not shipped in v1):** Linux `aarch64` (manylinux/musllinux) and musllinux `x86_64`. They are omitted only because a QEMU-emulated install-smoke is weaker evidence than the native lanes above; add each with its own build+smoke leg in `release-python.yml` before advertising it (an OS/arch is never published ahead of a green lane).

### Native artifact size

The macOS arm64 wheel is ~4.0 MiB compressed / ~12.5 MiB unpacked, dominated by the `_native.abi3.so` extension (~12.3 MiB) that carries every built-in dialect (the crate builds `squonk` with `features = ["serde", "full"]`). This is within budget for a batteries-included parser and needs no split; revisit only if a "core-dialects-only" wheel is ever requested.

## Metadata verification (done, keep true)

`pyproject.toml` (`crates/squonk-python/pyproject.toml`) carries, and `twine check` must keep passing:

- `description`, long description from `README.md` (`readme = "README.md"`), `keywords`, `authors`.
- `license = "MIT"` + `license-files = ["LICENSE"]` (no deprecated license *classifier* — it conflicts with `License-Expression` under Metadata 2.4 and PyPI rejects the pair).
- `classifiers`: `Development Status :: 5 - Production/Stable`, the Python 3.11–3.14 + CPython classifiers, and the three OS classifiers matching the shipped wheels.
- `[project.urls]`: Homepage / Repository / Documentation / Issues.

The `[project.urls]` and workspace repository metadata point at the public
`moderately-ai/squonk` repository. Verify those URLs again before upload.

## Procedure

### 0. Availability re-check (maintainer gate #0)

The distribution name must be ours before anything is published. This is a re-check even if reserved earlier — squatting happens.

- Confirm the PyPI project `squonk` is registered/reserved to the Moderately AI org (or is free): `curl -sS -o /dev/null -w '%{http_code}\n' https://pypi.org/pypi/squonk/json` (404 = free, 200 = exists — verify it is *ours*).
- Confirm PyPI **Trusted Publishing** is configured for the `squonk` project pointing at this repo + the `release-python.yml` workflow + the `pypi` environment. No API token is stored — the workflow uses OIDC.
- **Gate:** do not proceed unless the name is ours and Trusted Publishing is wired.

### 1. Version + metadata (maintainer gate #1)

- Set the release version (workspace `version` in the root `Cargo.toml`; the wheel version is `dynamic` and derived from it). The stable version choice is owned by `stable-release-versioning-security-and-public-repo-cutover`.
- Verify the URL flip above is done and `Development Status` matches the version.
- **Gate:** review the diff; the version and metadata are what will be immutable on PyPI.

### 2. Build + verify locally (rehearsal)

From a clean checkout with `maturin>=1.7,<2.0` available (`uv sync --group dev` or `pip install 'maturin>=1.7,<2.0'`):

```bash
cd crates/squonk-python
maturin build --release --out ../../dist      # this host's native wheel
maturin sdist --out ../../dist
```

Verify each artifact:

```bash
unzip -l dist/squonk-*.whl        # module + typing files + _native.abi3.so + dist-info; LICENSE under dist-info/licenses/; no stray files
python -m twine check dist/*      # metadata validity — must PASS for wheel and sdist
```

Clean-install acceptance test (the release gate — a bare venv holding only the wheel):

```bash
python -m venv /tmp/squonk-smoke
/tmp/squonk-smoke/bin/pip install dist/squonk-*.whl
/tmp/squonk-smoke/bin/python docs/release/smoke_python.py     # must print "SMOKE OK"
```

The smoke script refuses to pass if it imported the source tree instead of the installed wheel, so run it from anywhere and trust the "SMOKE OK" line.

### 3. Build the full matrix in CI (rehearsal, no publish)

- Trigger `release-python.yml` via **workflow_dispatch** with `publish: false` (the default), or push the coordinated `v<version>` release tag. The legacy `python-v<version>` trigger remains build-only and cannot publish.
- Every matrix leg builds its wheel and runs the native install-smoke; `build-sdist` additionally compiles the sdist from source and smokes it.
- **Gate:** all four wheel legs + the sdist leg green, and the wheel/sdist artifacts present on the run, before considering a publish.

### 4. Publish to PyPI (maintainer gate #4 — the real, irreversible upload)

- Re-dispatch `release-python.yml` from the exact `v<version>` tag with `publish: true`; the publish job rejects branch refs and surface-specific tags.
- The `publish` job targets the protected `pypi` environment: **approve the required-reviewer prompt only when everything above is green.** A PyPI version is immutable and cannot be re-uploaded.
- Post-publish smoke in a clean venv against real PyPI:

```bash
python -m venv /tmp/squonk-pypi
/tmp/squonk-pypi/bin/pip install squonk
/tmp/squonk-pypi/bin/python docs/release/smoke_python.py     # must print "SMOKE OK"
```

- Verify the PyPI project page renders the README, shows the correct classifiers, license, and project URLs, and lists all promised wheels plus the sdist.

## Rollback / yank

PyPI uploads are immutable — you cannot overwrite a version. Recovery is:

- **Yank** the bad release (`pip` will not select a yanked version for new installs unless pinned exactly): via the PyPI project UI, or `twine`/PyPI API. Yanking, not deletion, is the correct response to a broken-but-installed release.
- **Deleting** a release/file is possible in the PyPI UI but frees the version number for reuse with *different* contents — avoid it; prefer a yank plus a new patch version.
- Ship the fix as a new patch version (e.g. `1.0.1`) and repeat this runbook from step 1. Never attempt to reuse a yanked version number.
- If a bad wheel reached users, note it in the changelog/security policy (owned by `stable-release-versioning-security-and-public-repo-cutover`).
