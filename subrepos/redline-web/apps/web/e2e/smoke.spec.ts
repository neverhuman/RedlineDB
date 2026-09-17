import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

// The smoke proves three things against the real embedded binary:
//   1. the built SPA renders (index.html + /assets bundle hydrate),
//   2. a live POST /api/query round-trips through the SQLite connector,
//   3. the rendered page has no critical/serious accessibility violations.

test("SPA renders the workbench shell", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/redline-web/i);
  // The SQL editor is the primary control of the Query tab.
  await expect(page.getByLabel("SQL editor")).toBeVisible();
});

test("POST /api/query round-trips through the connector", async ({ request }) => {
  const res = await request.post("/api/query", {
    data: { sql: "select 1 as n, 'hello' as greeting", maxRows: 10 },
  });
  expect(res.ok()).toBeTruthy();
  const body: { columns: string[]; rows: unknown[][]; rowCount: number } =
    await res.json();
  expect(body.columns).toEqual(["n", "greeting"]);
  expect(body.rowCount).toBe(1);
  expect(body.rows[0]).toEqual([1, "hello"]);
});

test("running a query from the UI shows results", async ({ page }) => {
  await page.goto("/");
  const editor = page.getByLabel("SQL editor");
  await editor.fill("select 42 as answer");
  await page.getByRole("button", { name: /run/i }).click();
  await expect(page.getByRole("columnheader", { name: "answer" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "42" })).toBeVisible();
});

test("the workbench has no critical accessibility violations", async ({ page }) => {
  await page.goto("/");
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa"])
    .analyze();
  const serious = results.violations.filter(
    (v) => v.impact === "critical" || v.impact === "serious",
  );
  expect(serious).toEqual([]);
});
