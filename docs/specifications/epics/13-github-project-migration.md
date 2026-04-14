---
description: Migrate roadmap epics and stories to GitHub Issues and Project board
type: Spec
---

# Task: Migrate Roadmap (Epics & Stories) to GitHub Project

I want to move our high-level roadmap, epics, and user stories into GitHub Issues and link them to a GitHub Project (v2).

## Context
- **Source Material:** `docs/specifications/roadmap.md` and `docs/specifications/epics/*.md`
- **Target Repository:** https://github.com/pererikbergman/noupling
- **Project Name:** noupling Roadmap

## Requirements
1. **Schema Mapping:**
   - Parse the source documents to identify "Epics" (Milestones) and "Stories/Tasks" (Issues).
   - Identify labels for each (e.g., `feature`, `bug`, `architecture`, `documentation`, `ci/cd`, `parser`).
2. **Automation Script:**
   - Generate a shell script using the GitHub CLI (`gh`) to:
     - Create a **GitHub Milestone** for each Epic.
     - Create **GitHub Issues** for each Story, assigned to the correct Milestone.
     - Apply appropriate labels to each issue.
3. **Project Integration:**
   - Provide instructions or a script snippet to add these newly created issues to a specific **GitHub Project (v2)** board.
4. **Issue Templates:**
   - Based on the stories, create a `.github/ISSUE_TEMPLATE/task_template.md` to ensure future issues maintain the same quality as the existing feature_request and bug_report templates.

## Existing Epics to Migrate
- Epic 07: Diff Mode (PR/CI Gate)
- Epic 09: Homebrew Tap Infrastructure
- Epic 10: Documentation Overhaul
- Epic 11: CI/CD and Version Management
- Epic 12: Gold Standard README

## Constraints
- **Format:** Pure Markdown and Shell script. DO NOT use LaTeX.
- **Dry Run:** The script should include a way to "echo" the commands before executing them to prevent accidental bulk creation.
- **Vertical Slicing:** Ensure the issues are organized by "Features/Slices" rather than just horizontal layers.

## Acceptance Criteria
- [ ] All open epics have corresponding GitHub Milestones.
- [ ] All stories/tasks have corresponding GitHub Issues with labels.
- [ ] Issues are linked to their Milestone.
- [ ] A GitHub Project (v2) board is created with all issues added.
- [ ] A task issue template is created for future use.
- [ ] The migration script supports dry-run mode.
- [ ] Labels are created and applied consistently.

Please start by summarizing the Epics and Stories you find in the source documents and propose the label set we should use.
