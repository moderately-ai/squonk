// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { fileURLToPath, URL } from "node:url";

import { defineConfig } from "vite";

const wasmCrateRoot = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  server: {
    fs: {
      allow: [wasmCrateRoot],
    },
  },
});
