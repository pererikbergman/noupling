---
description: File discovery and Tree-sitter AST parsing for dependency extraction
type: Spec
---

# Epic: Scanner Slice

### 1. Objective & Value Proposition

* **Goal:** Implement parallel file discovery and Tree-sitter based import parsing to build the dependency graph across 11 languages.
* **User Value:** Core capability: scans a project directory and extracts all import relationships regardless of language.

### 2. User Stories

* As a user, I want to scan a project directory so that all source files are discovered recursively.
* As a user, I want source files parsed for import declarations so that dependencies are extracted.
* As a developer, I want file scanning to run in parallel (Rayon) so that large projects are handled efficiently.
* As a developer, I want a pluggable grammar system so that new languages can be added easily.
* As a user, I want unsupported file types to be skipped so that the scan doesn't crash.

### 3. Functional Requirements

* Recursively discover source files (.rs, .kt, .kts, .ts, .tsx, .swift, .cs, .go, .hs, .java, .js, .jsx, .py, .zig) with depth tracking.
* Use Rayon for parallel file reading and AST parsing.
* Implement Tree-sitter parsers for all 11 languages.
* Map parsed imports to project file paths via language-specific resolvers.
* Skip files with unsupported extensions.

### 4. Supported Languages

| Language | Extensions | Import Pattern |
| :--- | :--- | :--- |
| C# | `.cs` | `using` directives |
| Go | `.go` | `import` declarations |
| Haskell | `.hs` | `import` declarations |
| Java | `.java` | `import` declarations |
| JavaScript | `.js`, `.jsx` | ES `import` statements |
| Kotlin | `.kt`, `.kts` | `import` declarations |
| Python | `.py` | `import` and `from...import` statements |
| Rust | `.rs` | `use` declarations |
| Swift | `.swift` | `import` declarations |
| TypeScript | `.ts`, `.tsx` | ES `import` statements |
| Zig | `.zig` | `@import()` builtin calls |

### 5. Acceptance Criteria

- [x] Source files discovered recursively with correct depth.
- [x] All 11 language parsers extract imports with line numbers.
- [x] Import resolvers map imports to project file paths per language.
- [x] Parallel scanning works via Rayon thread pool.
- [x] Unsupported file types are skipped.
- [x] Integration test passes against mock projects in `tests/fixtures/` and `examples/`.
