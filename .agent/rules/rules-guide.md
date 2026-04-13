---
trigger: model_decision
description: Guide for creating and structuring rules with proper YAML frontmatter and best practices.
---

# Rule Creation Guide

## Constraints

- **Format:** Markdown + YAML Frontmatter.
- **Size:** Max 6,000 chars per file.
- **Total Budget:** Max 12,000 chars across all active rules.

## Schema

```yaml
---
name: [kebab-case-identifier]
description: [One sentence explaining WHEN to use this rule]
trigger: model_decision
globs: [File patterns, e.g., "**/commonMain/**/*.kt"]
always_apply: false
user-invocable: true
priority: 10
intent: [e.g., "refactor", "feature-logic", or "fix"]
---
```

## Protocol

- **Be Specific:** Target unique project patterns, NOT general Kotlin knowledge.
- **Consolidate:** Avoid many small/redundant files.
- **Align:** Connect rules to Fixed Skills via the "Supports:" header.
- **Iterate:** Update rules based on project evolution or tool feedback.
- **Safety:** NEVER duplicate Cascade's default agent knowledge.
