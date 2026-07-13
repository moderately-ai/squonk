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
npm ci --prefix bench/publication --ignore-scripts
cargo build --release -p squonk-bench --example publication_adapter

.publication-venv/bin/python bench/publication/run.py \
    --cpu 0 \
    --output bench/publication/results/headline.json
