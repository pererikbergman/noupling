---
description: Prepare the repository for public open-source release to a high standard
type: Spec
---

# Epic: Open Source Readiness

### 1. Objective & Value Proposition

* **Goal:** Bring the repository to a professional open-source standard so it can be made public with confidence. The project should look credible, be easy to contribute to, and meet community expectations.
* **User Value:** Attracts contributors, builds trust, and ensures the project is maintainable long-term.

### 2. Tasks

#### 2.1 License

- [ ] Add a LICENSE file (MIT, as declared in Cargo.toml).
- [ ] Verify all dependencies are compatible with MIT licensing.

#### 2.2 README Overhaul

- [ ] Add a project logo or banner.
- [ ] Add badges: CI status, crate version, license, supported languages.
- [ ] Add a "Why noupling?" section explaining the problem it solves.
- [ ] Add a quick demo/screenshot of the HTML report.
- [ ] Add a "How it works" section with a brief explanation of D_acc, BFS, and severity.
- [ ] Add a "Contributing" link.
- [ ] Add a "Changelog" link.

#### 2.3 Contributing Guide

- [ ] Create CONTRIBUTING.md with: how to build, test, add a new language parser, submit PRs.
- [ ] Define coding standards (cargo fmt, cargo clippy, TDD).
- [ ] Describe the architecture briefly so contributors know where to look.

#### 2.4 Code of Conduct

- [ ] Add CODE_OF_CONDUCT.md (Contributor Covenant or similar).

#### 2.5 Changelog

- [ ] Create CHANGELOG.md with notable changes grouped by version.
- [ ] Consider using conventional commits for future automation.

#### 2.6 CI/CD (GitHub Actions)

- [ ] Add workflow: build and test on push/PR (Linux, macOS, Windows).
- [ ] Add workflow: cargo clippy and cargo fmt check.
- [ ] Add workflow: release binaries on version tag (cross-platform).
- [ ] Add workflow: publish to crates.io on release.

#### 2.7 Code Quality

- [ ] Fix all cargo clippy warnings.
- [ ] Remove all unused imports and dead code warnings.
- [ ] Add rustdoc comments to all public types and functions.
- [ ] Ensure cargo doc builds without warnings.
- [ ] Review and clean up any TODO/FIXME comments.

#### 2.8 Testing

- [ ] Ensure all tests pass on clean checkout.
- [ ] Add integration tests that run noupling against the example projects.
- [ ] Verify test coverage is adequate for all parsers and the analyzer.

#### 2.9 Packaging

- [ ] Verify `cargo publish --dry-run` succeeds.
- [ ] Add repository, homepage, and description to Cargo.toml.
- [ ] Add categories and keywords to Cargo.toml for crates.io discoverability.
- [ ] Verify the binary name, version, and metadata are correct.

#### 2.10 Security

- [ ] Run `cargo audit` to check for known vulnerabilities in dependencies.
- [ ] Ensure no secrets, credentials, or personal paths are in the codebase or git history.
- [ ] Review .gitignore is comprehensive.

#### 2.11 Documentation

- [ ] Ensure `noupling --help` is complete and accurate (done).
- [ ] Add man page or extended docs if needed.
- [ ] Verify README examples actually work on a clean install.

#### 2.12 Repository Hygiene

- [ ] Remove .agent/ directory (internal workflow, not relevant to open source).
- [ ] Remove tasks/ directory (internal task tracking).
- [ ] Remove GEMINI.md (internal agent config).
- [ ] Remove AGENTS.md (internal agent config).
- [ ] Remove docs/specifications/ (internal planning docs) or move to a wiki.
- [ ] Clean up Thumbs.db and .DS_Store from the repo.
- [ ] Add GitHub issue templates (bug report, feature request).
- [ ] Add GitHub PR template.

### 3. Acceptance Criteria

- [ ] A new user can clone, build, test, and install in under 5 minutes.
- [ ] The README clearly explains what the tool does, how to install it, and how to use it.
- [ ] CI passes on all platforms (Linux, macOS, Windows).
- [ ] `cargo clippy` produces zero warnings.
- [ ] `cargo doc` builds without warnings.
- [ ] `cargo publish --dry-run` succeeds.
- [ ] `cargo audit` shows no known vulnerabilities.
- [ ] The repository looks professional and credible to potential contributors.
