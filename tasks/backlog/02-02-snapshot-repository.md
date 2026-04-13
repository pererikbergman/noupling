---
description: Implement SnapshotRepository for creating and querying snapshots
type: Task
story: 02-02
---

# Task: SnapshotRepository

### Acceptance Criteria

- [x] `create(root_path) -> Snapshot` inserts and returns a new snapshot with UUID.
- [x] `get_by_id(id) -> Option<Snapshot>` retrieves a specific snapshot.
- [x] `get_latest() -> Option<Snapshot>` retrieves the most recent snapshot.
- [x] All methods have passing unit tests.

### Implementation Steps

- [x] 1. Write failing tests for create, get_by_id, get_latest.
- [x] 2. Implement SnapshotRepository methods.
- [x] 3. Verify tests pass.
