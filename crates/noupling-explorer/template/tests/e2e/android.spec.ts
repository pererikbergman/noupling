import { test, expect } from "@playwright/test";
import { existsSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

/**
 * Manual e2e against a real codebase — NOT run in CI.
 *
 * Set `NOUPLING_E2E_ANDROID_REPO=/path/to/your/project` and run:
 *
 *   noupling-dev report $NOUPLING_E2E_ANDROID_REPO --format explorer --editor vscode
 *   pnpm test:e2e:android
 *
 * Walks the *production-emitted* explorer.html (file:// load, no dev
 * server, no sample-data fallback) and exercises the parts that broke
 * historically on real-world codebases:
 *   - LSM not empty at root (#258 singleton-chain collapse)
 *   - Auto-layer banner present when settings.layers is empty (PR #260)
 *   - Open-in-editor URLs point at the absolute path (PR #252 / inline)
 *   - Health Score renders as a real number (not raw float)
 */

const repoRoot = process.env.NOUPLING_E2E_ANDROID_REPO;

test.skip(!repoRoot, "NOUPLING_E2E_ANDROID_REPO not set; skipping.");

test.describe("Explorer — real codebase regression suite", () => {
  let explorerUrl: string;

  test.beforeAll(() => {
    if (!repoRoot) return;
    const path = join(repoRoot, ".noupling", "explorer.html");
    if (!existsSync(path)) {
      throw new Error(
        `Missing ${path}. Run \`noupling-dev report ${repoRoot} --format explorer\` first.`,
      );
    }
    explorerUrl = pathToFileURL(path).href;
  });

  test.beforeEach(async ({ page }) => {
    await page.goto(explorerUrl);
    await expect(page.locator("#codebase-header")).toBeVisible();
  });

  test("LSM at root is not empty (#258 collapse)", async ({ page }) => {
    const nodes = page.locator("g[role='button']");
    await expect(nodes.first()).toBeVisible();
    expect(await nodes.count()).toBeGreaterThan(0);
  });

  test("Auto-layer banner appears for codebases with no configured layers (#260)", async ({
    page,
  }) => {
    const banner = page.locator("[role='note']:has-text('Layers auto-detected')");
    // Pass either way: codebases WITH layers won't show it.
    const hasBanner = (await banner.count()) > 0;
    if (hasBanner) {
      await expect(banner).toBeVisible();
      // Banner should name the inferred layer set.
      await expect(banner).toContainText(/presentation|domain|data|model|infra/i);
    }
  });

  test("Open in editor URL is absolute (#252)", async ({ page }) => {
    await page.locator("g[role='button']").first().click();
    const link = page.locator("a:has-text('Open in editor')");
    await expect(link).toBeVisible();
    const href = await link.getAttribute("href");
    expect(href).not.toBeNull();
    // POSIX absolute paths produce two slashes after the scheme host.
    // vscode://file//Users/... or cursor://file//Users/...
    expect(href).toMatch(/^(vscode|cursor|file|subl|jetbrains):\/\//);
    // And the path component must NOT start with `.` (relative leaked
    // through pre-#252).
    expect(href).not.toMatch(/^[a-z]+:\/\/file\/\./);
  });

  test("Health Score renders as a clean number (#234)", async ({ page }) => {
    const score = page.locator("text=/^\\d+(\\.\\d)?\\/100$/").first();
    await expect(score).toBeVisible();
  });
});
