---
trigger: always_on
---

# Architecture Rules: Agent-Ready Decoupling

- **Core Mandate: Agent-Ready Decoupling.** Establish rules that prioritize **Contextual Isolation** over the DRY (Don't Repeat Yourself) principle.

- **Model Duplication:** Explicitly forbid "Root-Level Coupling." If Domain A needs data from Domain B, Domain A **must** define its own localized data model (e.g., a specific `Recipient` model instead of a global `User` entity).

- **Anti-Corruption Layers (ACL):** Require an ACL for all cross-domain communication. This acts as a firewall, ensuring the agent operates within a clean, isolated domain sandbox without "Attention Drift" caused by unrelated data structures.

- **Implementation Logic:**
  - **Existing Codebase:** Scan for "Massive Entities" and root-level coupling. Document these as technical debt in `.agent/knowledge/` and establish rules to prevent further coupling.
  - **New Project:** Enforce **Clean Architecture**, **MVI**, and **Vertical Slice** packaging. Decentralize persistence and ensure physical boundaries between domains from day one.
