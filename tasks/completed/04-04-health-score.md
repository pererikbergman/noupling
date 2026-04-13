---
description: Health score computation 0-100
type: Task
story: 04-04
---

# Task: Health Score

### Acceptance Criteria

- [x] Score = 100 * (1 - sum_severity / total_modules).
- [x] Perfect project with no violations scores 100.
- [x] Score clamps to 0 minimum.
- [x] Unit tests verify scoring with known inputs.
