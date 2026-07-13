#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { packages } from "./package-matrix.mjs";
import { currentNativePackage, nativePackages } from "./native-package-matrix.mjs";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const stageRoot = join(crateDir, "dist", "npm");
const version = JSON.parse(readFileSync(join(crateDir, "package.json"), "utf8")).version;
const optionalDependencies = Object.fromEntries(nativePackages.map(({ packageName }) => [packageName, version]));

rmSync(stageRoot, { force: true, recursive: true });
for (const variant of packages) {
  const out = join(stageRoot, variant.label);
  const jsOut = join(out, "js");
  mkdirSync(jsOut, { recursive: true });

  const browserEntry = variant.entry === "index" ? "browser" : `${variant.entry}-browser`;
  for (const stem of [variant.entry, browserEntry, "runtime", "node", "node-wasm", "module-wasm", "ast.generated", "ast-metadata.generated"]) {
    for (const extension of ["js", "d.ts"]) {
      const source = join(crateDir, "js", `${stem}.${extension}`);
      try {
        copyFileSync(source, join(jsOut, `${stem}.${extension}`));
      } catch (error) {
        if (stem !== "ast.generated" && stem !== "ast-metadata.generated") throw error;
      }
    }
  }

  const sourceEntry = readFileSync(join(jsOut, `${variant.entry}.js`), "utf8");
  writeFileSync(join(jsOut, `${variant.entry}-wasm.js`), sourceEntry
    .replace('from "./node.js"', 'from "./node-wasm.js"')
    .replaceAll("createNodeSquonk", "createNodeWasmSquonk"));
  copyFileSync(join(jsOut, `${variant.entry}.d.ts`), join(jsOut, `${variant.entry}-wasm.d.ts`));
  writeFileSync(join(jsOut, `${variant.entry}-workerd.js`), moduleEntry(variant, "workerd", ""));
  writeFileSync(join(jsOut, `${variant.entry}-edge-light.js`), moduleEntry(variant, "edge-light", "?module"));
  writeFileSync(join(jsOut, `${variant.entry}-deno.js`), denoEntry(variant));
  copyFileSync(join(jsOut, `${variant.entry}.d.ts`), join(jsOut, `${variant.entry}-workerd.d.ts`));
  copyFileSync(join(jsOut, `${variant.entry}.d.ts`), join(jsOut, `${variant.entry}-edge-light.d.ts`));
  copyFileSync(join(jsOut, `${variant.entry}.d.ts`), join(jsOut, `${variant.entry}-deno.d.ts`));

  cpSync(join(crateDir, `pkg-${variant.label}`), join(out, `pkg-${variant.label}`), { recursive: true });
  copyFileSync(join(crateDir, "LICENSE"), join(out, "LICENSE"));
  copyFileSync(join(crateDir, "README.md"), join(out, "README.md"));

  const manifest = {
    name: variant.packageName,
    version,
    private: true,
    description: `Typed WebAssembly SQL parser for ${variant.label === "all" ? "all built-in dialects" : variant.defaultDialect}`,
    type: "module",
    main: `./js/${variant.entry}.js`,
    types: `./js/${variant.entry}.d.ts`,
    exports: {
      ".": {
        bun: { types: `./js/${variant.entry}.d.ts`, default: `./js/${variant.entry}.js` },
        workerd: { types: `./js/${variant.entry}-workerd.d.ts`, default: `./js/${variant.entry}-workerd.js` },
        "edge-light": { types: `./js/${variant.entry}-edge-light.d.ts`, default: `./js/${variant.entry}-edge-light.js` },
        deno: { types: `./js/${variant.entry}-deno.d.ts`, default: `./js/${variant.entry}-deno.js` },
        "node-addons": { types: `./js/${variant.entry}.d.ts`, default: `./js/${variant.entry}.js` },
        node: { types: `./js/${variant.entry}-wasm.d.ts`, default: `./js/${variant.entry}-wasm.js` },
        browser: { types: `./js/${browserEntry}.d.ts`, default: `./js/${browserEntry}.js` },
        default: { types: `./js/${browserEntry}.d.ts`, default: `./js/${browserEntry}.js` },
      },
      "./node": { types: `./js/${variant.entry}.d.ts`, default: `./js/${variant.entry}.js` },
      "./wasm": {
        node: { types: `./js/${variant.entry}-wasm.d.ts`, default: `./js/${variant.entry}-wasm.js` },
        types: `./js/${browserEntry}.d.ts`,
        default: `./js/${browserEntry}.js`,
      },
      "./browser": { types: `./js/${browserEntry}.d.ts`, default: `./js/${browserEntry}.js` },
      "./workerd": { types: `./js/${variant.entry}-workerd.d.ts`, default: `./js/${variant.entry}-workerd.js` },
      "./edge-light": { types: `./js/${variant.entry}-edge-light.d.ts`, default: `./js/${variant.entry}-edge-light.js` },
      "./deno": { types: `./js/${variant.entry}-deno.d.ts`, default: `./js/${variant.entry}-deno.js` },
    },
    engines: { node: ">=22" },
    author: "Moderately AI",
    homepage: "https://github.com/moderately-ai/squonk#readme",
    repository: { type: "git", url: "git+https://github.com/moderately-ai/squonk.git", directory: "crates/squonk-wasm" },
    bugs: { url: "https://github.com/moderately-ai/squonk/issues" },
    publishConfig: { access: "public" },
    keywords: ["sql", "parser", "wasm", "webassembly", "ast", variant.defaultDialect],
    license: "MIT",
    optionalDependencies,
  };
  writeFileSync(join(out, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
}

stageNativePackages();

function moduleEntry(variant, host, query) {
  const options = JSON.stringify({
    defaultDialect: variant.defaultDialect,
    supportedDialects: variant.supportedDialects,
  });
  return `// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.
import { initSync } from "../pkg-${variant.label}/squonk_wasm.js";
import * as wasm from "../pkg-${variant.label}/squonk_wasm.js";
import wasmModule from "../pkg-${variant.label}/squonk_wasm_bg.wasm${query}";
import { createModuleWasmSquonk } from "./module-wasm.js";
export { Diagnostic, Document, Ident, Node, ObjectName, RecoveredDocument, SqlParseError } from "./runtime.js";
const api = createModuleWasmSquonk(initSync, wasm, wasmModule, ${options}, ${JSON.stringify(host)});
export const { isDialectName, assertDialectName, canonicalDialectName, parse, parseJson, parseWithLimit,
  parseRecovering, parseRecoveringJson, supportedDialects, tokenize, render, redact, format,
  transpile, version, schemaVersion, runtimeInfo } = api;
`;
}

function denoEntry(variant) {
  const options = JSON.stringify({
    defaultDialect: variant.defaultDialect,
    supportedDialects: variant.supportedDialects,
  });
  return `// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.
import * as wasm from "../pkg-${variant.label}/deno/squonk_wasm.js";
import { createSquonkApi } from "./runtime.js";
export { Diagnostic, Document, Ident, Node, ObjectName, RecoveredDocument, SqlParseError } from "./runtime.js";
const api = createSquonkApi(wasm, { ...${options}, runtime: { backend: "wasm", host: "deno" } });
export const { isDialectName, assertDialectName, canonicalDialectName, parse, parseJson, parseWithLimit,
  parseRecovering, parseRecoveringJson, supportedDialects, tokenize, render, redact, format,
  transpile, version, schemaVersion, runtimeInfo } = api;
`;
}

function stageNativePackages() {
  const current = currentNativePackage();
  for (const platformPackage of nativePackages) {
    const label = platformPackage.packageName.slice("@squonk-sql/".length);
    const artifact = join(crateDir, "native-artifacts", label, "squonk.node");
    const local = join(crateDir, "native", "squonk.node");
    const addon = existsSync(artifact)
      ? artifact
      : current?.packageName === platformPackage.packageName && existsSync(local)
        ? local
        : null;
    if (addon !== null) stageNativePackage(platformPackage, addon);
  }
}

function stageNativePackage(platformPackage, addon) {
  const label = platformPackage.packageName.slice("@squonk-sql/".length);
  const out = join(stageRoot, label);
  mkdirSync(out, { recursive: true });
  copyFileSync(addon, join(out, "squonk.node"));
  copyFileSync(join(crateDir, "LICENSE"), join(out, "LICENSE"));
  const manifest = {
    name: platformPackage.packageName,
    version,
    private: true,
    description: "Native Node-API backend for Squonk SQL packages",
    main: "./squonk.node",
    files: ["squonk.node", "LICENSE"],
    os: [platformPackage.os],
    cpu: [platformPackage.cpu],
    ...(platformPackage.libc ? { libc: [platformPackage.libc] } : {}),
    engines: { node: ">=22" },
    repository: { type: "git", url: "git+https://github.com/moderately-ai/squonk.git", directory: "crates/squonk-node" },
    publishConfig: { access: "public" },
    license: "MIT",
  };
  writeFileSync(join(out, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
}
