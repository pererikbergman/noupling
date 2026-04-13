---
description: Top-down BFS sibling coupling detection
type: Task
story: 04-02
---

# Task: BFS Coupling Detection

### Acceptance Criteria

- [x] Walk from depth 0, checking sibling directory pairs for coupling.
- [x] If D_acc(A) references a file in D_acc(B), flag as coupling violation.
- [x] Returns list of CouplingViolation structs.
- [x] Unit tests verify detection with known violation patterns.

### Implementation Steps

- [x] 1. Write failing tests for violation detection.
- [x] 2. Implement BFS audit logic.
- [x] 3. Verify tests pass.
