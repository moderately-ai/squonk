// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import initWasm, * as wasm from "../pkg-ansi/squonk_wasm.js";
import { createBrowserSquonk } from "./runtime.js";
export { Diagnostic, Document, Ident, Node, ObjectName, RecoveredDocument, SqlParseError } from "./runtime.js";
export const createSquonk = createBrowserSquonk(initWasm, wasm, new URL("../pkg-ansi/squonk_wasm_bg.wasm", import.meta.url), { defaultDialect: "ansi", supportedDialects: ["ansi"] });
