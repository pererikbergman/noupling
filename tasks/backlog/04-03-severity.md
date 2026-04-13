---
description: Severity calculation S = 1/(depth+1)
type: Task
story: 04-03
---

# Task: Severity Calculation

### Acceptance Criteria

- [x] Severity = 1.0 / (depth + 1) for each violation.
- [x] Root-level violations (depth 0) have severity 1.0.
- [x] Deeper violations have lower severity.
- [x] Unit tests verify calculation at various depths.
