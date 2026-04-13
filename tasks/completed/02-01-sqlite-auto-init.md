---
description: Auto-initialize SQLite database with schema on first access
type: Task
story: 02-01
---

# Task: SQLite Auto-Initialize

### Acceptance Criteria

- [x] `.noupling/` directory is created if it doesn't exist.
- [x] `history.db` is created with snapshots, nodes, dependencies tables.
- [x] Schema matches the spec exactly (column names, types, constraints).
- [x] Calling init multiple times is idempotent (CREATE TABLE IF NOT EXISTS).
- [x] Unit tests verify table creation.

### Implementation Steps

- [x] 1. Write failing test that opens DB and verifies tables exist.
- [x] 2. Implement `Database` struct with `open` method.
- [x] 3. Implement schema migration in `initialize_schema`.
- [x] 4. Verify tests pass.
