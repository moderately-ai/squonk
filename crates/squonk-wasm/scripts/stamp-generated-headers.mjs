#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const crateDir = dirname(dirname(fileURLToPath(import.meta.url)));
const jsDir = join(crateDir, "js");
const header = "// SPDX-License-Identifier: MIT\n// Copyright (c) 2026 Moderately AI Inc.\n\n";
const existingHeader = /^\/\/ SPDX-License-Identifier: MIT\r?\n\/\/ Copyright \(c\) 2026 Moderately AI Inc\.\r?\n(?:\r?\n)?/;

for (const name of readdirSync(jsDir)) {
  if (!name.endsWith(".js") && !name.endsWith(".d.ts")) continue;
  const path = join(jsDir, name);
  const source = readFileSync(path, "utf8");
  const stamped = `${header}${source.replace(existingHeader, "")}`;
  if (source !== stamped) {
    writeFileSync(path, stamped);
  }
}
