// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import initWasm, * as wasm from "../pkg-quiltdb/squonk_wasm.js";
import { createBrowserSquonk } from "./runtime.js";
export type * from "./runtime.js";
export type * from "../js/ast.generated.js";
export { Diagnostic, Document, Ident, Node, ObjectName, RecoveredDocument, SqlParseError } from "./runtime.js";
export const createSquonk = createBrowserSquonk(initWasm, wasm,
  new URL("../pkg-quiltdb/squonk_wasm_bg.wasm", import.meta.url),
  { defaultDialect: "quiltdb", supportedDialects: ["ansi", "quiltdb"] as const });
