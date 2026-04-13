---
description: Wire audit command to load from storage and run analyzer
type: Task
story: 06-02
---

# Task: Wire Audit Command

### Acceptance Criteria

- [x] `noupling audit` loads latest snapshot, runs analyzer, displays violations and score.
- [x] `noupling audit --snapshot <ID>` audits specific snapshot.
- [x] Violations displayed sorted by severity.
