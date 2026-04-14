---
description: Professional documentation overhaul for open-source release
type: Spec
---

# Task: Professional Documentation Overhaul

I want to prepare this Rust project for open-source release. Please audit the current codebase and generate/update the following documentation files.

## Context
- **Project Goal:** noupling is a high-performance CLI tool that audits software architecture by detecting coupling violations and circular dependencies across 11 programming languages using Tree-sitter.
- **Target Audience:** Software Architects, Mobile/Backend Developers, DevOps Engineers, and teams adopting modular architecture.
- **Architecture Style:** Flat module structure with domain-separated modules (scanner, storage, analyzer, reporter).

## Requirements
1. **README.md:** Create a high-quality README including:
   - A clear project "Hook" and Feature list.
   - Quick Start code block (how to install and run a basic scan).
   - Usage instructions for the CLI.
   - Installation guide (including cargo install, Homebrew tap, and prebuilt binaries).
   - Configuration guide (settings.json).
   - All report format examples.
   - CI/CD integration guide (diff mode, SonarCloud).
2. **Crate Documentation (`main.rs`):** Audit the public API and add/improve Triple-Slash (`///`) doc comments.
   - Include a `# Examples` section in the main module documentation that is compatible with `cargo test`.
3. **CONTRIBUTING.md:** Define the contribution workflow:
   - Requirements for passing CI (clippy, fmt, tests).
   - Branching strategy (if applicable).
   - How to add a new language parser (step-by-step).
4. **Architecture Overview:** Create `docs/architecture.md` explaining how the project is structured, specifically highlighting the module layout, data flow (scan -> store -> analyze -> report), and how to navigate the codebase.

## Constraints
- **Format:** Use Markdown only. DO NOT use LaTeX.
- **Style:** Professional, concise, and helpful. Avoid marketing jargon.
- **Consistency:** Ensure that any examples provided in the README actually match the current CLI interface and function signatures in the source code.

## Acceptance Criteria
- [ ] README.md is comprehensive with hook, features, install, usage, config, CI integration.
- [ ] All public types and functions have `///` doc comments.
- [ ] `cargo doc` builds without warnings.
- [ ] CONTRIBUTING.md includes step-by-step for adding a new language.
- [ ] `docs/architecture.md` explains the full data flow and module responsibilities.
- [ ] All code examples in documentation actually work.

Please start by analyzing the `src/` directory and `Cargo.toml` to extract the core functionality, then propose a Table of Contents for the README.
