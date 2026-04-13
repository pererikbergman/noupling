---
name: atomic-commit
description: Standard for committing code and log changes
---

# Atomic-Commit Skill

- **Protocol:**
  1. One commit per single, logical change.
  2. Format: `type(scope): <short-summary>`. Types: `feat`, `fix`, `refactor`, `chore`, `docs`.
  3. Always stage the relevant `.agent/` and `tasks/` updates (like moved task files) in the same commit as the code.
