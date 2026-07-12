// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import {
  Document,
  Ident,
  ObjectName,
  parse,
  redact,
  SqlParseError,
  tokenize,
  transpile,
  type WrappedAstValue,
} from "../../../js/postgres.js";

const sql =
  process.argv.slice(2).join(" ") ||
  "SELECT u.id, u.email FROM public.users AS u WHERE u.id = $1";

try {
  const document = parse(sql, {
    dialect: "postgres",
    captureTrivia: true,
  });

  console.log(`dialect: ${document.dialect}`);
  console.log(`statements: ${document.statements.length}`);
  console.log(`canonical: ${document.toSQL()}`);
  console.log(`redacted: ${redact(document)}`);
  console.log(`transpiled: ${transpile(sql, {
    sourceDialect: "postgres",
    targetDialect: "postgres",
  })}`);

  const identifiers = Array.from(document.findAll(Ident), (ident) => ({
    text: ident.text,
    line: oneBasedLine(document, ident),
    source: ident.sourceText(),
  }));
  console.table(identifiers);

  const tableNames = Array.from(document.findAll("Table"))
    .map((node) => objectNameText(node.get("name")))
    .filter((name): name is string => name !== null);
  console.log(`tables: ${tableNames.join(", ") || "(none)"}`);

  const tokenized = tokenize(sql, {
    dialect: "postgres",
    includeTrivia: true,
  });
  console.log(`tokens: ${tokenized.tokens.length}`);
  console.log(`trivia: ${tokenized.trivia?.length ?? 0}`);
} catch (error) {
  if (error instanceof SqlParseError) {
    console.error(`${error.kind}: ${error.message}`);
    console.error(error.span ? sourceAt(sql, error.span) : "no source span");
    process.exitCode = 1;
  } else {
    throw error;
  }
}

function objectNameText(value: WrappedAstValue): string | null {
  return value instanceof ObjectName ? value.text : null;
}

function oneBasedLine(document: Document, ident: Ident): number | null {
  const location = ident.location();
  return location ? location.line + 1 : null;
}

function sourceAt(source: string, span: { start: number; end: number }): string {
  const bytes = new TextEncoder().encode(source);
  return new TextDecoder().decode(bytes.slice(span.start, span.end));
}
