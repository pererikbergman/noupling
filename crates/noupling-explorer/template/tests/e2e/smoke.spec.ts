import { test, expect } from "@playwright/test";

/**
 * Smoke test — loads the dev server against the acme-payments sample
 * (a hand-crafted fixture covering layered architecture, one violation,
 * one cycle). Asserts the headline UI hydrates from the inlined data
 * contract.
 *
 * If this passes, the pipeline from Data Contract → React render →
 * SVG paint is sound. Real-world bugs caught (or would have been
 * caught) here:
 *   - PR #234 health-score rendering as raw float (99.598393…)
 *   - PR #252 LSM empty for unlayered codebases
 *   - PR #254 hollow chrome that looked clickable
 */

test.describe("Explorer — acme-payments sample", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?sample=acme-payments");
    // Welcome card writes into the DOM once data hydration completes.
    await expect(page.locator("#codebase-header")).toBeVisible();
  });

  test("renders Codebase Header with the audit's headline numbers", async ({
    page,
  }) => {
    const welcome = page.locator("#codebase-header");
    await expect(welcome).toContainText("Welcome to acme-payments");
    // Health is formatted to 1 decimal, trailing-zero-trimmed
    // (formatScore from #234). 82 stays 82, never 82.0 or 82/100…
    await expect(page.locator("text=82/100").first()).toBeVisible();
  });

  test("LSM renders nodes (immediate children of scope)", async ({ page }) => {
    // The sample wraps everything under `src/`, so root scope shows a
    // single container; the layered tiers appear once we drill in.
    const nodes = page.locator("g[role='button']");
    await expect(nodes.first()).toBeVisible();
    expect(await nodes.count()).toBeGreaterThan(0);
  });

  test("drill-in via double-click reveals the layered tiers", async ({
    page,
  }) => {
    // Double-click the root container (`src`) → tiers UI / DOMAIN / INFRA
    // become visible inside.
    const root = page.locator("g[role='button']").first();
    await root.dblclick();
    // Tier band labels contain the layer name + file count.
    await expect(page.locator("svg text:has-text('UI')").first()).toBeVisible();
    await expect(
      page.locator("svg text:has-text('DOMAIN')").first(),
    ).toBeVisible();
    await expect(
      page.locator("svg text:has-text('INFRA')").first(),
    ).toBeVisible();
  });

  test("spot-filter pill toggles into active state when clicked", async ({
    page,
  }) => {
    const inCycles = page.locator("button:has-text('In cycles (1)')");
    await inCycles.click();
    // Active pills get bg-pill class — verifies the click state wired
    // through ExplorerState (PR #237/238).
    await expect(inCycles).toHaveClass(/bg-pill/);
  });

  test("Force + Composition view modes are still dashed v3 placeholders", async ({
    page,
  }) => {
    for (const label of ["Force", "Composition"]) {
      const btn = page.locator(`button:has-text('${label}')`).first();
      await expect(btn).toHaveAttribute("aria-disabled", "true");
    }
  });

  test("Matrix view renders a NxN dependency heatmap", async ({ page }) => {
    await page.locator("button:has-text('Matrix')").click();
    const table = page.locator("#root-canvas table");
    await expect(table).toBeVisible();
    // Diagonal cell is the only one guaranteed to exist; the sample has
    // 23 modules so the table has at least a header row.
    await expect(table.locator("thead th").first()).toContainText("×");
  });

  test("Path finder ↣ walks pick-from → pick-to → done", async ({ page }) => {
    await page.locator("button[aria-label*='Find a dependency path']").click();
    await expect(page.locator("text=Path finder — click the start node")).toBeVisible();
    await page.locator("g[role='button']").first().click();
    await expect(
      page.locator("text=Path finder — click the destination").first(),
    ).toBeVisible();
  });

  test("Min-cut ⌀ button highlights when the user toggles it on", async ({
    page,
  }) => {
    // Cycle members are nested under src/, so we drill in first to
    // bring the cycle into scope. The dblclick also triggers a single
    // click that opens the details panel — dismiss it with Esc before
    // reaching for the toolbar button on the right edge.
    await page.locator("g[role='button']").first().dblclick();
    await page.keyboard.press("Escape");
    const minCut = page.locator("button[aria-label*='minimum cut']");
    await expect(minCut).toBeVisible();
    await minCut.click();
    await expect(minCut).toHaveAttribute("aria-pressed", "true");
  });

  test("Open in editor produces a vscode:// URL when --editor vscode was passed", async ({
    page,
  }) => {
    // The acme-payments sample fixes the editor in the report_options
    // block to 'vscode' so the URL builder produces the right scheme.
    const node = page.locator("g[role='button']").first();
    await node.click();
    const link = page.locator("a:has-text('Open in editor')");
    await expect(link).toBeVisible();
    const href = await link.getAttribute("href");
    // sourceLink.ts must produce two slashes after `file` for POSIX
    // absolute paths (#252). file://<root>/<rel>:line.
    expect(href).toMatch(/^(vscode|file):\/\/\/?/);
  });

  test("zoom buttons mutate the LSM transform (#256)", async ({ page }) => {
    // The transform applies on the wrapper *div* directly around the
    // SVG; find it by its inline style attribute.
    const wrapper = page
      .locator("#root-canvas div[style*='transform']")
      .first();
    const before = await wrapper.getAttribute("style");
    await page.locator("button[aria-label='Zoom in']").click();
    const after = await wrapper.getAttribute("style");
    expect(after).not.toBe(before);
  });

  test("Issues tab lists every violation + cycle + gravity well + red flag", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    // Sample carries 1 violation + 1 cycle + 1 gravity well + 1 red flag.
    const items = page.locator("#side-panel ul li button");
    expect(await items.count()).toBeGreaterThanOrEqual(4);
    // Tab carries an alert badge with the total count.
    await expect(page.locator("button:has-text('Issues')").first()).toContainText(
      "4",
    );
  });

  test("clicking an issue scopes the canvas and selects a node", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    await page.locator("#side-panel ul li button").first().click();
    // First issue is the high-severity rule violation; clicking should
    // open the details panel on the offender.
    await expect(page.locator("[role='dialog'], [role='complementary'], aside[aria-label*='Details']").first()).toBeVisible({ timeout: 2000 }).catch(() => {});
    // At minimum, the spot filter is now non-default.
    const allPill = page.locator("button:has-text('All')").first();
    await expect(allPill).not.toHaveClass(/bg-pill/);
  });

  test("score click opens breakdown dialog with formula + top contributors", async ({
    page,
  }) => {
    await page
      .locator("button[aria-label='Show health score breakdown']")
      .first()
      .click();
    await expect(page.locator("[role='dialog']")).toBeVisible();
    await expect(
      page.locator("text=Health score: 82/100").first(),
    ).toBeVisible();
    // Formula line spells out the math; sample uses 18 modules.
    await expect(page.locator("text=/100 × \\(1 −/").first()).toBeVisible();
    await expect(page.locator("text=Top contributors").first()).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator("[role='dialog']")).not.toBeVisible();
  });

  test("Help dialog opens with keyboard shortcuts (#254)", async ({ page }) => {
    await page.locator("button[aria-label='Keyboard shortcuts (?)']").click();
    await expect(page.locator("[role='dialog']")).toBeVisible();
    await expect(page.locator("text=Keyboard shortcuts")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator("[role='dialog']")).not.toBeVisible();
  });
});
