---
trigger: always_on
description: Core development rules for all code changes
---

# Development Rules

## Code Quality

- Run `cargo fmt` after every change.
- Run `cargo clippy -- -D warnings` before committing. Zero warnings allowed.
- Run `RUSTFLAGS="-Dwarnings" cargo test` before committing. This matches CI which treats warnings as errors.
- All tests must pass on all platforms (Linux, macOS, Windows).

## TDD (Test-Driven Development)

Follow Red-Green-Refactor for all implementations:

1. **Red**: Write a failing test first.
2. **Green**: Write the simplest code to make it pass.
3. **Refactor**: Clean up while keeping tests green.

**Exception**: Spikes for exploratory work. Label the commit as `Spike`.

## Commits

- Format: `type(scope): short-summary`
- Types: `feat`, `fix`, `refactor`, `chore`, `docs`
- One logical change per commit.

## Architecture

- **No root-level coupling**: If module A needs data from module B, define a local type.
- **Modules are independent**: scanner, storage, analyzer, reporter should not depend on each other directly. Use `core/` for shared types.
- Keep the flat `src/` layout. No nested module groups.

## Documentation

Every feature addition or modification MUST include documentation updates in the same PR:

- **README.md**: Update usage examples, CLI flags, configuration options, or feature list.
- **`--help` text**: Update clap doc comments in `src/cli.rs` for any new or changed flags.
- **CHANGELOG.md**: Add an entry under the next version section.
- **docs/architecture.md**: Update if the data flow or module responsibilities change.
- **CONTRIBUTING.md**: Update if the development workflow or project structure changes.
- **`.agent/knowledge/`**: Update project-map.md or tech-stack.md if architecture or dependencies change.

A PR without documentation for user-facing changes is incomplete.

## Safety

- Never delete files in `docs/` or `.agent/` without confirmation.
- If a task requires modifying unrelated files, stop and create a new GitHub Issue.
