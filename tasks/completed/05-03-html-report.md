---
description: Static HTML report with hierarchical drill-down navigation
type: Task
story: 05-03
---

# Task: HTML Report Generator

### Acceptance Criteria

- [x] `noupling report --format html` generates static HTML in `.noupling/report/`.
- [x] Landing page shows root score, child directories with scores and violation indicators.
- [x] Clicking a directory navigates to its page showing children and violations.
- [x] Breadcrumb navigation at top of each page.
- [x] Color-coded scores (green >= 90, yellow >= 70, red < 70).
- [x] Child nodes with violations show a warning indicator.
- [x] Circular dependencies show direction indicator (fewer couplings = wrong direction).
- [x] All CSS/JS embedded (no external dependencies).
- [x] Unit tests verify HTML generation.
