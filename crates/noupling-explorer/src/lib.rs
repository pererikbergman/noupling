//! noupling-explorer — the `--format explorer` reporter.
//!
//! Renders a self-contained HTML file the user opens in a browser to
//! navigate, understand, and reason about a noupling-scanned codebase.
//! See `docs/noupling-explorer-prd.md` for the product spec and
//! `docs/noupling-explorer-design.md` for the visual brief.
//!
//! Dependency rule (enforced via `.noupling/settings.json`):
//! `noupling-explorer` depends only on `noupling-core`. It must not
//! depend on `noupling` (the cli crate) or any reporter sibling.

use anyhow::Result;
use noupling_core::analyzer::AuditResult;
use noupling_core::core::{Dependency, Module, Snapshot};
use noupling_core::settings::Settings;

mod data_contract;
mod render;

/// Options controlling how the Explorer report is generated.
///
/// Plumbed in from the cli (`noupling report --format explorer …`) so the
/// renderer is decoupled from `clap` parsing.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Editor URL scheme for click-to-source links.
    /// e.g. `Some("vscode")` → produces `vscode://file/…` URLs in the report.
    pub editor: Option<String>,
    /// Override for the codebase title shown in the Explorer header.
    pub title: Option<String>,
    /// Include the `history[]` snapshot list in the Data Contract.
    /// Set to `false` via `--no-history` to shrink the output file.
    pub include_history: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            editor: None,
            title: None,
            include_history: true,
        }
    }
}

/// Render the Explorer HTML for a noupling-scanned codebase.
///
/// Returns the full self-contained HTML as a `String`. The caller is
/// responsible for writing it to disk (typically `.noupling/explorer.html`).
pub fn render(
    modules: &[Module],
    dependencies: &[Dependency],
    audit_result: &AuditResult,
    settings: &Settings,
    snapshot: &Snapshot,
    options: &RenderOptions,
) -> Result<String> {
    let contract = data_contract::build(
        modules,
        dependencies,
        audit_result,
        settings,
        snapshot,
        options,
    );
    let json = serde_json::to_string(&contract)?;
    Ok(render::embed_data(&json))
}
