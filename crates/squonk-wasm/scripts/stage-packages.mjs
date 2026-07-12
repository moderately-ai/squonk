#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { copyFileSync, cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { packages } from "./package-matrix.mjs";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const stageRoot = join(crateDir, "dist", "npm");
const version = JSON.parse(readFileSync(join(crateDir, "package.json"), "utf8")).version;

rmSync(stageRoot, { force: true, recursive: true });
for (const variant of packages) {
  const out = join(stageRoot, variant.label);
  const jsOut = join(out, "js");
  mkdirSync(jsOut, { recursive: true });

  const browserEntry = variant.entry === "index" ? "browser" : `${variant.entry}-browser`;
  for (const stem of [variant.entry, browserEntry, "runtime", "node", "ast.generated", "ast-metadata.generated"]) {
    for (const extension of ["js", "d.ts"]) {
      const source = join(crateDir, "js", `${stem}.${extension}`);
      try {
        copyFileSync(source, join(jsOut, `${stem}.${extension}`));
      } catch (error) {
        if (stem !== "ast.generated" && stem !== "ast-metadata.generated") throw error;
      }
    }
  }

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
      ".": { types: `./js/${variant.entry}.d.ts`, default: `./js/${variant.entry}.js` },
      "./browser": { types: `./js/${browserEntry}.d.ts`, default: `./js/${browserEntry}.js` },
    },
    engines: { node: ">=22" },
    author: "Moderately AI",
    homepage: "https://github.com/moderately-ai/squonk#readme",
    repository: { type: "git", url: "git+https://github.com/moderately-ai/squonk.git", directory: "crates/squonk-wasm" },
    bugs: { url: "https://github.com/moderately-ai/squonk/issues" },
    publishConfig: { access: "public" },
    keywords: ["sql", "parser", "wasm", "webassembly", "ast", variant.defaultDialect],
    license: "MIT",
  };
  writeFileSync(join(out, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
}
