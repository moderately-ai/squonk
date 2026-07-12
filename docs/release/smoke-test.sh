#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.
#
# Post-publish install smoke test. Creates a throwaway crate, pulls `squonk`
# (and transitively `squonk-ast`) FROM crates.io, and compiles + runs a parse.
# This proves the published tarballs install and build from a clean registry.
#
# It ONLY works after both crates are live on crates.io — before that,
# `cargo add squonk` fails to resolve. Not part of the pre-publish gate.
#
# Usage:
#   docs/release/smoke-test.sh            # latest published version
#   docs/release/smoke-test.sh 1.0.0      # pin a specific version

set -euo pipefail

VERSION="${1:-}"
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

echo "smoke-test: scratch project at $WORKDIR"
cd "$WORKDIR"

cargo new --bin squonk-smoke >/dev/null
cd squonk-smoke

if [ -n "$VERSION" ]; then
  cargo add "squonk@$VERSION"
else
  cargo add squonk
fi

cat > src/main.rs <<'RUST'
use squonk::parse;

fn main() {
    let parsed = parse("select 1 + 2").expect("well-formed SQL parses");
    assert_eq!(parsed.statements().len(), 1);
    let rendered = parsed.to_string();
    assert_eq!(rendered, "SELECT 1 + 2");
    println!("smoke-test OK: {rendered}");
}
RUST

# `--locked` is intentionally omitted: there is no committed lockfile to honour in
# this throwaway project, and we want a fresh resolve straight from the registry.
cargo run

echo "smoke-test: PASS"
