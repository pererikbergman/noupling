---
trigger: always_on
---

# Task Management Rules

All project work must be driven by tasks defined in the `tasks/` directory.

1. **Creating Tasks:**
   - Scaffold new tasks in `tasks/backlog/<task-name>.md`.
   - Each task file must contain: Context, Objective, Acceptance Criteria, Technical Details, and Implementation Steps.

2. **Implementing Tasks:**
   - Before writing code, ensure the task exists in the backlog.
   - Implement the task strictly following TDD (Red-Green-Refactor).
   - Check off the completion steps inside the markdown file as you progress.

3. **Completing Tasks:**
   - **Update the Knowledge Base:** Review if this implementation shifted any architectural boundaries or added new domains. If so, you **must** update `.agent/knowledge/project-map.md`.
   - **Update Tech Stack Log:** If a new third-party dependency was introduced, document the rationale and version in `.agent/knowledge/tech-stack.md` to prevent future redundancy.
   - **Update README:** If command-line usage, public APIs, or setup instructions changed, update the root `README.md`.
   - Ask the user for validation once implementation is complete.
   - Upon approval, move the task file using: `mv tasks/backlog/<task-name>.md tasks/completed/<task-name>.md`.
   - Commit the codebase changes, the moved task file, and any updated knowledge/docs files atomically.
