// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:5173",
    browserName: "chromium",
    trace: "retain-on-failure",
    ...devices["Desktop Chrome"],
  },
  webServer: {
    command: "npm run dev -- --port 5173",
    reuseExistingServer: !process.env.CI,
    url: "http://127.0.0.1:5173",
    timeout: 30_000,
  },
});
