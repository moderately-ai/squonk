---
title: Transpiling between SQL dialects
description: Convert SQL from one dialect to another — parse under a source dialect and render under a target dialect in a single call with squonk's SQL transpiler.
---

**Transpiling** is parsing SQL under one dialect and rendering it under another.
Because squonk's AST is dialect-agnostic, converting a query between engines is a
single call — no string rewriting, no regex.

## Transpile in one call

```ts
import { transpile } from "squonk";

const mysqlSql = transpile("SELECT id FROM t LIMIT 10", {
  sourceDialect: "postgres",
  targetDialect: "mysql",
});
```

The source dialect controls how the input is parsed; the target dialect controls
how the resulting AST is rendered. Either side defaults to the package's dialect
when omitted.

## The two-step equivalent

`transpile` is shorthand for a parse followed by a dialect-targeted render, which
you can also do explicitly when you want to inspect or transform the AST in
between:

```python
import squonk as sql

doc = sql.parse(query, dialect="postgres")
# ...inspect or rewrite the tree...
out = doc.to_sql(dialect="mysql")
```

## Rendering modes

Rendering — and therefore transpiling — understands context. You can emit
canonical SQL, fully-parenthesized SQL, or PII-redacted SQL for the target
dialect, so the same pipeline covers dialect conversion, normalization, and
redaction.

## Try it

The [playground](/playground/) transpiles SQL between any two built-in dialects
live in your browser — paste a query, pick a source and target, and see the
converted output.
