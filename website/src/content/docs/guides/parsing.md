---
title: Parsing SQL to an AST
description: Parse SQL into a compact, owned abstract syntax tree with byte spans and stable node IDs, and recover from syntax errors instead of failing on the first one.
---

The core operation in squonk is turning a SQL string into an **abstract syntax
tree (AST)** — a structured, owned representation of the query you can inspect,
transform, and render back to SQL.

## A first parse

```python
import squonk as sql

doc = sql.parse("SELECT id, name FROM users WHERE id = 1", dialect="ansi")
```

`parse` returns a **document** that owns the whole tree. Nodes are `'static` and
carry a byte span plus a stable node ID, so the tree can be stored, sent across
threads, and mapped back to the exact source bytes it came from.

## Walking the tree

Every node type can be located structurally. To collect every identifier in a
statement:

```python
idents = [ident.text for ident in doc.find_all(sql.Ident)]
```

Because each node retains its span, an editor or linter can highlight the precise
range in the original source that produced a given node.

## Recovering from errors

A strict `parse` fails on the first syntax error. A **recovering** parse instead
preserves the valid statements in a script while reporting the invalid ones with
locations and structured diagnostics — one malformed query no longer discards the
rest of the batch:

```ts
import { parseRecovering } from "squonk";

const result = parseRecovering(script, { dialect: "postgres" });
// Valid statements are available; each diagnostic carries a span and message.
```

## Rendering back to SQL

An AST can be rendered back to a string — canonical, fully parenthesized, or
PII-redacted — and targeted at a specific dialect. Rendering under a *different*
dialect than you parsed is [transpilation](/guides/transpiling/).

## See also

- [Dialects](/guides/dialects/) — choosing the grammar the parser applies.
- [Transpiling](/guides/transpiling/) — render an AST under another dialect.
- The full node inventory lives in the [architecture reference](https://github.com/moderately-ai/squonk/blob/main/docs/architecture.md).
