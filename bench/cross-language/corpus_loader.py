# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Shared corpus loader for the cross-language throughput runners.

This is the Python half of the "same corpus, same segmentation" contract: it cuts
the three vendored conformance corpora into the EXACT same candidate statements,
in the EXACT same order, as the Rust harness (`bench/benches/corpus/mod.rs` and
`bench/benches/upstream/mod.rs`). Keeping the segmentation identical is what makes
the cross-language throughput numbers comparable across tools AND relatable to the
in-process Rust numbers: candidate `i` of corpus `c` is the same SQL string for
every runner, so an accept/reject manifest from one tool intersects cleanly with
another's by `<corpus>:<index>` id.

It deliberately mirrors the Rust segmentation rules rather than inventing new ones:

  * `sqlglot_identity`, `sqllogictest_statements` are LINE-PER-STATEMENT: every
    non-blank line is one candidate, kept verbatim (untrimmed), exactly like the
    Rust `Shape::LinePerStatement` (`lines().filter(non-empty-after-trim)`).
  * `postgres_regress_supported` is SEMICOLON-DELIMITED behind a leading
    `--`/blank SPDX header. The header is dropped WHOLESALE first (the header prose
    itself contains a ';' — "identifiers exactly; unquoted" — so a naive split
    would glue it onto the first statement), then the remainder is split on ';',
    each piece trimmed, empties dropped. This is the `upstream/mod.rs`
    `pg_regress_statements` rule.

The Java runners (`calcite_throughput.java`, `jsqlparser_throughput.java`) inline a
byte-for-byte port of these same three rules; `docs/performance.md`
is the spec all three implementations are checked against.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import List

# The three corpora, in a fixed order — the SAME order and keys as the Rust
# harness's `CORPORA` array, so ids line up across runners and across languages.
# (key, relative path under the corpus root, shape)
CORPORA = [
    ("sqlglot_identity", os.path.join("sqlglot", "identity.sql"), "line"),
    ("sqllogictest_statements", os.path.join("sqllogictest", "statements.sql"), "line"),
    ("postgres_regress_supported", os.path.join("postgres", "regress-supported.sql"), "semicolon"),
]


@dataclass(frozen=True)
class Candidate:
    """One corpus candidate statement plus its stable cross-runner id."""

    corpus: str
    index: int
    sql: str

    @property
    def id(self) -> str:
        # `<corpus>:<index>` — the manifest key every runner agrees on.
        return f"{self.corpus}:{self.index}"


def _split_line_per_statement(text: str) -> List[str]:
    """Every non-blank line, kept verbatim (matches Rust `LinePerStatement`)."""
    return [line for line in text.splitlines() if line.strip()]


def _split_semicolon(text: str) -> List[str]:
    """Drop the leading `--`/blank header wholesale, then split on ';'.

    Mirrors `upstream/mod.rs::pg_regress_statements`. The wholesale header drop
    (not a per-chunk comment strip) is load-bearing: the header contains a literal
    ';' inside its prose, so splitting first would corrupt the first statement.
    """
    pos = 0
    n = len(text)
    while pos < n:
        eol = text.find("\n", pos)
        line_end = n if eol == -1 else eol + 1
        stripped = text[pos:line_end].lstrip()
        # The header is the contiguous leading run of blank / `--`-comment lines;
        # the first real statement line ends it.
        if stripped and not stripped.startswith("--"):
            break
        pos = line_end
    body = text[pos:]
    return [chunk.strip() for chunk in body.split(";") if chunk.strip()]


def default_corpus_root() -> str:
    """`conformance/corpus`, resolved relative to this file's location.

    This script lives at `bench/cross-language/`, so the corpus tree is two levels
    up. An explicit `--corpus-root` or `$SQUONK_CORPUS_ROOT` overrides this.
    """
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.normpath(os.path.join(here, "..", "..", "conformance", "corpus"))


def load_candidates(corpus_root: str) -> List[Candidate]:
    """All candidates from all three corpora, in fixed corpus-then-source order."""
    out: List[Candidate] = []
    for key, rel, shape in CORPORA:
        path = os.path.join(corpus_root, rel)
        with open(path, "r", encoding="utf-8") as fh:
            text = fh.read()
        statements = (
            _split_line_per_statement(text) if shape == "line" else _split_semicolon(text)
        )
        for i, sql in enumerate(statements):
            out.append(Candidate(corpus=key, index=i, sql=sql))
    return out
