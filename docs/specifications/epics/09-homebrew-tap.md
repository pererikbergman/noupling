---
description: Create Homebrew Tap infrastructure for installing noupling via brew install
type: Spec
---

# Task: Create Homebrew Tap Infrastructure for Rust Project

I want to set up a dedicated Homebrew Tap for this Rust project so users can install it via `brew install`.

## Context
- **Project Name:** noupling
- **Binary Name:** noupling
- **GitHub Repo:** https://github.com/pererikbergman/noupling

## Requirements
1. **Formula Generation:** Create a Ruby formula file named `noupling.rb`.
   - Use `depends_on "rust" => :build`.
   - Use `system "cargo", "install", *std_cargo_args` in the install block.
   - Include a `test` block that runs `#{bin}/noupling --version`.
2. **TAP Repository Scaffolding:** Create a local directory structure representing a Homebrew Tap (e.g., `homebrew-tools/Formula/`).
3. **Automated Checksum Logic:** Write a small shell script or specialized command that:
   - Takes a version tag as an argument.
   - Calculates the SHA256 of the GitHub source tarball.
   - Updates the `url` and `sha256` fields in the `.rb` formula automatically.
4. **GitHub Actions:** Create a `.github/workflows/brew-update.yml` that can eventually be used to update the formula whenever a new release is tagged.

## Constraints
- Do NOT use LaTeX for any formatting.
- Ensure the formula adheres to Homebrew's "build from source" standards for Rust.
- The project uses a flat structure (not a workspace), so `std_cargo_args` works directly.

## Acceptance Criteria
- [ ] `noupling.rb` formula file created and valid.
- [ ] TAP repository structure scaffolded.
- [ ] Checksum update script works with version tags.
- [ ] GitHub Actions workflow created for automated formula updates.
- [ ] `brew install pererikbergman/tools/noupling` works from source.
