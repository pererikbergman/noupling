---
description: SQLite persistence layer with schema migrations and repository patterns
type: Spec
---

# Epic: Storage Slice

### 1. Objective & Value Proposition

* **Goal:** Implement the SQLite persistence layer that stores snapshots, nodes, and dependencies.
* **User Value:** Enables trend analysis by persisting scan results locally in `.noupling/history.db`.

### 2. User Stories

* As a user, I want the database auto-initialized on first run so that I don't need manual setup.
* As a developer, I want a repository pattern for snapshots so that storage concerns are isolated.
* As a developer, I want a repository pattern for nodes so that tree structures are persisted correctly.
* As a developer, I want a repository pattern for dependencies so that import relationships are stored with line numbers.

### 3. Functional Requirements

* Auto-create `.noupling/history.db` with the specified schema on first access.
* Implement SnapshotRepository: create, get_by_id, get_latest.
* Implement NodeRepository: bulk_insert, get_by_snapshot, get_children.
* Implement DependencyRepository: bulk_insert, get_by_snapshot.
* All repositories must use parameterized queries (no SQL injection).

### 4. Acceptance Criteria

- [ ] Database file is created at `.noupling/history.db` automatically.
- [ ] Schema matches the specification exactly (snapshots, nodes, dependencies tables).
- [ ] All repository methods have passing unit tests.
- [ ] UUID generation works for snapshot and node IDs.
