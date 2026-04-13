---
description: Implement DependencyRepository for bulk inserting and querying dependencies
type: Task
story: 02-04
---

# Task: DependencyRepository

### Acceptance Criteria

- [x] `bulk_insert(deps: &[Dependency])` inserts multiple dependencies in a transaction.
- [x] `get_by_snapshot(snapshot_id) -> Vec<Dependency>` retrieves all deps for a snapshot (via node join).
- [x] All methods have passing unit tests.

### Implementation Steps

- [x] 1. Write failing tests for bulk_insert, get_by_snapshot.
- [x] 2. Implement DependencyRepository methods.
- [x] 3. Verify tests pass.
