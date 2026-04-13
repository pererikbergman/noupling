---
description: Implement clap CLI skeleton with scan, audit, report subcommands
type: Task
story: 01-04
---

# Task: Implement CLI Skeleton

### Context

The noupling binary needs a clap-based CLI with three subcommands: scan, audit, report.

### Objective

Create the CLI argument structure so that `noupling --help` and all subcommand help texts work.

### Acceptance Criteria

- [x] `noupling scan <PATH>` accepts a path argument.
- [x] `noupling audit` works with optional `--snapshot <ID>`.
- [x] `noupling report --format <json|md>` accepts format flag.
- [x] `--help` works for root and all subcommands.
- [x] Subcommands print "Not yet implemented" placeholder messages.

### TDD Strategy

1. **Red:** Write tests that parse known arg vectors and assert correct command matching.
2. **Green:** Implement clap derive structs.
3. **Refactor:** Clean up help text descriptions.

### Implementation Steps

- [x] 1. Write failing tests for CLI arg parsing.
- [x] 2. Define Cli, Commands enum, ScanArgs, AuditArgs, ReportArgs with clap derive.
- [x] 3. Wire main.rs to match on commands and print placeholders.
- [x] 4. Verify tests pass and `cargo run -- --help` works.
