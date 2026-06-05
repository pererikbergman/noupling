import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright config — drives the Explorer template in a real browser.
 *
 * Two modes:
 *  1. `pnpm test:e2e` — boots the Vite dev server, runs the smoke suite
 *     against `public/samples/*.json`. CI-friendly, no external state.
 *  2. `pnpm test:e2e:android` (manual only, gated on the env var) — drives
 *     a pre-emitted explorer.html against the real noupling-scanned
 *     repo at $NOUPLING_E2E_ANDROID_REPO. See tests/e2e/android.spec.ts.
 */
export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://localhost:5174",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: process.env.NOUPLING_E2E_ANDROID_REPO
    ? undefined // android.spec.ts loads file:// directly; no dev server needed.
    : {
        command: "pnpm dev",
        url: "http://localhost:5174",
        reuseExistingServer: !process.env.CI,
        stdout: "ignore",
        stderr: "pipe",
      },
});
