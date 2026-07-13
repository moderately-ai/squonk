#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { createRequire } from "node:module";
import { resolve } from "node:path";

const addonPath = process.argv[2];
if (!addonPath) throw new Error("usage: smoke-native-addon.mjs <path-to-addon>");
const require = createRequire(import.meta.url);
const addon = require(resolve(addonPath));
const document = addon.parse_document_with("select 1", "ansi", undefined, false, false);
if (document.render("ansi", "canonical") !== "SELECT 1") {
  throw new Error("native addon parse/render smoke failed");
}
if (typeof addon.version() !== "string" || addon.version().length === 0) {
  throw new Error("native addon version smoke failed");
}
