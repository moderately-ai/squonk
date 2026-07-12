#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//
// RESEARCH ARTIFACT — NOT a shipping example, NOT a benchmark. It is the sketch called
// for by the formatter-API spike: "can a third party build a pretty-printer on our
// public wasm/JS API today, and where do they hit walls?" It attempts to indent one
// SELECT and DELIBERATELY runs into the friction, which it prints. The walls it finds
// are the evidence behind the recommendation in
// `docs/performance.md` (Formatter-API spike section).
//
// Run AFTER building the wasm package:
//   (cd crates/squonk-wasm && npm install && npm run build)
//   node bench/cross-language/formatter_sketch.mjs
//
// It imports the FULL variant (every dialect + document render) to give the sketch the
// most surface to work with; the walls below are not variant-specific.

import { readFile } from "node:fs/promises";
import { defaultWasmUrl, init, parse, render } from "../../crates/squonk-wasm/js/full.js";

await init(await readFile(defaultWasmUrl));

const SQL = "select a, b, count(*) from t where a > 1 and b < 2 group by a, b -- keep\norder by a";
const doc = parse(SQL, { dialect: "postgres" });

console.log("== input ==");
console.log(SQL);

// ---------------------------------------------------------------------------------
// WALL 1: render() gives ONE flat, single-line canonical string. There is no
// indentation / line-break / max-width knob in RenderOptions ({ dialect?, mode? } only,
// mode in canonical|redacted|parenthesized). So the "format" you get is a normalizer,
// not a pretty-printer.
console.log("\n== render() canonical (the only layout the API offers) ==");
console.log(render(SQL, { dialect: "postgres" })); // one line, no newlines/indent

// ---------------------------------------------------------------------------------
// WALL 2: comments are GONE. The parser discards trivia from the token stream at lex
// time; the AST never carries it and render() never re-emits it. The `-- keep` above
// does not survive the round-trip above. `parse(..., { captureTrivia: true })` records
// trivia spans in a SEPARATE side-array, but there is no API that feeds trivia back
// into render — a formatter would have to re-insert comments itself by span arithmetic.
const withTrivia = parse(SQL, { dialect: "postgres", captureTrivia: true });
console.log("\n== trivia is a side-channel, not attached to nodes ==");
// Shape check: trivia (if exposed on the raw payload) is a flat list of {kind, span},
// decoupled from any node. A formatter must re-associate each comment span with the
// nearest node span to decide WHERE to place it — non-trivial, and lossy for
// comments that sit between clauses.
try {
  const raw = withTrivia.raw ?? {};
  console.log("raw trivia field present:", Array.isArray(raw.trivia), "(flat, span-keyed, node-detached)");
} catch (e) {
  console.log("could not introspect trivia payload:", e.message);
}

// ---------------------------------------------------------------------------------
// WALL 3: you cannot render a SUB-node to a SQL fragment. To place a WHERE clause on
// its own indented line you need "the canonical SQL for JUST this sub-tree". The API
// does not offer it:
//   * `render()` accepts only a full SQL string or a full Document.
//   * `Node.toSQL()` renders the node's OWNING DOCUMENT (the whole statement), not the
//     node — see crates/squonk-wasm/js/runtime.d.ts (Node.toSQL doc comment).
//   * `Node.sourceText()` returns the ORIGINAL source slice for the span (with the
//     original whitespace/case), which is not canonicalized and defeats the point of
//     reformatting; and sub-node source spans nest/overlap, so you cannot cleanly
//     recompose a statement from sliced sub-node sources.
// So a would-be formatter is pushed to walk the raw AST JSON (`node.raw`, `node.get`)
// and RE-IMPLEMENT the spelling of every node kind in JS — i.e. rebuild the renderer
// that already exists in Rust. Demonstrate the wall:
console.log("\n== attempt: indent each top-level clause by rendering sub-nodes ==");
const stmt = doc.children()[0] ?? [...doc.walk()][0];
let hitWall = false;
for (const child of (stmt ? stmt.children() : [])) {
  const frag = child.toSQL(); // NOTE: renders the whole document, not `child`
  const whole = render(SQL, { dialect: "postgres" });
  if (frag === whole) {
    hitWall = true;
    console.log(`  node kind=${child.kind}: toSQL() returned the WHOLE statement, not the clause -> cannot indent per-clause`);
    break;
  }
}
if (!hitWall) console.log("  (walk produced no children to probe — see WALL 3 comment for the API limitation)");

// ---------------------------------------------------------------------------------
// The closest thing a consumer CAN do today, and why it is not a formatter:
//   1. `render()` to normalize keyword case + spacing + minimal parens (canonical), then
//   2. a PURELY TEXTUAL post-pass that inserts newlines before top-level keywords
//      (FROM/WHERE/GROUP BY/...) found by string scanning.
// That is a regex-on-canonical-SQL hack: it is not dialect-aware about where keywords
// may legally appear (a keyword inside a string literal or a subquery breaks it), it
// cannot restore comments, and it re-derives structure the parser already knew and threw
// away at the JS boundary. It is exactly the fragile "tokenize-and-relayout" approach we
// are BETTER than — so building it on top of us throws away our advantage.
console.log("\n== textual post-pass hack (NOT recommended; shown to mark the ceiling) ==");
const canonical = render(SQL, { dialect: "postgres" });
const KW = ["FROM", "WHERE", "GROUP BY", "ORDER BY", "HAVING", "LIMIT"];
let pretty = canonical;
for (const kw of KW) pretty = pretty.replaceAll(` ${kw} `, `\n  ${kw} `);
console.log(pretty);
console.log("\n(brittle: string-level keyword scan, comment-lossy, not span/AST-aware — the wall the spike documents)");
