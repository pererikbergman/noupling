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

mod auto_layers;
mod clusters;
mod data_contract;
mod render;

pub use auto_layers::detect_layers;
pub use clusters::ClusterEntry as ComputedClusterEntry;

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
    /// True when the cli is rendering with auto-detected layers
    /// rather than the project's hand-written settings.json. The
    /// template surfaces a banner so the user knows.
    pub layers_auto_detected: bool,
    /// Prior snapshots with their recorded health scores, used by the
    /// history scrubber. The cli loads these from
    /// `SnapshotRepository::get_all_with_scores()` and passes them
    /// through so noupling-explorer doesn't depend on the storage
    /// layer directly.
    pub history: Vec<HistoryEntry>,
    /// LLM-generated module enrichment loaded from
    /// `.noupling/enrichment/modules.json` (PR #280). Keyed by module
    /// path; merged into each node's `metrics.llm` block at render
    /// time. Skipped (left as `None` on the node) for any module
    /// whose enrichment is missing or stale.
    pub module_enrichment: Vec<ModuleEnrichmentEntry>,
}

/// One per-container enrichment record written by a skill into
/// `.noupling/enrichment/modules.json`. Schema versioned so the
/// renderer can warn-and-ignore newer-version entries.
#[derive(Debug, Clone)]
pub struct ModuleEnrichmentEntry {
    /// Module path the entry annotates — must match `NodeEntry.id`.
    pub module_path: String,
    /// One-line natural-language purpose ("Payment processing", etc.).
    pub summary: Option<String>,
    /// Longer responsibility description (multi-sentence).
    pub responsibility: Option<String>,
    /// Tags / classifications the skill chose (e.g. "domain", "test").
    pub tags: Vec<String>,
    /// ISO-8601 timestamp when the skill generated this entry. Used
    /// by the Explorer to surface staleness.
    pub generated_at: Option<String>,
    /// LLM identifier (model name) — for provenance.
    pub model: Option<String>,
}

/// One historical snapshot the Explorer's scrubber can navigate to.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub snapshot_id: String,
    pub taken_at: String,
    pub health_score: f64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            editor: None,
            title: None,
            include_history: true,
            layers_auto_detected: false,
            history: Vec::new(),
            module_enrichment: Vec::new(),
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
