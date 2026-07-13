// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { readFileSync } from "node:fs";

import {
  createSquonkApi,
  type CanonicalDialectName,
  type SquonkApi,
  type WasmBindings,
} from "./runtime.js";

type WasmInitSync = (
  input: { module: BufferSource | WebAssembly.Module } | BufferSource | WebAssembly.Module,
) => unknown;

/** Create the portable synchronous backend used when native addons are disabled or unavailable. */
export function createNodeWasmSquonk<
  const TSupportedDialects extends readonly CanonicalDialectName[],
  const TDefaultDialect extends TSupportedDialects[number],
>(
  initSync: WasmInitSync,
  wasm: WasmBindings,
  wasmUrl: URL,
  options: {
    readonly defaultDialect: TDefaultDialect;
    readonly supportedDialects: TSupportedDialects;
  },
): SquonkApi<TSupportedDialects[number], TDefaultDialect> {
  try {
    initSync({ module: readFileSync(wasmUrl) });
  } catch (error) {
    throw new Error(`failed to initialize Squonk wasm from ${wasmUrl.href}`, { cause: error });
  }
  const host = "Deno" in globalThis ? "deno" : "node";
  return createSquonkApi(wasm, { ...options, runtime: { backend: "wasm", host } });
}
