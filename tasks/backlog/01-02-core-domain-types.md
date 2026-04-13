---
description: Define core domain types (Node, Dependency, Snapshot) with serde support
type: Task
story: 01-02
---

# Task: Define Core Domain Types

### Context

All slices need a common vocabulary for nodes, dependencies, and snapshots. These live in the `core` module.

### Objective

Define the core data structures that represent the project tree, import dependencies, and scan snapshots.

### Acceptance Criteria

- [ ] `NodeType` enum with FILE and DIR variants.
- [ ] `Node` struct with id, snapshot_id, parent_id, name, path, node_type, depth.
- [ ] `Dependency` struct with from_node_id, to_node_id, line_number.
- [ ] `Snapshot` struct with id, timestamp, root_path.
- [ ] All types derive Debug, Serialize, Deserialize.
- [ ] Unit tests verify serialization roundtrip.

### TDD Strategy

1. **Red:** Write tests that construct each type and verify serde JSON roundtrip.
2. **Green:** Define the structs/enums to make tests pass.
3. **Refactor:** Ensure naming and field types match the SQLite schema exactly.

### Implementation Steps

- [ ] 1. Write failing tests for Node, Dependency, Snapshot serialization.
- [ ] 2. Define NodeType enum.
- [ ] 3. Define Node, Dependency, Snapshot structs.
- [ ] 4. Verify all tests pass.
