---
description: End-to-end CLI command wiring connecting all slices
type: Spec
---

# Epic: CLI Integration

### 1. Objective & Value Proposition

* **Goal:** Wire the scan, audit, and report subcommands to their respective slices for end-to-end functionality.
* **User Value:** A working CLI tool that can be installed and used against real projects.

### 2. User Stories

* As a user, I want `noupling scan <PATH>` to discover files, parse ASTs, and store results in SQLite.
* As a user, I want `noupling audit` to run BFS analysis on the latest snapshot and display violations.
* As a user, I want `noupling audit --snapshot <ID>` to audit a specific historical snapshot.
* As a user, I want `noupling report --format json` to output JSON to stdout.
* As a user, I want `noupling report --format md` to output Markdown to stdout.

### 3. Functional Requirements

* `scan` command: orchestrate scanner -> storage pipeline.
* `audit` command: load from storage -> run analyzer -> display results.
* `report` command: load latest audit -> generate formatted output.
* All commands must use tracing for structured logging.
* Single static binary output.

### 4. Acceptance Criteria

- [ ] `noupling scan <PATH>` completes and stores data in `.noupling/history.db`.
- [ ] `noupling audit` displays violations sorted by severity.
- [ ] `noupling report --format json` outputs valid JSON.
- [ ] `noupling report --format md` outputs valid Markdown.
- [ ] Integration tests pass against `tests/fixtures/` mock project.
- [ ] `cargo install --path .` produces a working binary.
