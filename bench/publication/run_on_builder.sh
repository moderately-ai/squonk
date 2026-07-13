#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

set -euo pipefail

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
    echo "publication benchmarks require the canonical Linux x86_64 builder" >&2
    exit 1
fi

python -m venv --clear .publication-venv
.publication-venv/bin/python -m pip install --disable-pip-version-check \
    --requirement bench/publication/requirements.txt
rm -rf target/publication-python
.publication-venv/bin/maturin build --release \
    --out target/publication-python \
    --manifest-path crates/squonk-python/Cargo.toml
.publication-venv/bin/python -m pip install --disable-pip-version-check \
    --force-reinstall --no-deps target/publication-python/squonk-*.whl

npm ci --prefix bench/publication --ignore-scripts
npm run --prefix crates/squonk-wasm build:native
cargo build --release -p squonk-bench --example publication_adapter

.publication-venv/bin/python bench/publication/run.py \
    --cpu 0 \
    --output bench/publication/results/headline.json
