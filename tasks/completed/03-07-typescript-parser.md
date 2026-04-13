---
description: TypeScript/TSX grammar parser for import declarations
type: Task
story: 03-07
---

# Task: TypeScript Parser

### Acceptance Criteria

- [x] Add tree-sitter-typescript dependency.
- [x] Parse TypeScript/TSX `import` declarations with line numbers.
- [x] Update discovery to include .ts and .tsx files.
- [x] Update scan_project to route .ts/.tsx files to TypeScript parser.
- [x] Resolve relative imports (./  ../) to project file paths.
- [x] Resolve index.ts barrel imports.
- [x] Unit tests verify parsing and resolution.
