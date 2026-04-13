---
description: MoSCoW-prioritized phased roadmap for the noupling CLI
type: Spec
---

# Project Roadmap: noupling CLI

### 1. Phased Execution View

*   **Phase 1: Foundation & MVP (Must-Haves):**
    - [x] 01-01 - Restructure workspace into vertical slice architecture (Epic: Project Foundation)
    - [x] 01-02 - Define core domain types: Node, Dependency, Snapshot (Epic: Project Foundation)
    - [x] 01-03 - Add all workspace dependencies (Epic: Project Foundation)
    - [x] 01-04 - Implement clap CLI skeleton with scan/audit/report stubs (Epic: Project Foundation)
    - [x] 02-01 - Auto-initialize SQLite database with schema (Epic: Storage Slice)
    - [x] 02-02 - Implement SnapshotRepository (Epic: Storage Slice)
    - [x] 02-03 - Implement NodeRepository (Epic: Storage Slice)
    - [x] 02-04 - Implement DependencyRepository (Epic: Storage Slice)
    - [x] 03-01 - Recursive file discovery with tree building (Epic: Scanner Slice)
    - [x] 03-02 - Tree-sitter Rust grammar: parse `use` declarations (Epic: Scanner Slice)
    - [x] 03-03 - Map Rust imports to project file paths (Epic: Scanner Slice)
    - [x] 03-04 - Parallel scanning with Rayon (Epic: Scanner Slice)
    - [x] 04-01 - Bottom-up D_acc aggregation (Epic: Analyzer Slice)
    - [x] 04-02 - Top-down BFS sibling coupling detection (Epic: Analyzer Slice)
    - [x] 04-03 - Severity calculation: S = 1/(depth+1) (Epic: Analyzer Slice)
    - [x] 04-04 - Health score computation (Epic: Analyzer Slice)
    - [x] 05-01 - JSON reporter with critical_violations and score (Epic: Reporter Slice)
    - [x] 06-01 - Wire scan command: scanner -> storage pipeline (Epic: CLI Integration)
    - [x] 06-02 - Wire audit command: storage -> analyzer -> display (Epic: CLI Integration)
    - [x] 06-03 - Wire report command: audit -> JSON output (Epic: CLI Integration)

*   **Phase 2: Core Expansion (Should-Haves):**
    - [x] 04-05 - Circular dependency detection (Structural Loop, Severity 1.0) (Epic: Analyzer Slice)
    - [x] 05-02 - Markdown reporter with tables and sections (Epic: Reporter Slice)
    - [x] 06-04 - Wire report --format md command (Epic: CLI Integration)
    - [x] 06-05 - Audit --snapshot <ID> for historical snapshots (Epic: CLI Integration)
    - [x] 03-05 - Graceful skip for unsupported file types with warning (Epic: Scanner Slice)

*   **Phase 3: Scaling & Polish (Could-Haves):**
    - [ ] 03-06 - Kotlin grammar parser (Epic: Scanner Slice)
    - [ ] 03-07 - TypeScript grammar parser (Epic: Scanner Slice)
    - [ ] 03-08 - Swift grammar parser (Epic: Scanner Slice)
    - [ ] 03-09 - C# grammar parser (Epic: Scanner Slice)
    - [ ] 06-06 - End-to-end integration tests against fixtures (Epic: CLI Integration)

### 2. MoSCoW Prioritization Checklist

| Done | Priority | Story ID | Title | Epic Reference | Business Value / Impact |
| :--- | :--- | :--- | :--- | :--- | :--- |
| - [x] | Must | 01-01 | Restructure workspace | Project Foundation | Prerequisite for all development |
| - [x] | Must | 01-02 | Core domain types | Project Foundation | Common vocabulary for all slices |
| - [x] | Must | 01-03 | Workspace dependencies | Project Foundation | Enables compilation of all slices |
| - [x] | Must | 01-04 | CLI skeleton | Project Foundation | Entry point for user interaction |
| - [x] | Must | 02-01 | SQLite auto-init | Storage Slice | Persistence prerequisite |
| - [x] | Must | 02-02 | SnapshotRepository | Storage Slice | Snapshot lifecycle management |
| - [x] | Must | 02-03 | NodeRepository | Storage Slice | Tree structure persistence |
| - [x] | Must | 02-04 | DependencyRepository | Storage Slice | Import relationship storage |
| - [x] | Must | 03-01 | File discovery | Scanner Slice | Core scanning capability |
| - [x] | Must | 03-02 | Rust Tree-sitter parser | Scanner Slice | First language: Rust (user priority) |
| - [x] | Must | 03-03 | Rust import resolution | Scanner Slice | Maps imports to project files |
| - [x] | Must | 03-04 | Parallel scanning (Rayon) | Scanner Slice | Performance requirement |
| - [x] | Must | 04-01 | D_acc aggregation | Analyzer Slice | Core analysis algorithm |
| - [x] | Must | 04-02 | BFS coupling detection | Analyzer Slice | Core violation detection |
| - [x] | Must | 04-03 | Severity calculation | Analyzer Slice | Quantifies violation impact |
| - [x] | Must | 04-04 | Health score | Analyzer Slice | Summary metric for users |
| - [x] | Must | 05-01 | JSON reporter | Reporter Slice | CI-consumable output |
| - [x] | Must | 06-01 | Wire scan command | CLI Integration | End-to-end scan flow |
| - [x] | Must | 06-02 | Wire audit command | CLI Integration | End-to-end audit flow |
| - [x] | Must | 06-03 | Wire report (JSON) | CLI Integration | End-to-end report flow |
| - [x] | Should | 04-05 | Circular dependency detection | Analyzer Slice | Catches structural loops |
| - [x] | Should | 05-02 | Markdown reporter | Reporter Slice | Human-readable output |
| - [x] | Should | 06-04 | Wire report (Markdown) | CLI Integration | Format choice for users |
| - [x] | Should | 06-05 | Historical snapshot audit | CLI Integration | Trend analysis capability |
| - [x] | Should | 03-05 | Unsupported file skip | Scanner Slice | Robustness for mixed repos |
| - [ ] | Could | 03-06 | Kotlin parser | Scanner Slice | Multi-language expansion |
| - [ ] | Could | 03-07 | TypeScript parser | Scanner Slice | Multi-language expansion |
| - [ ] | Could | 03-08 | Swift parser | Scanner Slice | Multi-language expansion |
| - [ ] | Could | 03-09 | C# parser | Scanner Slice | Multi-language expansion |
| - [ ] | Could | 06-06 | E2E integration tests | CLI Integration | Confidence in full pipeline |

### 3. Business Value Analysis

**Must-Haves (MVP):**
* Project Foundation (01-01 to 01-04): Without structure and dependencies, nothing compiles. Zero value without this.
* Storage Slice (02-01 to 02-04): Persistence enables trend analysis, the key differentiator over one-shot linters.
* Scanner with Rust (03-01 to 03-04): Rust is the first target language per user priority. Scanning is the data ingestion layer.
* Analyzer (04-01 to 04-04): The core algorithm. Without analysis, the tool produces no actionable insight.
* JSON Reporter + CLI Wiring (05-01, 06-01 to 06-03): Minimum viable output for CI integration.

**Should-Haves:**
* Circular dependency detection adds a high-impact failure mode catch.
* Markdown reporter improves human review experience.
* Historical snapshot audit enables the trend analysis promise.

**Could-Haves:**
* Additional language parsers expand addressable market but are not needed for a working Rust-focused MVP.
