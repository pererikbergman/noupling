# Report Formats

noupling generates reports in 7 formats. All reports are saved to `.noupling/`.

## HTML (`--format html`)

Interactive static HTML with drill-down navigation, saved to `.noupling/report/`.

- Color-coded health scores per directory (green/yellow/red)
- Click into directories to explore the tree
- Circular dependencies with expandable cycle details and per-hop XS counts
- Coupling violations sorted by severity

Best for: exploring violations interactively, sharing with team members.

## JSON (`--format json`)

Comprehensive JSON with the full directory tree, grouped circular dependencies, coupling details, and hotspots. Saved to `.noupling/report.json`.

Includes: `score`, `total_modules`, `total_xs`, `suppressed_count`, `circular_dependencies` (grouped by order), `coupling_violations`, `hotspots`, `directory_tree`.

Best for: programmatic consumption, custom dashboards, CI integrations.

## XML (`--format xml`)

Same structure as JSON but in XML format. Saved to `.noupling/report.xml`.

Best for: tools that consume XML (enterprise CI systems, XSLT pipelines).

## Markdown (`--format md`)

Multi-file navigable Markdown with a `README.md` per directory. Saved to `.noupling/report-md/`.

Each directory page includes a summary table, contents listing, and violations. Pages link to parent and child directories.

Best for: GitHub/GitLab wiki, documentation sites, text-based review.

## SonarCloud (`--format sonar`)

Generic issue import format for SonarCloud/SonarQube. Saved to `.noupling/noupling-sonar.json`.

Add to `sonar-project.properties`:

```properties
sonar.externalIssuesReportPaths=.noupling/noupling-sonar.json
```

Issues use rule IDs `noupling:circular-dependency` and `noupling:coupling` with appropriate severity levels (CRITICAL, MAJOR, MINOR).

Best for: integrating architectural violations into your SonarCloud dashboard.

## Mermaid (`--format mermaid`)

Mermaid flowchart diagram of module dependencies. Saved to `.noupling/report.mermaid`.

Nodes are color-coded: green (healthy), orange (coupled), red (circular). Paste into any Markdown renderer that supports Mermaid.

Best for: quick visual overview, embedding in documentation.

## DOT (`--format dot`)

GraphViz DOT format dependency graph. Saved to `.noupling/report.dot`.

Render with:

```bash
dot -Tpng .noupling/report.dot -o graph.png
```

Best for: high-quality graph rendering, custom visualization pipelines.
