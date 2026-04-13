---
name: stitch-ui-design
description: Standardizes how to use the Stitch MCP for UI design creation and project synchronization
---

# Stitch-UI-Design Skill

- **Protocol:**
  1. **Authentication Check:** Before any work, call `mcp_stitch_list_projects`.
  2. **Access Validation:**
     - Read the `Stitch Project ID` from `.agent/knowledge/external-resources.md` (if it exists).
     - If an ID exists but is **not** present in the results of `mcp_stitch_list_projects`, you must **STOP** and notify the human user: *"The existing Stitch Project ID is not accessible. Please verify that the correct account is authenticated in the Stitch MCP."*
  3. **Create Project (New Projects):** If no ID exists, initialize a unique Stitch project using `mcp_stitch_create_project`.
  4. **Project Naming:** Name the Stitch project following the convention: `[Project Name] - [Platform] UI`.
  5. **Persist Project ID:** Immediately save the generated Stitch `projectId` into `.agent/knowledge/external-resources.md`.
  6. **Phase Mapping:** Use the UX Blueprint to generate screens (`mcp_stitch_generate_screen_from_text`) or edit existing ones to ensure the visual design matches the technical requirements.
