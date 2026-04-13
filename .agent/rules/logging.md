---
trigger: always_on
---

# Logging & Trajectory Rules

## 1. Trajectory Format

Every major action must be logged in `logs/trajectories/` using the following structure:

- **Thought:** What are you trying to achieve?
- **Action:** What tool/skill are you invoking?
- **Observation:** What was the result of that action (Error, Success, Output)?
- **Reflection:** What was learned? (e.g., "The API endpoint was deprecated, switching to v2").

*Keep logs concise to preserve context tokens. If a trajectory log becomes exceedingly long, summarize it and rotate to a new file (e.g., `trajectory-002.md`).*

## 2. Feedback Loop

- If a human provides a correction, save it immediately to `logs/feedback/critical-notes.md`.
- This file must be read at the start of every new session to avoid repeating mistakes.
