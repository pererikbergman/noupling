---
description: JSON and Markdown report generation from audit results
type: Spec
---

# Epic: Reporter Slice

### 1. Objective & Value Proposition

* **Goal:** Generate structured output (JSON and Markdown) from audit results.
* **User Value:** Consumable reports for CI pipelines (JSON) and human review (Markdown).

### 2. User Stories

* As a user, I want JSON output with `critical_violations` and `score` fields so that CI can parse results.
* As a user, I want Markdown output with formatted tables and sections so that I can review findings easily.
* As a user, I want violations sorted by severity (highest first) so that critical issues are immediately visible.

### 3. Functional Requirements

* JSON reporter: serialize audit results to JSON with `critical_violations`, `score`, `violations[]`, and `snapshot_id`.
* Markdown reporter: generate a human-readable report with a summary table, violation details, and health score.
* Violations must be sorted by severity descending in both formats.
* Never use the long dash character in CLI text output.

### 4. Acceptance Criteria

- [ ] JSON output is valid and contains all required fields.
- [ ] Markdown output is well-formatted with tables and headings.
- [ ] Violations are sorted by severity (highest first).
- [ ] Unit tests verify output structure for both formats.
