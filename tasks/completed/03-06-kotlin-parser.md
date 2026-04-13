---
description: Kotlin grammar parser for import declarations
type: Task
story: 03-06
---

# Task: Kotlin Parser

### Acceptance Criteria

- [x] Add tree-sitter-kotlin dependency.
- [x] Parse Kotlin `import` declarations with line numbers.
- [x] Update discovery to include .kt and .kts files.
- [x] Update scan_project to route .kt files to Kotlin parser.
- [x] Unit tests verify parsing against known Kotlin source strings.
- [x] Integration test with mock Kotlin fixture.
