---
description: Configure GitHub Actions CI/CD and version management pipeline
type: Spec
---

# Task: Configure GitHub Actions CI/CD and Version Management

I need to establish a robust automation pipeline for this Rust project. The goal is to ensure code quality on every PR and automate the release/versioning process.

## Context
- **Tooling:** GitHub Actions.
- **Project Type:** Rust (Binary).
- **Versioning Strategy:** Semantic Versioning (SemVer).
- **Current CI:** Basic ci.yml and release.yml already exist but may need refinement.

## Requirements
1. **CI Pipeline (`.github/workflows/ci.yml`):**
   - Create a workflow that triggers on push to `main` and all Pull Requests.
   - Include jobs for:
     - **Check:** `cargo check` to verify compilation.
     - **Tests:** `cargo test` to ensure logic is sound.
     - **Linting:** `cargo clippy -- -D warnings` to enforce clean code.
     - **Formatting:** `cargo fmt --check` to ensure style consistency.
   - **Optimization:** Implement `swatinem/rust-cache` to speed up build times.

2. **Release & Versioning Pipeline (`.github/workflows/release.yml`):**
   - Create a workflow that triggers when a tag (e.g., `v*`) is pushed.
   - It should automatically build optimized binaries for target platforms (macOS aarch64, macOS x86_64, Linux x86_64, Windows x86_64).
   - Use `softprops/action-gh-release` to create a GitHub Release and upload the binaries.

3. **Automatic Version Bumping:**
   - Propose a method (e.g., using `cargo-release` or a custom script) to bump the version in `Cargo.toml`, commit the change, and push a git tag in one command.

## Constraints
- **Format:** Pure text/markdown. No LaTeX.
- **Safety:** Ensure the CI fails if there are any Clippy warnings.
- **Architecture:** Keep the YAML modular and easy to read.

## Acceptance Criteria
- [ ] CI runs on every push to main and every PR.
- [ ] CI includes check, test, clippy (-D warnings), and fmt --check.
- [ ] CI uses rust-cache for faster builds.
- [ ] CI runs on Linux, macOS, and Windows.
- [ ] Release workflow triggers on version tags (v*).
- [ ] Release builds cross-platform binaries.
- [ ] Release creates GitHub Release with binaries attached.
- [ ] Version bump process is documented and works in one command.
- [ ] All YAML is clean, modular, and well-commented.

Please start by generating the `ci.yml` file and suggesting the best tool for managing version bumps within my local development workflow.
