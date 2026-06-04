# Noupling Explorer — Visual Design Brief

**Status:** Draft 1
**Companion to:** [`noupling-explorer-prd.md`](./noupling-explorer-prd.md) (product requirements / behaviors)
**Source images:** `image_d884a2.jpg` (dark canvas), `image_d8665a.jpg` (light canvas), `image_404021.jpg` (dark canvas detail)

This document captures the visual and structural design intent for the Explorer report. The PRD answers *what to build* and *how it behaves*; this brief answers *how it should look*.

---

Act as an expert frontend engineer. Recreate the dashboard layout, component hierarchy, and interactive canvas based on the structures in image_d884a2.jpg, image_d8665a.jpg, and image_404021.jpg.

Your implementation must support both light and dark themes using a clean token system. Focus strictly on structure, spacing, and semantic theme switching using generic placeholder text.

### 1. Color Palette & Design Tokens

| Token Role | Dark Theme (image_d884a2.jpg / image_404021.jpg) | Light Theme (image_d8665a.jpg) |
| :--- | :--- | :--- |
| **Main Canvas Background** | Deep off-black (approx. #0B0B0C) | Soft off-white / light gray (approx. #F5F5F7) |
| **Primary Card Body** | Dark gray (approx. #18181C) | Pure white (#FFFFFF) |
| **Card Header Accent** | Uniform card color | Subtle light gray container cap (approx. #F0F0F2) |
| **Borders / Dividers** | Muted dark gray hairline outline | Clean, crisp light gray rule (approx. #E5E5EA) |
| **Primary Text** | Pure white (#FFFFFF) | Dark charcoal / black (#1C1C1E) |
| **Secondary / Label Text** | Muted medium gray (#8E8E93) | Medium slate gray (#636366) |
| **Active Selection Pills** | Light gray background / white text | High-contrast black background / white text |
| **Primary Action Button** | Pure white background / dark text | Solid black background / white text |
| **Success State Badge** | Vibrant green capsule | Vibrant green capsule |
| **Diagram Canvas Line/Edge** | Semi-translucent muted gray with arrowheads | Medium-gray stroke with arrowheads |
| **Node Left Accent Border** | Dynamic categorical colors (e.g., Purple, Teal, Orange, Green) | Dynamic categorical colors (matching theme vibrancy) |

### 2. Layout & Spacing Tokens
* **Border Radius:** Consistent modern rounding (approx. 8px for small inputs, 12px for nodes/nested elements, 16px for main framework cards).
* **Layout Structure:** A centered vertical stack featuring uniform padding and clean vertical gap spacing.

### 3. Component Hierarchy

#### A. Global Navigation Header
* A horizontal bar featuring a minimalist vector logo icon, a compact version status badge with a green indicator node, and a responsive utility menu toggle.

#### B. Segmented Control (Pill Tabs)
* A multi-option horizontal switcher layout. The active tab utilizes the primary high-contrast token according to the active theme, while inactive choices remain visually nested into the background.

#### C. Card Type 1: Input Action Form
* **Header Container:** All-caps section title anchored by an action icon.
* **Content Body:** A responsive row configuration containing a primary key text input field, a secondary optional input modifier, and a right-aligned validation action button.

#### D. Card Type 2: Compound Query Bar
* **Header Container:** All-caps section title paired with a search icon.
* **Content Body:**
    * Row 1: A prominent wide-fill text input field, a contextual sort dropdown menu selector, and a high-contrast primary call-to-action trigger button.
    * Row 2: A low-emphasis secondary button for toggle variables followed by a muted numerical results counter string.

#### E. Card Type 3: Node-Based Diagram Workspace (Graph Interface)
* **Header Container:** Section icon, all-caps workspace title, and layout-specific control toggles or view filters on the right edge.
* **Interactive Canvas:** A dedicated canvas field rendering a directed graph workflow.
    * **Directed Edges:** Nodes are interconnected using thin, sleek path lines with crisp arrowheads at target destinations indicating direction or workflow flow.
    * **Edge Labels:** Micro-copy text anchors centrally directly along or over the vector path lines to label relationships.
    * **Floating Panes (Nodes):** Compact, floating rounded cards arranged in structured tiers.
        * **Structural Accent:** A thick vertical indicator stripe spans the entire length of the far-left border of each node card. The color is dynamic based on node category.
        * **Internal Top Margin:** Displays low-contrast system tracking terms or badges aligned to the far edges.
        * **Internal Typography:** Features high-contrast primary text header titles with 2 to 3 lines of smaller, muted metadata sentences stacked directly below.

#### F. Card Type 4: Operational Status Queue
* **Header Container:** All-caps tracking title with a list element icon.
* **Content Row:** Displays an active item locator string, a distinct pill badge indicating a completed state, and a right-aligned utility removal icon.
