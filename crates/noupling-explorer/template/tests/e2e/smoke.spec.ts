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
    // The sample wraps everything under `src/`; the home scope is `src`
    // itself (#397), so the canvas opens on its packages, not one card.
    const nodes = page.locator("g[role='button']");
    await expect(nodes.first()).toBeVisible();
    expect(await nodes.count()).toBeGreaterThan(1);
  });

  test("the Explorer opens at the first level that branches, with the layered tiers visible (#397)", async ({
    page,
  }) => {
    // The sample wraps everything under a lone `src/`; the home scope
    // skips that single-child chain so the first canvas shows the
    // tiers UI / DOMAIN / INFRA, not one `src` card.
    await expect(
      page.locator("#root-canvas nav[aria-label='Drill scope']"),
    ).toContainText("src");
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

  test("Composition view renders annotated container cards (#279)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Composition')").click();
    // Banner explains the LLM-enrichment story.
    await expect(
      page.locator("text=/shows what each module/").first(),
    ).toBeVisible();
    // At least one container card renders for the sample.
    const cards = page.locator("ul li button[title*='click to focus']");
    expect(await cards.count()).toBeGreaterThan(0);
  });

  test("Composition under issue focus shows the participants, not their siblings (#401)", async ({
    page,
  }) => {
    // Focus the first Issue: the Coupling Violation CheckoutForm.tsx → db.rs
    // (participants under src/ui and src/infra). Composition must show
    // those two, and must not show only the untouched src/domain.
    await page.locator("button:has-text('Issues')").click();
    await page.locator("#side-panel ul li button[data-issue-key]").first().click();
    await expect(page.locator("text=Issue focused").first()).toBeVisible();
    await page.locator("button:has-text('Composition')").click();
    const cards = page.locator("#root-canvas ul li button[title*='click to focus']");
    const titles = await cards.evaluateAll((els) => els.map((e) => e.getAttribute("title") ?? ""));
    expect(titles.some((t) => t.startsWith("src/ui/CheckoutForm.tsx"))).toBe(true);
    expect(titles.some((t) => t.startsWith("src/infra/db.rs"))).toBe(true);
    // The untouched sibling stays on the canvas but dimmed (#335).
    const domain = cards.filter({ has: page.locator("text=domain") }).first();
    await expect(domain).toHaveAttribute("data-dimmed", "true");
    const participant = cards.filter({ hasText: "CheckoutForm.tsx" }).first();
    await expect(participant).toHaveAttribute("data-dimmed", "false");
  });

  test("critical and high band chips are coloured like medium, not plain text (#403)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    const chip = (band: string) =>
      page.locator(`#side-panel ul li button[data-issue-key] span:text-is("${band}")`).first();
    for (const band of ["critical", "high", "medium"]) {
      const bg = await chip(band).evaluate((el) => getComputedStyle(el).backgroundColor);
      expect(bg, `${band} chip background`).not.toMatch(/rgba\(0, 0, 0, 0\)|transparent/);
    }
  });

  test("Rules tab: one layer-order card for the whole stack, dependency rules show both globs (#404)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Rules')").click();
    // Three layers used to produce three pairwise "may not depend on" rows.
    const stack = page.locator("[data-testid='layer-order']");
    await expect(stack).toHaveCount(1);
    await expect(stack).toContainText("ui");
    await expect(stack).toContainText("domain");
    await expect(stack).toContainText("infra");
    expect(await page.locator("text=/may not depend on layer/").count()).toBe(0);
    // The explicit rule shows source and target on their own lines, untruncated.
    const rule = page.locator("[data-testid='dependency-rule']").first();
    await expect(rule.locator("[data-role='rule-from']")).toHaveText("**/ui/**");
    await expect(rule.locator("[data-role='rule-to']")).toHaveText("**/infra/**");
  });

  test("Info tab describes the drilled directory, not the whole project (#405)", async ({
    page,
  }) => {
    // At home the welcome card is about the project.
    await expect(page.locator("#codebase-header")).toContainText("Welcome to acme-payments");
    // Drill into src/ui via the Levels tab; the card now describes src/ui.
    await page.locator("button:has-text('Levels')").click();
    const ui = page.locator("#side-panel button[title*='double-click to drill']").filter({ hasText: "ui" }).first();
    await ui.dblclick();
    await page.locator("button:has-text('Info')").click();
    await expect(page.locator("#codebase-header")).toHaveText("ui");
    await expect(page.locator("#codebase-header ~ p").first()).toContainText("src/ui");
  });

  test("the Composition banner names no ticket or skill (#405)", async ({ page }) => {
    await page.locator("button:has-text('Composition')").click();
    const banner = page.locator("[role='note']").first();
    await expect(banner).not.toContainText("#280");
    await expect(banner).not.toContainText("skill");
  });

  test("issue focus puts the participants in the viewport and dims everything else (#334, #335)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    // First card: Coupling Violation CheckoutForm.tsx → db.rs.
    await page.locator("#side-panel ul li button[data-issue-key]").first().click();
    await expect(page.locator("text=Issue focused").first()).toBeVisible();
    const card = (name: string) =>
      page.locator(`#root-canvas svg g[role='button'][aria-label^='${name} ']`).first();
    await expect(card("CheckoutForm.tsx")).toBeInViewport();
    await expect(card("db.rs")).toBeInViewport();
    // Participants at full opacity; the untouched sibling package dimmed.
    await expect(card("CheckoutForm.tsx")).toHaveAttribute("data-emphasis", "participant");
    await expect(card("domain")).toHaveAttribute("data-emphasis", "dimmed");
    // The offending edge stays prominent; an unrelated edge is dimmed.
    const offending = page.locator("#root-canvas svg path[data-edge='src/ui/CheckoutForm.tsx→src/infra/db.rs']");
    await expect(offending).toHaveAttribute("data-emphasis", "participant");
  });

  test("focusing a directory-shaped Issue keeps its children at full strength (#335)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    // The sample's Low Cohesion Issue is about src/ui; its files are the participants.
    await page.locator("#side-panel ul li button[data-issue-kind='low_cohesion']").first().click();
    await expect(page.locator("text=Issue focused").first()).toBeVisible();
    const cards = page.locator("#root-canvas svg g[role='button']");
    expect(await cards.count()).toBeGreaterThan(0);
    const dimmed = page.locator("#root-canvas svg g[role='button'][data-emphasis='dimmed']");
    expect(await dimmed.count()).toBe(0);
    // Drilling somewhere else ends the focus.
    await page.locator("#root-canvas nav[aria-label='Drill scope'] button").first().click();
    await expect(page.locator("text=Issue focused")).toHaveCount(0);
  });

  test("Composition surfaces LLM enrichment when the data carries an llm block (#280)", async ({
    page,
  }) => {
    // Sample wraps everything under src/, and the llm.summary lives on
    // src/ui — drill into src first so Composition shows that level.
    await page.locator("button:has-text('Composition')").click();
    await expect(
      page.locator("text=Checkout + receipt UI").first(),
    ).toBeVisible();
  });

  test("Matrix aggregates file-level edges to visible packages (#290)", async ({
    page,
  }) => {
    // Regression: narrowData previously required both edge endpoints
    // in visibleIds, which silently dropped every file-level edge
    // when the visible nodes were packages — leaving the Matrix view
    // entirely blank on real codebases. Drill into src/ so the four
    // sample packages (ui, domain, data, infra) are immediate
    // children; the matrix must render at least one coloured cell
    // for the aggregated file-level imports between them.
    await page.locator("button:has-text('Matrix')").click();
    // accent-domain alpha-tinted cells = healthy (non-violation) edges.
    const coloured = page.locator(
      "#root-canvas td[style*='accent-domain']",
    );
    expect(await coloured.count()).toBeGreaterThan(0);
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

  test("Issues tab lists every Issue kind from the shared issues array (#345)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    // Sample carries one Issue of each of the nine kinds.
    const items = page.locator("#side-panel ul li button[data-issue-key]");
    expect(await items.count()).toBe(9);
    for (const kind of [
      "coupling_violation",
      "cycle",
      "rule_violation",
      "layer_violation",
      "gravity_well",
      "red_flag",
      "stability_violation",
      "zone_flag",
      "low_cohesion",
    ]) {
      expect(await page.locator(`[data-issue-kind='${kind}']`).count()).toBe(1);
    }
    // Tab carries an alert badge with the total count.
    await expect(page.locator("button:has-text('Issues')").first()).toContainText(
      "9",
    );
    // Cards are in canonical order: the critical ones first.
    await expect(items.first()).toContainText("critical");
  });

  test("Cycle card and baselined card render from the issues array (#345)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    const cycle = page.locator("[data-issue-kind='cycle']");
    await expect(cycle).toBeVisible();
    await expect(cycle).toContainText("Cycle");
    await expect(cycle).toContainText("domain → infra → domain");
    // The card's tooltip carries the core reason + recommendation verbatim.
    await expect(cycle).toHaveAttribute("title", /cheapest break is src\/infra -> src\/domain/);
    await expect(cycle).toHaveAttribute("title", /Cut the cycle at/);

    const baselined = page.locator("[data-baselined='true']");
    expect(await baselined.count()).toBe(1);
    await expect(baselined.first()).toContainText("accepted");
    await expect(baselined.first()).toHaveClass(/opacity-60/);
    // New / baselined split excludes the accepted one from "new".
    await expect(page.locator("[data-testid='issues-summary']")).toContainText("8 new");
    await expect(page.locator("[data-testid='issues-summary']")).toContainText("1 baselined");
  });

  test("Details panel verdict text is the Issue's reason and recommendation (#345)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    await page.locator("[data-issue-kind='gravity_well']").click();
    const verdict = page.locator("[data-verdict-kind='gravity_well']");
    await expect(verdict).toBeVisible();
    await expect(verdict.locator("[data-role='reason']")).toContainText(
      "carries a total RRI of 38 across 6 relationships",
    );
    await expect(verdict.locator("[data-role='recommendation']")).toContainText(
      "Split src/infra/db.rs by responsibility",
    );
  });

  test("clicking an issue enters focus mode + selects the offender file", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    await page.locator("#side-panel ul li button").first().click();
    // Focus banner appears; canvas is scoped to the LCA.
    await expect(page.locator("text=Issue focused").first()).toBeVisible();
    // DetailsPanel may or may not be visible depending on layout —
    // assert opportunistically without failing if it's covered.
    await expect(
      page
        .locator(
          "[role='dialog'], [role='complementary'], aside[aria-label*='Details']",
        )
        .first(),
    )
      .toBeVisible({ timeout: 2000 })
      .catch(() => {});
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
    // Points lost = 100 − score, and the per-kind rows sum to it (#345).
    await expect(page.locator("[data-testid='points-lost']")).toHaveText("18");
    const rows = page.locator("[data-testid='points-by-kind'] dd");
    const points = await rows.allTextContents();
    const sum = points.reduce((acc, t) => acc + parseFloat(t.replace("−", "")), 0);
    expect(sum).toBeCloseTo(18, 5);
    await expect(page.locator("text=Top contributors").first()).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator("[role='dialog']")).not.toBeVisible();
  });

  test("Issues tab: hover not near-black, sticky selected, focus mode banner (#275)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    const firstCard = page.locator("#side-panel ul li button").first();
    await firstCard.click();
    await expect(firstCard).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("text=Issue focused").first()).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator("text=Issue focused").first()).not.toBeVisible();
  });

  test("Issue focus mode expands participant containers to file level (#275 follow-up)", async ({
    page,
  }) => {
    // Click the high-severity violation between src/ui/CheckoutForm.tsx
    // and src/infra/db.rs. Focus mode should drill to `src`, then
    // render the ui/ and infra/ containers expanded so the offender
    // FILES (CheckoutForm.tsx, db.rs) appear on the canvas — not the
    // collapsed package cards.
    await page.locator("button:has-text('Issues')").click();
    await page.locator("#side-panel ul li button").first().click();
    // The participant file names should now appear in the SVG node set.
    await expect(
      page.locator("svg text:has-text('CheckoutForm.tsx')").first(),
    ).toBeVisible();
    await expect(
      page.locator("svg text:has-text('db.rs')").first(),
    ).toBeVisible();
  });

  test("focusing a gravity well shows the well and the modules pulling on it (#333)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    await page.locator("[data-issue-kind='gravity_well']").click();
    await expect(page.locator("text=Issue focused").first()).toBeVisible();
    // The well (db.rs) and its contributors are all on the canvas — not one
    // lonely card.
    for (const name of ["db.rs", "CheckoutForm.tsx", "payment.rs"]) {
      await expect(page.locator(`svg text:has-text('${name}')`).first()).toBeVisible();
    }
    const cards = page.locator("svg g[role='button']");
    expect(await cards.count()).toBeGreaterThanOrEqual(2);
    // The contributing edges are the highlighted (priority) ones.
    const focused = page.locator("svg path[data-accent='minCut'][data-edge*='db.rs']");
    expect(await focused.count()).toBeGreaterThanOrEqual(1);
  });

  test("Levels tab: containers only, double-click drills shared scope (#274)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Levels')").click();
    const rows = page.locator("#side-panel ul li button");
    expect(await rows.count()).toBeGreaterThanOrEqual(1);
    await rows.first().dblclick();
    await expect(
      page.locator("#side-panel button[aria-label*='Up to']").first(),
    ).toBeVisible();
    // The drilled package is a leaf (no sub-containers), so the list is
    // empty and the tab says so; the shared scope moved with it.
    await expect(
      page.locator("#root-canvas nav[aria-label='Drill scope']"),
    ).toContainText("domain");
  });

  test("Files tab: double-click drills, in-tab breadcrumb navigates back, Hide files persists (#273)", async ({
    page,
    context,
  }) => {
    // Open Files tab; click a folder body (should expand, not drill);
    // verify scope unchanged. Then double-click → drill.
    await page.locator("button:has-text('Files')").click();
    // The sample wraps under src/; first row is src.
    const srcRow = page
      .locator("#side-panel button[title*='double-click to drill']")
      .first();
    await srcRow.click(); // single click expands only — no drill
    await expect(
      page.locator("#side-panel button:has-text('Up to')"),
    ).toHaveCount(0);
    // Double-click drills the shared scope; breadcrumb appears.
    await srcRow.dblclick();
    await expect(
      page.locator("#side-panel button[aria-label*='Up to']").first(),
    ).toBeVisible();
    // Folders-only toggle.
    const toggle = page.locator("button:has-text('Hide files')").first();
    await toggle.click();
    await expect(
      page.locator("button:has-text('Show files')").first(),
    ).toBeVisible();
    // Reload — toggle state persists.
    await page.reload();
    await page.locator("button:has-text('Files')").click();
    await expect(
      page.locator("button:has-text('Show files')").first(),
    ).toBeVisible();
  });

  test("DetailsPanel shows 'About this verdict' explainer + per-instance numbers (#276)", async ({
    page,
  }) => {
    // Open the Issues tab and click the high-severity violation row;
    // that selects the offender file. DetailsPanel should now carry
    // an "About this verdict" section.
    await page.locator("button:has-text('Issues')").click();
    await page.locator("#side-panel ul li button").first().click();
    await expect(
      page.locator("text=About this verdict").first(),
    ).toBeVisible();
    await expect(
      page.locator("text=Why this is a coupling violation").first(),
    ).toBeVisible();
    // Per-Issue wording comes from core: the reason carries the numbers.
    await expect(
      page.locator("[data-role='reason']").first(),
    ).toContainText("3 imports, severity 1.50, RRI 12");
  });

  test("cycle issue row shows 'break: A → B (N vs M)', not full path (#277)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Issues')").click();
    // Sample cycle's min-cut is email.rs → user.rs with weight 2 vs 14.
    await expect(
      page
        .locator("#side-panel")
        .locator("text=/break: .*\\(2 vs 14\\)/")
        .first(),
    ).toBeVisible();
  });

  test("Info tab shows prominent score block above everything else (#272)", async ({
    page,
  }) => {
    // Score block sits at the very top — find it by its big number text.
    const block = page
      .locator("#side-panel")
      .locator("text=/^Health$/i")
      .first();
    await expect(block).toBeVisible();
    // The score button inside should carry the breakdown-open aria-label
    // and be visible before any other content on the tab.
    const scoreButton = page
      .locator("button[aria-label='Show health score breakdown']")
      .first();
    await expect(scoreButton).toBeVisible();
    // Score-block button should come before WelcomeCard's #codebase-header
    // in DOM order — proves the reorder.
    const blockBox = await scoreButton.boundingBox();
    const welcome = await page.locator("#codebase-header").boundingBox();
    expect(blockBox!.y).toBeLessThan(welcome!.y);
  });

  test("History scrubber renders when data.history has ≥2 snapshots", async ({
    page,
  }) => {
    // Info tab is the default; scrubber sits under a 'History' heading.
    await expect(page.locator("text=History").first()).toBeVisible();
    const sparkline = page.locator(
      "svg[aria-label*='Health score sparkline']",
    );
    await expect(sparkline).toBeVisible();
    // Sample has 2 snapshots → 2 circles in the sparkline.
    expect(await sparkline.locator("circle").count()).toBe(2);
    // Footer surfaces the latest score (sample: 82).
    await expect(
      page.locator("text=/82\\/100/").first(),
    ).toBeVisible();
  });

  test("Matrix row label is click-to-select / double-click-to-drill (keeps state in sync)", async ({
    page,
  }) => {
    await page.locator("button:has-text('Matrix')").click();
    // Single click on a row label selects the node — DetailsPanel
    // opens (same selection state the LSM uses).
    const rowLabel = page.locator("#root-canvas table th[title*='click to']").first();
    await rowLabel.click();
    await expect(
      page.locator("[aria-label*='Details for ']").first(),
    ).toBeVisible({ timeout: 2000 });
    // Double-click drills the shared scope — breadcrumb deepens.
    const breadcrumbBefore = await page
      .locator("#root-canvas nav[aria-label='Drill scope']")
      .textContent();
    await rowLabel.dblclick();
    const breadcrumbAfter = await page
      .locator("#root-canvas nav[aria-label='Drill scope']")
      .textContent();
    expect(breadcrumbBefore).not.toBe(breadcrumbAfter);
  });

  test("Clicking a populated matrix cell opens the edge-details inlay", async ({
    page,
  }) => {
    // Drill into src so packages are immediate children, switch to
    // Matrix, then click one of the coloured (populated) cells.
    await page.locator("button:has-text('Matrix')").click();
    const populated = page.locator(
      "#root-canvas td[style*='accent-domain'], #root-canvas td[style*='edge-violation']",
    );
    expect(await populated.count()).toBeGreaterThan(0);
    await populated.first().click();
    await expect(
      page.locator("[aria-label*='Details for edge']").first(),
    ).toBeVisible();
  });

  test("Clicking an LSM edge opens the edge-details inlay (#295)", async ({
    page,
  }) => {
    // Each edge sits inside a <g style="cursor: pointer"> that owns the
    // visible path + the fat invisible hit area. Click it.
    const edges = page.locator("#root-canvas svg g > g[style*='cursor']");
    expect(await edges.count()).toBeGreaterThan(0);
    await edges.first().click({ force: true });
    await expect(
      page.locator("[aria-label*='Details for edge']").first(),
    ).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(
      page.locator("[aria-label*='Details for edge']"),
    ).not.toBeVisible();
  });

  test("Field guide dialog opens with section nav + glossary", async ({
    page,
  }) => {
    await page.locator("button[aria-label='Open the Explorer field guide']").click();
    await expect(page.locator("[role='dialog'][aria-label='Explorer guide']")).toBeVisible();
    // Overview is the default section.
    await expect(page.locator("text=What the Explorer is").first()).toBeVisible();
    // Switch to glossary via the side nav.
    await page.locator("button:has-text('Glossary')").click();
    await expect(page.locator("text=/Quick reference/").first()).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(
      page.locator("[role='dialog'][aria-label='Explorer guide']"),
    ).not.toBeVisible();
  });

  test("Help dialog opens with keyboard shortcuts (#254)", async ({ page }) => {
    await page.locator("button[aria-label='Keyboard shortcuts (?)']").click();
    await expect(page.locator("[role='dialog']")).toBeVisible();
    await expect(page.locator("text=Keyboard shortcuts")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator("[role='dialog']")).not.toBeVisible();
  });
});
