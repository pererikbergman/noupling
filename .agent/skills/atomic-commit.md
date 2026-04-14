---
name: atomic-commit
description: Standard for committing code changes
---

# Atomic Commit

1. One commit per single logical change.
2. Format: `type(scope): short-summary`
   - Types: `feat`, `fix`, `refactor`, `chore`, `docs`
   - Scope: module name (scanner, analyzer, reporter, cli, etc.)
3. Reference GitHub Issues when applicable: `feat(scanner): add Ruby parser (#57)`
4. Close issues in commit messages: `Closes #57`
5. Always run `cargo test` and `cargo clippy` before committing.
