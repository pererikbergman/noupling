---
trigger: always_on
description: Rules for working with GitHub Issues, Milestones, and PRs
---

# GitHub Workflow Rules

## Before Starting Work

1. Check the GitHub Project board for the next priority issue.
2. Assign yourself to the issue.
3. Read the issue description and acceptance criteria.

## During Implementation

- Reference the issue number in commits: `feat(scanner): add Ruby parser (#57)`
- Follow TDD and development rules.
- Keep changes focused on the issue scope.

## Completing Work

1. Ensure all tests pass and clippy is clean.
2. Close the issue with a commit message: `Closes #57`
3. If the milestone is fully completed, it will auto-close.
4. Push changes and verify CI passes.

## Creating New Issues

- Use the appropriate template (bug, feature, task).
- Assign to the correct milestone.
- Add relevant labels (parser, architecture, ci/cd, documentation, reporter).
- Add to the "noupling Roadmap" project board.

## Labels

| Label | Use for |
| :--- | :--- |
| `enhancement` | New features |
| `bug` | Bug fixes |
| `documentation` | Docs changes |
| `architecture` | Structural changes |
| `ci/cd` | CI/CD pipeline |
| `parser` | Language parser work |
| `reporter` | Report generation |
| `epic` | High-level feature set |
