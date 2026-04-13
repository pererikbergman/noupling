---
description: Bottom-up dependency aggregation and top-down BFS coupling audit
type: Spec
---

# Epic: Analyzer Slice

### 1. Objective & Value Proposition

* **Goal:** Implement the mathematical analysis engine that computes coupling violations and health scores.
* **User Value:** The core value proposition: quantifies architectural health with actionable severity scores.

### 2. User Stories

* As a user, I want D_acc computed for every directory so that accumulated dependencies are visible.
* As a user, I want sibling coupling violations detected via BFS so that I know where boundaries are broken.
* As a user, I want severity calculated as 1/(depth+1) so that root-level violations are flagged as most critical.
* As a user, I want an overall health score (0-100) so that I can track architectural drift.
* As a user, I want circular dependencies detected and flagged with Severity 1.0.

### 3. Functional Requirements

* Bottom-up aggregation: compute D_acc(N) as the union of all sub-tree dependencies, excluding internal deps.
* Top-down BFS: walk from depth 0, checking sibling pairs for coupling violations.
* Severity calculation: S = 1 / (depth + 1).
* Health score: 100 * (1 - (sum_severity / total_modules)).
* Circular dependency detection during BFS, flagged as "Structural Loop" with Severity 1.0.
* Use efficient hashing (fxhash) for set union operations.

### 4. Acceptance Criteria

- [ ] D_acc correctly accumulates and excludes internal dependencies.
- [ ] BFS detects coupling violations between sibling directories.
- [ ] Severity is correctly computed based on depth.
- [ ] Health score formula produces correct results.
- [ ] Circular dependencies are detected and reported.
- [ ] All analysis methods have unit tests with known inputs/outputs.
