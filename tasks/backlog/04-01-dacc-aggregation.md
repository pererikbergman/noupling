---
description: Bottom-up D_acc aggregation of dependencies per directory
type: Task
story: 04-01
---

# Task: D_acc Aggregation

### Acceptance Criteria

- [x] Derive directory hierarchy from module file paths.
- [x] For every virtual directory, compute D_acc as union of all sub-tree dependencies.
- [x] Internal dependencies (within same directory) are excluded from D_acc.
- [x] Unit tests verify aggregation with known inputs.

### Implementation Steps

- [x] 1. Write failing tests for D_acc computation.
- [x] 2. Implement directory tree derivation from file paths.
- [x] 3. Implement bottom-up D_acc aggregation.
- [x] 4. Verify tests pass.
