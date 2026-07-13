// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { createSquonkApi, } from "./runtime.js";
const require = createRequire(import.meta.url);
/**
 * Resolve the prebuilt Node-API engine for the current platform.
 *
 * Local development uses `../native/squonk.node`; published packages resolve one
 * platform-selected optional dependency. Failure is intentionally non-fatal so an
 * unsupported Node platform retains the portable WebAssembly backend.
 */
function loadNativeBindings() {
    const developmentBinary = new URL("../native/squonk.node", import.meta.url);
    if (existsSync(developmentBinary)) {
        return requireBindings(developmentBinary.pathname, "local development addon");
    }
    const packageName = nativePackageName();
    if (packageName === null)
        return null;
    let resolved;
    try {
        resolved = require.resolve(packageName);
    }
    catch (error) {
        if (isMissingModule(error, packageName))
            return null;
        throw error;
    }
    return requireBindings(resolved, packageName);
}
function requireBindings(request, description) {
    let loaded;
    try {
        loaded = require(request);
    }
    catch (error) {
        throw new Error(`failed to load Squonk native backend from ${description}`, { cause: error });
    }
    if (!isBindings(loaded)) {
        throw new Error(`Squonk native backend ${description} has an incompatible export surface`);
    }
    return loaded;
}
function nativePackageName() {
    const { platform, arch } = process;
    if (platform === "darwin" && (arch === "arm64" || arch === "x64")) {
        return `@squonk-sql/native-darwin-${arch}`;
    }
    if (platform === "win32" && (arch === "arm64" || arch === "x64")) {
        return `@squonk-sql/native-win32-${arch}-msvc`;
    }
    if (platform === "linux" && (arch === "arm64" || arch === "x64")) {
        const libc = linuxLibc();
        return `@squonk-sql/native-linux-${arch}-${libc}`;
    }
    return null;
}
function linuxLibc() {
    const report = process.report?.getReport();
    if (report !== undefined && "header" in report &&
        typeof report.header === "object" && report.header !== null &&
        "glibcVersionRuntime" in report.header)
        return "gnu";
    if (report !== undefined && "sharedObjects" in report && Array.isArray(report.sharedObjects) &&
        report.sharedObjects.some((path) => typeof path === "string" && path.includes("libc.musl-"))) {
        return "musl";
    }
    try {
        if (readFileSync("/usr/bin/ldd", "utf8").toLowerCase().includes("musl"))
            return "musl";
    }
    catch {
        // Distroless images may not ship ldd. Supported Node Linux builds default to glibc.
    }
    return "gnu";
}
function isBindings(value) {
    if (typeof value !== "object" || value === null)
        return false;
    const candidate = value;
    return typeof candidate.parse_document_with === "function" &&
        typeof candidate.parse_with === "function" &&
        typeof candidate.render_sql === "function" &&
        typeof candidate.version === "function";
}
function isMissingModule(error, candidate) {
    return typeof error === "object" && error !== null && "code" in error &&
        error.code === "MODULE_NOT_FOUND" &&
        "message" in error && typeof error.message === "string" &&
        error.message.includes(candidate);
}
/** Initialize one colocated wasm artifact synchronously for a Node entrypoint. */
export function createNodeSquonk(initSync, wasm, wasmUrl, options) {
    const native = loadNativeBindings();
    if (native !== null) {
        return createSquonkApi(native, {
            ...options,
            runtime: { backend: "native", host: hostFamily() },
        });
    }
    try {
        initSync({ module: readFileSync(wasmUrl) });
    }
    catch (error) {
        throw new Error(`failed to initialize Squonk wasm from ${wasmUrl.href}`, { cause: error });
    }
    return createSquonkApi(wasm, {
        ...options,
        runtime: { backend: "wasm", host: hostFamily() },
    });
}
function hostFamily() {
    return "Bun" in globalThis ? "bun" : "node";
}
