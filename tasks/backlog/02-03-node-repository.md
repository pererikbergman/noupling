---
description: Implement NodeRepository for bulk inserting and querying nodes
type: Task
story: 02-03
---

# Task: NodeRepository

### Acceptance Criteria

- [x] `bulk_insert(nodes: &[Node])` inserts multiple nodes in a transaction.
- [x] `get_by_snapshot(snapshot_id) -> Vec<Node>` retrieves all nodes for a snapshot.
- [x] `get_children(parent_id) -> Vec<Node>` retrieves child nodes.
- [x] All methods have passing unit tests.

### Implementation Steps

- [x] 1. Write failing tests for bulk_insert, get_by_snapshot, get_children.
- [x] 2. Implement NodeRepository methods.
- [x] 3. Verify tests pass.
