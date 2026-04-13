---
description: Recursive file discovery with tree building and depth tracking
type: Task
story: 03-01
---

# Task: File Discovery

### Acceptance Criteria

- [x] Recursively walk a directory building Node tree with parent/child relationships.
- [x] Each node has correct depth, name, path, and node_type (FILE/DIR).
- [x] Respects common ignore patterns (.git, target, node_modules).
- [x] Unit tests verify tree structure against a temp directory.

### Implementation Steps

- [x] 1. Write failing tests for tree building from a temp directory.
- [x] 2. Implement `discover_files` function.
- [x] 3. Verify tests pass.
