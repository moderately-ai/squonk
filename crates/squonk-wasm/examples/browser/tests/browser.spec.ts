// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { expect, test } from "@playwright/test";

test("loads the interactive wasm AST explorer", async ({ page }) => {
  const consoleErrors: string[] = [];
  const failedRequests: string[] = [];

  page.on("console", (message) => {
    if (message.type() === "error") {
      consoleErrors.push(message.text());
    }
  });
  page.on("requestfailed", (request) => {
    failedRequests.push(`${request.method()} ${request.url()}`);
  });

  await page.goto("/");

  await expect(page.locator("#dialect option")).toHaveCount(13);
  await expect(page.locator("#target-dialect option")).toHaveCount(13);
  await expect(page.locator("#dialect")).toHaveValue("postgres");
  await expect(page.locator("#target-dialect")).toHaveValue("postgres");
  await expect(page.locator(".cm-editor")).toBeVisible();
  await expect(page.locator(".cm-content")).toContainText("COUNT");
  await expect(page.locator(".cm-ast-span").first()).toBeVisible();
  await expect(page.locator("#status")).toContainText("nodes");
  await expect(page.locator("#metrics")).toContainText("Parse");
  await expect(page.locator("#metrics")).toContainText("Tokenize");
  await expect(page.locator("#resources")).toContainText("AST JSON");
  await expect(page.locator("#ast-tree svg")).toBeVisible();
  await expect(page.locator("#node-details")).toContainText("Table");

  await page.locator('#ast-tree [data-node-kind="Table"]').first().click();
  await expect(page.locator("#selected-node-kind")).toHaveText("Table");
  await expect(page.locator("#selection-summary")).toContainText("Table");

  await page.getByRole("button", { name: "CTE rollup" }).click();
  await expect(page.locator(".cm-content")).toContainText("regional_sales");
  await expect(page.locator("#status")).toContainText("nodes");
  await expect(page.locator("#diagnostics")).toContainText("No diagnostics");

  await page.getByRole("button", { name: "Recovery" }).click();
  await expect(page.locator("#recovering")).toBeChecked();
  await expect(page.locator("#dialect")).toHaveValue("ansi");
  await expect(page.locator("#status")).toContainText("2 stmts");
  await expect(page.locator("#diagnostics")).toContainText("syntax");
  await expect(page.locator("#rendered")).toContainText("SELECT 1 AS ok");

  expect(consoleErrors).toEqual([]);
  expect(failedRequests).toEqual([]);
});
