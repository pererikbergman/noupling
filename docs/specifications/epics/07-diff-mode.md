---
description: Run analysis only on files changed compared to a base branch
type: Spec
---

# Epic: Diff Mode (PR/CI Gate)

### 1. Objective & Value Proposition

* **Goal:** Allow noupling to analyze only the files that changed compared to a base branch (e.g., main or master), reporting only violations introduced or affected by those changes.
* **User Value:** Enables noupling as a CI/PR gate. Teams can run it on every pull request without being blocked by existing violations in untouched code. Only new issues are flagged, making adoption incremental.

### 2. User Stories

* As a developer, I want to run `noupling scan . --diff-base main` so that only files changed compared to main are analyzed.
* As a developer, I want `noupling scan .` (without the flag) to continue scanning the entire codebase as it does today.
* As a CI engineer, I want to add noupling to our PR pipeline so that it fails only if the PR introduces new coupling violations or circular dependencies.
* As a developer, I want the diff mode to still detect circular dependencies if my changes create a new cycle, even if the other files in the cycle were not changed.
* As a team lead, I want the report to clearly distinguish between "new violations introduced in this diff" and the overall project health.

### 3. Functional Requirements

* Add `--diff-base <branch>` optional flag to the `scan` command.
* When `--diff-base` is provided:
  - Use `git diff --name-only <branch>...HEAD` to get the list of changed files.
  - Filter discovered modules to only include changed files.
  - Still resolve dependencies from changed files to ANY file in the project (not just changed ones), since a new import in a changed file can create coupling to an existing unchanged file.
  - Run the full D_acc and BFS analysis on the filtered set, but only report violations where at least one file in the violation is a changed file.
  - For circular dependencies, report a cycle if any file in the cycle chain was changed.
* When `--diff-base` is omitted, behavior is unchanged (full project scan).
* The audit and report commands should indicate whether the results are from a full scan or a diff scan.
* The snapshot should store whether it was a diff scan and what the base branch was.

### 4. Technical Considerations

* The scan still needs the full project's module list for dependency resolution (you can't resolve imports without knowing all files). The diff filter applies to which violations are reported, not which files are parsed.
* Approach: scan everything, then filter violations to only those involving at least one changed file.
* Git integration: shell out to `git diff --name-only <branch>...HEAD` from the project root.
* Edge case: deleted files should remove dependencies but not create new violations.
* Edge case: renamed files should be treated as new files.

### 5. CLI Interface

```
# Full scan (existing behavior)
noupling scan /path/to/project

# Diff scan against main
noupling scan /path/to/project --diff-base main

# Diff scan against specific branch
noupling scan /path/to/project --diff-base origin/develop

# Audit shows diff context
noupling audit /path/to/project
# Output: "Diff scan against main: 3 new violations in 12 changed files"

# Reports include diff metadata
noupling report /path/to/project --format sonar
# Sonar report only contains issues from changed files
```

### 6. Acceptance Criteria

- [ ] `noupling scan . --diff-base main` only reports violations involving changed files.
- [ ] `noupling scan .` without the flag scans the entire project (no behavior change).
- [ ] Changed files that introduce new coupling are detected.
- [ ] Changed files that create new circular dependencies are detected.
- [ ] Circular dependencies are reported if any hop in the cycle involves a changed file.
- [ ] Deleted files do not produce violations.
- [ ] Audit output indicates whether results are from a full or diff scan.
- [ ] Reports include diff metadata (base branch, changed file count).
- [ ] Sonar report only contains issues from changed files when in diff mode.
- [ ] Works with both `main` and `master` as base branches.
- [ ] Works with remote refs like `origin/main`.
