---
description: Standard workflow for implementing a GitHub Issue
---

# Implement Issue Workflow

## Phase 1: Preparation

1. Read the GitHub Issue description and acceptance criteria.
2. Check `.agent/knowledge/project-map.md` for architecture context.
3. Check `.agent/knowledge/tech-stack.md` to avoid adding redundant dependencies.
4. Plan the implementation approach.

## Phase 2: Implementation

1. Follow TDD (Red-Green-Refactor) per `.agent/rules/development.md`.
2. Keep changes scoped to the issue. If scope expands, create a new Issue.
3. Commit atomically per `.agent/skills/atomic-commit.md`.

## Phase 3: Completion

1. Run `cargo test` - all tests pass.
2. Run `cargo clippy -- -D warnings` - zero warnings.
3. Run `cargo fmt --check` - clean formatting.
4. Commit with `Closes #<issue-number>` in the message.
5. Push and verify CI passes.
6. If this was the last issue in a milestone, the milestone auto-closes.

## Phase 4: Knowledge Update

- If a new dependency was added, update `.agent/knowledge/tech-stack.md`.
- If architecture changed, update `.agent/knowledge/project-map.md`.
- If README usage changed, update `README.md`.
