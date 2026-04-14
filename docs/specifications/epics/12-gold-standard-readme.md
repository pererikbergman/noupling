---
description: Create a world-class open-source README as the project landing page
type: Spec
---

# Task: Create a "Gold Standard" Open Source README

I want to transform the README.md of this Rust project into a world-class open-source landing page. It should be visually professional, informative, and clear.

## Context
- **Project Name:** noupling
- **Description:** A high-performance CLI that audits software architecture by detecting coupling violations and circular dependencies across 11 languages.
- **Key Tech:** Rust, Tree-sitter, SQLite (rusqlite), Clap, Rayon, FxHash, GlobSet
- **Repository URL:** https://github.com/pererikbergman/noupling

## Requirements
1. **Visual Header:**
   - Add high-quality dynamic badges for: GitHub Actions Build Status, crates.io version (if applicable), License type, and Rust Edition (2021).
   - Create a clean, bold title and a catchy 1-2 sentence sub-title.
2. **The "Why" Section:**
   - A concise "Features" section using bullet points.
   - A "Why this exists" section comparing it to existing alternatives (be objective and professional).
3. **Interactive Quick Start:**
   - An "Installation" section covering `cargo install`, prebuilt binaries, and the Homebrew tap (once Epic 09 is done).
   - A "Usage" section with clear, copy-pasteable CLI command examples.
   - A "Configuration" section showing settings.json.
4. **Visual Hierarchy:**
   - Use horizontal rules to separate sections.
   - Use a Table of Contents for easy navigation.
5. **Report Formats:**
   - Show all 6 report formats (json, xml, md, html, sonar) with brief descriptions.
   - Include a screenshot or example of the HTML report.
6. **CI/CD Integration:**
   - Show how to use diff mode in a GitHub Actions workflow.
   - Show how to integrate with SonarCloud.
7. **Community & Meta:**
   - Sections for "Contributing," "Security," and "License."
   - A "Contributors" section (can use the `contrib.rocks` placeholder or similar).

## Constraints
- **NO LaTeX:** Use standard Markdown formatting only.
- **Modern Style:** Use Unicode icons/emojis sparingly for visual cues.
- **Accuracy:** Cross-reference the "Quick Start" with the actual CLI interface to ensure the examples work.
- **Verify:** All commands shown in the README must work when copy-pasted.

## Acceptance Criteria
- [ ] README has badges (CI, license, version).
- [ ] README has a compelling "Why noupling?" section.
- [ ] README has Table of Contents.
- [ ] Installation covers all methods (cargo, binary, homebrew).
- [ ] Usage examples are accurate and copy-pasteable.
- [ ] Configuration section shows full settings.json with comments.
- [ ] All 6 report formats documented.
- [ ] CI integration guide (diff mode + SonarCloud).
- [ ] Contributing, Security, and License sections present.
- [ ] README looks professional and credible.

Please analyze the current project state and generate the full Markdown for a "Gold Standard" README.md.
