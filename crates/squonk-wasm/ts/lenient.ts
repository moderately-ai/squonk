// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { initSync } from "../pkg-lenient/squonk_wasm.js";
import * as wasm from "../pkg-lenient/squonk_wasm.js";
import { createNodeSquonk } from "./node.js";

const api = createNodeSquonk(initSync, wasm, new URL("../pkg-lenient/squonk_wasm_bg.wasm", import.meta.url), {
  defaultDialect: "lenient", supportedDialects: ["ansi", "lenient"] as const,
});
export type * from "./runtime.js";
export type * from "../js/ast.generated.js";
export { SqlParseError, Document, RecoveredDocument, Node, Ident, ObjectName, Diagnostic } from "./runtime.js";
export const { isDialectName, assertDialectName, canonicalDialectName, parse, parseJson, parseWithLimit,
  parseRecovering, parseRecoveringJson, supportedDialects, tokenize, render, redact, format,
  transpile, version, schemaVersion, runtimeInfo } = api;
