// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { readFileSync } from "node:fs";
import { createSquonkApi, } from "./runtime.js";
/** Create the portable synchronous backend used when native addons are disabled or unavailable. */
export function createNodeWasmSquonk(initSync, wasm, wasmUrl, options) {
    try {
        initSync({ module: readFileSync(wasmUrl) });
    }
    catch (error) {
        throw new Error(`failed to initialize Squonk wasm from ${wasmUrl.href}`, { cause: error });
    }
    const host = "Deno" in globalThis ? "deno" : "node";
    return createSquonkApi(wasm, { ...options, runtime: { backend: "wasm", host } });
}
