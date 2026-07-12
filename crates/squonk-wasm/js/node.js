// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { readFileSync } from "node:fs";
import { createSquonkApi, } from "./runtime.js";
/** Initialize one colocated wasm artifact synchronously for a Node entrypoint. */
export function createNodeSquonk(initSync, wasm, wasmUrl, options) {
    try {
        initSync({ module: readFileSync(wasmUrl) });
    }
    catch (error) {
        throw new Error(`failed to initialize Squonk wasm from ${wasmUrl.href}`, { cause: error });
    }
    return createSquonkApi(wasm, options);
}
