// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { createSquonkApi, } from "./runtime.js";
/** Create a synchronous backend from a module supplied by an edge runtime's bundler. */
export function createModuleWasmSquonk(initSync, wasm, module, options, host) {
    initSync({ module });
    return createSquonkApi(wasm, { ...options, runtime: { backend: "wasm", host } });
}
