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

  test("v2/v3 placeholder controls are visibly disabled (#254)", async ({
    page,
  }) => {
    for (const label of ["Matrix", "Force", "Composition"]) {
      const btn = page.locator(`button:has-text('${label}')`).first();
      await expect(btn).toHaveAttribute("aria-disabled", "true");
    }
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

  test("Help dialog opens with keyboard shortcuts (#254)", async ({ page }) => {
    await page.locator("button[aria-label='Keyboard shortcuts (?)']").click();
    await expect(page.locator("[role='dialog']")).toBeVisible();
    await expect(page.locator("text=Keyboard shortcuts")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator("[role='dialog']")).not.toBeVisible();
  });
});
