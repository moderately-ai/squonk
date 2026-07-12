// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

// Shared corpus loader for the Node cross-language throughput runner.
//
// This is the JavaScript half of the "same corpus, same segmentation" contract
// that `corpus_loader.py` (sqlglot) and the inlined Java ports (Calcite,
// JSQLParser) already implement. It cuts the three vendored conformance corpora
// into the EXACT same candidate statements, in the EXACT same order, as the Rust
// harness (`bench/benches/corpus/mod.rs`, `bench/benches/upstream/mod.rs`) and the
// Python loader, so candidate `i` of corpus `c` is the identical SQL string in
// every runner and a `<corpus>:<index>` accept id from one tool intersects
// another's. `docs/performance.md` is the spec all
// implementations are checked against.
//
// Segmentation rules (a deliberate port of the Rust rules, matching the Python):
//   * `sqlglot_identity`, `sqllogictest_statements` — LINE-PER-STATEMENT: every
//     non-blank line is one candidate, kept verbatim (untrimmed), exactly like the
//     Rust `Shape::LinePerStatement` and the Python `_split_line_per_statement`.
//   * `postgres_regress_supported` — SEMICOLON-DELIMITED behind a leading
//     `--`/blank SPDX header. The header is dropped WHOLESALE first (its prose
//     itself contains a ';' — "identifiers exactly; unquoted" — so a naive split
//     would glue it onto the first statement), then the remainder is split on ';',
//     each piece trimmed, empties dropped. This is `upstream/mod.rs`'s
//     `pg_regress_statements` rule and the Python `_split_semicolon`.

import { readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

// The three corpora, in the SAME fixed order and with the SAME keys as the Rust
// harness's `CORPORA` array and `corpus_loader.py`, so ids line up across runners.
// [key, relative path under the corpus root, shape].
export const CORPORA = [
  ["sqlglot_identity", join("sqlglot", "identity.sql"), "line"],
  ["sqllogictest_statements", join("sqllogictest", "statements.sql"), "line"],
  ["postgres_regress_supported", join("postgres", "regress-supported.sql"), "semicolon"],
];

// `<corpus>:<index>` — the manifest key every runner agrees on.
export function candidateId(corpus, index) {
  return `${corpus}:${index}`;
}

// Every non-blank line, kept verbatim (matches Rust `LinePerStatement` and the
// Python `_split_line_per_statement`). Split on \r?\n so a CRLF checkout segments
// identically; the kept line is NOT trimmed, matching the other loaders.
function splitLinePerStatement(text) {
  return text.split(/\r?\n/).filter((line) => line.trim() !== "");
}

// Drop the leading `--`/blank header wholesale, then split on ';'. Mirrors
// `upstream/mod.rs::pg_regress_statements` and the Python `_split_semicolon`. The
// wholesale header drop (not a per-chunk comment strip) is load-bearing: the
// header contains a literal ';' inside its prose, so splitting first would corrupt
// the first statement.
function splitSemicolon(text) {
  let pos = 0;
  const n = text.length;
  while (pos < n) {
    const eol = text.indexOf("\n", pos);
    const lineEnd = eol === -1 ? n : eol + 1;
    const stripped = text.slice(pos, lineEnd).replace(/^\s+/, "");
    // The header is the contiguous leading run of blank / `--`-comment lines; the
    // first real statement line ends it.
    if (stripped !== "" && !stripped.startsWith("--")) {
      break;
    }
    pos = lineEnd;
  }
  return text
    .slice(pos)
    .split(";")
    .map((chunk) => chunk.trim())
    .filter((chunk) => chunk !== "");
}

// `conformance/corpus`, resolved relative to this file. This script lives at
// `bench/cross-language/`, so the corpus tree is two levels up. An explicit
// `--corpus-root` or `$SQUONK_CORPUS_ROOT` overrides this.
export function defaultCorpusRoot() {
  const here = dirname(fileURLToPath(import.meta.url));
  return normalize(join(here, "..", "..", "conformance", "corpus"));
}

// All candidates from all three corpora, in fixed corpus-then-source order. Each
// candidate is `{ corpus, index, sql, id }`.
export function loadCandidates(corpusRoot) {
  const out = [];
  for (const [key, rel, shape] of CORPORA) {
    const text = readFileSync(join(corpusRoot, rel), "utf8");
    const statements = shape === "line" ? splitLinePerStatement(text) : splitSemicolon(text);
    statements.forEach((sql, index) => {
      out.push({ corpus: key, index, sql, id: candidateId(key, index) });
    });
  }
  return out;
}
