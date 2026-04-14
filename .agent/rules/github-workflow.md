---
trigger: always_on
description: Rules for working with GitHub Issues, Milestones, and PRs
---

# GitHub Workflow Rules

## Before Starting Work

1. Select the next issue following `.agent/rules/issue-priority.md`.
2. Assign yourself to the issue.
3. Read the issue description and acceptance criteria.

## Branch Rules

**Every issue MUST be worked on in a dedicated branch. Never commit directly to main.**

1. Always start from an up-to-date main:
   ```bash
   git checkout main
   git pull
   ```
2. Create a branch named `feature/<issue-number>-<short-description>`:
   ```bash
   git checkout -b feature/72-exit-code-threshold
   ```
3. For bug fixes, use `fix/` prefix:
   ```bash
   git checkout -b fix/99-windows-path-crash
   ```
4. One branch per issue. Do not combine unrelated issues.

## During Implementation

- Reference the issue number in commits: `feat(scanner): add Ruby parser (#85)`
- Follow TDD and development rules from `.agent/rules/development.md`.
- Keep changes focused on the issue scope.

## Creating a Pull Request

1. Push the branch:
   ```bash
   git push -u origin feature/72-exit-code-threshold
   ```
2. Create a PR with `gh pr create`:
   - Title: clear description of the change
   - Body: summary, what changed, `Closes #<issue-number>`
   - Target: `main`
3. Wait for CI to pass on all platforms.
4. Never merge your own PR without CI passing.

## Completing Work

1. Ensure all tests pass and clippy is clean.
2. PR is merged via squash merge (keeps history clean).
3. The `Closes #72` in the PR body auto-closes the issue.
4. If the milestone is fully completed, close it manually.
5. Delete the branch after merge.

## Creating New Issues

- Use the appropriate template (bug, feature, task).
- Assign to the correct milestone (if applicable).
- Add relevant labels: type (enhancement, bug, parser, etc.), priority, impact, effort.
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
| `priority: now/critical/high/medium/low` | Work order |
| `impact: high/medium/low` | User value |
| `effort: tiny/small/medium/large` | Implementation size |
