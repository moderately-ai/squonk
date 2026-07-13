// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { type CanonicalDialectName, type SquonkApi, type WasmBindings } from "./runtime.js";
type WasmInitSync = (input: {
    module: BufferSource | WebAssembly.Module;
} | BufferSource | WebAssembly.Module) => unknown;
/** Create the portable synchronous backend used when native addons are disabled or unavailable. */
export declare function createNodeWasmSquonk<const TSupportedDialects extends readonly CanonicalDialectName[], const TDefaultDialect extends TSupportedDialects[number]>(initSync: WasmInitSync, wasm: WasmBindings, wasmUrl: URL, options: {
    readonly defaultDialect: TDefaultDialect;
    readonly supportedDialects: TSupportedDialects;
}): SquonkApi<TSupportedDialects[number], TDefaultDialect>;
export {};
