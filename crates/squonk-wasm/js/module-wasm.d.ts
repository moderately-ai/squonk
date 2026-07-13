// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { type CanonicalDialectName, type RuntimeInfo, type SquonkApi, type WasmBindings } from "./runtime.js";
type WasmInitSync = (input: {
    module: WebAssembly.Module;
} | WebAssembly.Module) => unknown;
/** Create a synchronous backend from a module supplied by an edge runtime's bundler. */
export declare function createModuleWasmSquonk<const TSupportedDialects extends readonly CanonicalDialectName[], const TDefaultDialect extends TSupportedDialects[number]>(initSync: WasmInitSync, wasm: WasmBindings, module: WebAssembly.Module, options: {
    readonly defaultDialect: TDefaultDialect;
    readonly supportedDialects: TSupportedDialects;
}, host: RuntimeInfo["host"]): SquonkApi<TSupportedDialects[number], TDefaultDialect>;
export {};
