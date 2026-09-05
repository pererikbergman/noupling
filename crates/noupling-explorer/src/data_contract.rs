//! Data Contract serialization for the Explorer.
//!
//! Builds the JSON shape documented in `docs/noupling-explorer-prd.md` §6,
//! which the embedded template hydrates at startup. Fields are additive
//! across versions; the template treats unknown keys as optional.

use globset::Glob;
use noupling_core::analyzer::{AuditResult, Issue, IssueCard, IssueDetail, IssueKind};
use noupling_core::core::{Dependency, Module, ModuleType, Snapshot};
use noupling_core::settings::{DependencyRule, Layer, Settings};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::RenderOptions;

#[derive(Debug, Serialize)]
pub(crate) struct DataContract {
    pub format_version: u32,
    pub noupling_version: String,
    pub generated_at: String,
    pub report_options: ReportOptions,
    /// True when the audit inferred `layers` from path-segment
    /// patterns because the project's settings.json had no
    /// configured layers (ADR 0001). The template surfaces a banner so
    /// the user knows the layers aren't authoritative.
    pub layers_auto_detected: bool,
    pub codebase: Codebase,
    pub health_score: f64,
    pub score_breakdown: ScoreBreakdown,
    pub summary_counts: SummaryCounts,
    pub layers: Vec<LayerEntry>,
    pub dependency_rules: Vec<DependencyRuleEntry>,
    pub effective_rules: Vec<EffectiveRuleEntry>,
    pub nodes: Vec<NodeEntry>,
    pub edges: Vec<EdgeEntry>,
    /// Ring geometry for the canvas (min-cut overlay, cycle accents).
    /// The Cycle *Issue* — band, reason, recommendation — is in `issues`.
    pub cycles: Vec<CycleEntry>,
    /// Edge geometry for the canvas (which edges to accent). The
    /// Coupling / Rule / Layer Violation *Issues* are in `issues`.
    pub violations: Vec<ViolationEntry>,
    /// Every Issue, as the same Issue cards the JSON report emits
    /// (ADR 0002, #345), plus the participant node ids focus mode uses.
    pub issues: Vec<IssueEntry>,
    pub history: Vec<HistoryEntry>,
    /// Pre-computed module clusters for the Force view. Tightly
    /// coupled groups detected via label propagation; the Force view
    /// renders a faint boundary around each cluster's nodes so the
    /// user sees the structure the force layout would otherwise
    /// blur. Empty by default and never gates view rendering.
    pub clusters: Vec<ClusterEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ClusterEntry {
    pub id: String,
    pub members: Vec<String>,
}

/// Explains how `health_score` was derived: the points lost, split by
/// Issue kind, and the Issues that cost the most. Sums come from each
/// Issue's score impact (#342), so every row adds up to `points_lost`.
#[derive(Debug, Serialize)]
pub(crate) struct ScoreBreakdown {
    pub total_modules: usize,
    pub points_lost: f64,
    /// One row per kind that scores, canonical order, zero rows omitted.
    pub by_kind: Vec<KindPoints>,
    /// Top Issues by score impact. Capped at 5 — anything longer is noise
    /// in a dialog the user reads once.
    pub top_contributors: Vec<ScoreContributor>,
}

#[derive(Debug, Serialize)]
pub(crate) struct KindPoints {
    pub kind: &'static str,
    pub kind_name: &'static str,
    pub points: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ScoreContributor {
    pub kind: &'static str,
    pub kind_name: &'static str,
    /// The Issue's subject, rendered (`from -> to`, a path, or a ring).
    pub subject: String,
    /// The node to select when the row is clicked.
    pub focus_id: String,
    pub points: f64,
    pub fingerprint: String,
}

/// One Issue in the contract: the shared Issue card verbatim, plus the
/// node ids that participate in it (what focus mode expands and
/// highlights, #333).
#[derive(Debug, Serialize)]
pub(crate) struct IssueEntry {
    #[serde(flatten)]
    pub card: IssueCard,
    pub participants: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NodeEntry {
    pub id: String,
    pub kind: &'static str,
    pub parent: Option<String>,
    pub layer: Option<String>,
    pub metrics: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub(crate) struct EdgeEntry {
    pub from: String,
    pub to: String,
    pub weight: usize,
    pub violates_rule: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CycleEntry {
    pub id: String,
    pub size: usize,
    pub members: Vec<String>,
    pub minimum_cut: Vec<CutEdge>,
}

#[derive(Debug, Serialize)]
pub(crate) struct EdgeRef {
    pub from: String,
    pub to: String,
}

/// An edge in a cycle's minimum-cut set. Carries the cost of cutting
/// it (`weight` = imports at this hop) and the heaviest other hop in
/// the same cycle (`vs_weight`), so the UI can render
/// `break: A → B (2 vs 14)` and justify the recommendation. Issue #277.
#[derive(Debug, Serialize)]
pub(crate) struct CutEdge {
    pub from: String,
    pub to: String,
    pub weight: usize,
    pub vs_weight: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct ViolationEntry {
    pub rule: EdgeRef,
    pub edge: EdgeRef,
    pub severity: &'static str,
    pub introduced_in: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct HistoryEntry {
    pub snapshot_id: String,
    pub taken_at: String,
    pub health_score: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct DependencyRuleEntry {
    pub from: String,
    pub to: String,
    pub allow: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct EffectiveRuleEntry {
    pub from: String,
    pub to: String,
    pub allow: bool,
    pub message: String,
    pub source: &'static str,
    pub current_violation_count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct LayerEntry {
    pub name: String,
    pub pattern: String,
    pub allow_sibling: bool,
    pub index: usize,
    pub file_count: usize,
    pub afferent: usize,
    pub efferent: usize,
    pub instability: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SummaryCounts {
    pub violations: usize,
    pub cycles: usize,
    pub gravity_wells: usize,
    pub red_flags: usize,
    /// Every Issue, and how many the baseline accepts (#343).
    pub issues: usize,
    pub new_issues: usize,
    pub baselined_issues: usize,
    /// One row per kind, all nine, zero included, canonical order.
    pub by_kind: Vec<KindCount>,
}

#[derive(Debug, Serialize)]
pub(crate) struct KindCount {
    pub kind: &'static str,
    pub kind_name: &'static str,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReportOptions {
    /// Editor URL scheme (`vscode`/`jetbrains`/`sublime`/`cursor`) or a
    /// custom template like `myeditor://x/{path}:{line}`. `None` means
    /// the template falls back to the OS default for `file://` links.
    pub editor: Option<String>,
    /// Override for the codebase title shown in the header.
    pub title: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Codebase {
    pub path: String,
    pub module_count: usize,
    pub file_count: usize,
    pub edge_count: usize,
    pub language_distribution: Vec<LanguageEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LanguageEntry {
    pub language: String,
    pub file_count: usize,
}

pub(crate) fn build(
    modules: &[Module],
    dependencies: &[Dependency],
    audit_result: &AuditResult,
    settings: &Settings,
    snapshot: &Snapshot,
    options: &RenderOptions,
) -> DataContract {
    ContractBuilder::new(
        modules,
        dependencies,
        audit_result,
        settings,
        snapshot,
        options,
    )
    .build()
}

/// Holds the six inputs the Data Contract needs once, then exposes a
/// method per stage. Issue #303. The seam is "what fields the
/// contract is built from" — invariant across stages, kept in one
/// struct instead of plumbed through every helper signature.
///
/// Stages can be inspected in isolation in tests (`builder.score_breakdown()`)
/// without re-running the full `build()` orchestrator.
pub(crate) struct ContractBuilder<'a> {
    modules: &'a [Module],
    dependencies: &'a [Dependency],
    audit_result: &'a AuditResult,
    settings: &'a Settings,
    snapshot: &'a Snapshot,
    options: &'a RenderOptions,
}

impl<'a> ContractBuilder<'a> {
    pub(crate) fn new(
        modules: &'a [Module],
        dependencies: &'a [Dependency],
        audit_result: &'a AuditResult,
        settings: &'a Settings,
        snapshot: &'a Snapshot,
        options: &'a RenderOptions,
    ) -> Self {
        Self {
            modules,
            dependencies,
            audit_result,
            settings,
            snapshot,
            options,
        }
    }

    /// Run every stage and assemble the contract. The orchestration
    /// order is explicit here, instead of implicit in a struct literal.
    pub(crate) fn build(&self) -> DataContract {
        DataContract {
            format_version: 2,
            noupling_version: env!("CARGO_PKG_VERSION").to_string(),
            generated_at: self.snapshot.timestamp.clone(),
            report_options: self.report_options(),
            layers_auto_detected: self.audit_result.layers_auto_detected,
            codebase: self.codebase(),
            health_score: self.audit_result.score,
            score_breakdown: self.score_breakdown(),
            summary_counts: self.summary_counts(),
            layers: self.layers(),
            dependency_rules: self.dependency_rules(),
            effective_rules: self.effective_rules(),
            nodes: self.nodes(),
            edges: self.edges(),
            cycles: self.cycles(),
            violations: self.violations(),
            issues: self.issues(),
            history: self.history(),
            clusters: self.clusters(),
        }
    }

    pub(crate) fn report_options(&self) -> ReportOptions {
        ReportOptions {
            editor: self.options.editor.clone(),
            title: self.options.title.clone(),
        }
    }

    pub(crate) fn codebase(&self) -> Codebase {
        build_codebase(
            self.modules,
            self.dependencies,
            self.audit_result,
            self.snapshot,
        )
    }

    pub(crate) fn score_breakdown(&self) -> ScoreBreakdown {
        build_score_breakdown(self.audit_result)
    }

    pub(crate) fn summary_counts(&self) -> SummaryCounts {
        let issues = self.audit_result.issues();
        SummaryCounts {
            violations: self.audit_result.violations.len(),
            cycles: self
                .audit_result
                .violations
                .iter()
                .filter(|v| v.is_circular)
                .count(),
            gravity_wells: self.audit_result.gravity_wells.len(),
            red_flags: self.audit_result.red_flags.len(),
            issues: issues.len(),
            new_issues: issues.iter().filter(|i| !i.baselined).count(),
            baselined_issues: issues.iter().filter(|i| i.baselined).count(),
            by_kind: IssueKind::ALL
                .iter()
                .map(|k| KindCount {
                    kind: k.id(),
                    kind_name: k.name(),
                    count: issues.iter().filter(|i| i.kind() == *k).count(),
                })
                .collect(),
        }
    }

    /// The shared Issue cards (same code path as the JSON report), each
    /// with the participant node ids focus mode expands and highlights.
    pub(crate) fn issues(&self) -> Vec<IssueEntry> {
        self.audit_result
            .issues()
            .iter()
            .map(|issue| IssueEntry {
                card: issue.to_card(),
                participants: participants_of(issue, self.audit_result),
            })
            .collect()
    }

    pub(crate) fn layers(&self) -> Vec<LayerEntry> {
        build_layers(&self.audit_result.layers, self.modules, self.dependencies)
    }

    pub(crate) fn dependency_rules(&self) -> Vec<DependencyRuleEntry> {
        build_dependency_rules(&self.settings.dependency_rules)
    }

    pub(crate) fn effective_rules(&self) -> Vec<EffectiveRuleEntry> {
        build_effective_rules(
            &self.audit_result.layers,
            &self.settings.dependency_rules,
            self.audit_result,
        )
    }

    pub(crate) fn nodes(&self) -> Vec<NodeEntry> {
        merge_enrichment(
            build_nodes(
                &self.audit_result.layers,
                self.modules,
                self.dependencies,
                self.audit_result,
                &self.snapshot.root_path,
            ),
            &self.options.module_enrichment,
        )
    }

    pub(crate) fn edges(&self) -> Vec<EdgeEntry> {
        build_edges(self.modules, self.dependencies, self.audit_result)
    }

    pub(crate) fn cycles(&self) -> Vec<CycleEntry> {
        build_cycles(self.audit_result)
    }

    pub(crate) fn violations(&self) -> Vec<ViolationEntry> {
        build_violations(self.audit_result)
    }

    pub(crate) fn history(&self) -> Vec<HistoryEntry> {
        if !self.options.include_history {
            return Vec::new();
        }
        self.options
            .history
            .iter()
            .map(|h| HistoryEntry {
                snapshot_id: h.snapshot_id.clone(),
                taken_at: h.taken_at.clone(),
                health_score: h.health_score,
            })
            .collect()
    }

    pub(crate) fn clusters(&self) -> Vec<ClusterEntry> {
        crate::clusters::detect_clusters(self.modules, self.dependencies)
            .into_iter()
            .map(|c| ClusterEntry {
                id: c.id,
                members: c.members,
            })
            .collect()
    }
}

fn build_nodes(
    layers: &[Layer],
    modules: &[Module],
    dependencies: &[Dependency],
    audit_result: &AuditResult,
    codebase_root: &str,
) -> Vec<NodeEntry> {
    let compiled: Vec<Option<globset::GlobMatcher>> = layers
        .iter()
        .map(|l| Glob::new(&l.pattern).ok().map(|g| g.compile_matcher()))
        .collect();
    let layer_of = |path: &str| -> Option<String> {
        compiled.iter().enumerate().find_map(|(i, m)| {
            m.as_ref()
                .and_then(|mm| mm.is_match(path).then(|| layers[i].name.clone()))
        })
    };

    let mut nodes: Vec<NodeEntry> = Vec::new();

    // Per-file Ca/Ce: walk dependencies once and bucket by from/to module path.
    let id_to_path: HashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();
    let mut afferent_by_path: HashMap<String, usize> = HashMap::new();
    let mut efferent_by_path: HashMap<String, usize> = HashMap::new();
    for d in dependencies {
        let from = id_to_path.get(d.from_module_id.as_str());
        let to = id_to_path.get(d.to_module_id.as_str());
        if let (Some(f), Some(t)) = (from, to) {
            if f != t {
                *efferent_by_path.entry((*f).to_string()).or_insert(0) += 1;
                *afferent_by_path.entry((*t).to_string()).or_insert(0) += 1;
            }
        }
    }

    // Blast-radius BFS adjacency (file-path keyed).
    let mut downstream_adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut upstream_adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for d in dependencies {
        let from = id_to_path.get(d.from_module_id.as_str()).copied();
        let to = id_to_path.get(d.to_module_id.as_str()).copied();
        if let (Some(f), Some(t)) = (from, to) {
            if f != t {
                downstream_adj.entry(f).or_default().push(t);
                upstream_adj.entry(t).or_default().push(f);
            }
        }
    }
    let blast_radius_for = |start: &str, adj: &HashMap<&str, Vec<&str>>| -> usize {
        let mut visited: HashSet<&str> = HashSet::new();
        let mut q: VecDeque<&str> = VecDeque::new();
        q.push_back(start);
        visited.insert(start);
        while let Some(cur) = q.pop_front() {
            if let Some(neighbors) = adj.get(cur) {
                for n in neighbors {
                    if visited.insert(n) {
                        q.push_back(n);
                    }
                }
            }
        }
        // Exclude the start node from the count — blast radius is the
        // number of *other* modules reachable.
        visited.len().saturating_sub(1)
    };

    // Pre-build dir → contained file set so package metrics + layer
    // inheritance can be computed without re-walking the module list.
    let mut files_in_dir: HashMap<String, HashSet<String>> = HashMap::new();
    let files: Vec<&Module> = modules
        .iter()
        .filter(|m| matches!(m.module_type, ModuleType::File))
        .collect();
    for m in &files {
        let mut p = parent_dir(&m.path).map(str::to_string);
        while let Some(dir) = p.clone() {
            files_in_dir
                .entry(dir.clone())
                .or_default()
                .insert(m.path.clone());
            p = parent_dir(&dir).map(str::to_string);
        }
    }

    // File nodes
    for m in &files {
        let parent = parent_dir(&m.path).map(str::to_string);
        let ca = afferent_by_path.get(&m.path).copied().unwrap_or(0);
        let ce = efferent_by_path.get(&m.path).copied().unwrap_or(0);
        let instability = if ca + ce == 0 {
            serde_json::Value::Null
        } else {
            serde_json::Value::from((ce as f64 / (ca + ce) as f64 * 100.0).round() / 100.0)
        };
        let loc = count_lines(codebase_root, &m.path);
        let blast_up = blast_radius_for(m.path.as_str(), &upstream_adj);
        let blast_down = blast_radius_for(m.path.as_str(), &downstream_adj);
        nodes.push(NodeEntry {
            id: m.path.clone(),
            kind: "file",
            parent,
            layer: layer_of(&m.path),
            metrics: serde_json::json!({
                "afferent": ca,
                "efferent": ce,
                "instability": instability,
                "loc": loc,
                "blast_radius_upstream": blast_up,
                "blast_radius_downstream": blast_down,
            }),
        });
    }

    // Directory nodes: package (has direct files) or container (only subdirs).
    let mut dirs: BTreeMap<String, DirInfo> = BTreeMap::new();
    for m in &files {
        let mut p = parent_dir(&m.path).map(str::to_string);
        let mut first = true;
        while let Some(dir) = p {
            let entry = dirs.entry(dir.clone()).or_insert_with(|| DirInfo {
                has_files: false,
                file_count: 0,
            });
            if first {
                entry.has_files = true;
                entry.file_count += 1;
            }
            first = false;
            p = parent_dir(&dir).map(str::to_string);
        }
    }
    for (path, info) in &dirs {
        let kind = if info.has_files {
            "package"
        } else {
            "container"
        };
        let parent = parent_dir(path).map(str::to_string);

        // Cohesion from the audit (Packages only).
        let cohesion = if info.has_files {
            audit_result
                .cohesion
                .iter()
                .find(|c| c.dir == *path)
                .and_then(|c| c.cohesion)
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };

        // Package layer: prefer the configured layer pattern (`layer_of`),
        // but fall back to inheriting from any contained file when the
        // directory itself doesn't match a layer pattern. This is what
        // makes the LSM put packages on the right tier when settings.json
        // patterns target file-inside-dir but not the dir itself.
        let layer = layer_of(path).or_else(|| {
            files_in_dir
                .get(path)
                .and_then(|fs| fs.iter().find_map(|f| layer_of(f)))
        });

        // Ca/Ce per directory: count edges crossing the directory boundary.
        let inside = files_in_dir.get(path);
        let (ca, ce) = if let Some(inside) = inside {
            let mut ca = 0usize;
            let mut ce = 0usize;
            for d in dependencies {
                let f = id_to_path
                    .get(d.from_module_id.as_str())
                    .copied()
                    .unwrap_or("");
                let t = id_to_path
                    .get(d.to_module_id.as_str())
                    .copied()
                    .unwrap_or("");
                let f_in = inside.contains(f);
                let t_in = inside.contains(t);
                if f_in && !t_in {
                    ce += 1;
                }
                if !f_in && t_in {
                    ca += 1;
                }
            }
            (ca, ce)
        } else {
            (0, 0)
        };
        let instability = if ca + ce == 0 {
            serde_json::Value::Null
        } else {
            serde_json::Value::from((ce as f64 / (ca + ce) as f64 * 100.0).round() / 100.0)
        };

        nodes.push(NodeEntry {
            id: path.clone(),
            kind,
            parent,
            layer,
            metrics: serde_json::json!({
                "afferent": ca,
                "efferent": ce,
                "instability": instability,
                "abstractness": null,
                "distance_from_main_sequence": null,
                "cohesion": cohesion,
                "file_count": info.file_count,
                "loc": 0,
            }),
        });
    }

    nodes
}

/// Count newlines in a source file. Falls back to 0 when the file can't be
/// read (deleted between scan and emit, permission denied, etc.) so the
/// Data Contract stays serialisable regardless.
fn count_lines(codebase_root: &str, rel_path: &str) -> usize {
    let full = if Path::new(rel_path).is_absolute() {
        Path::new(rel_path).to_path_buf()
    } else {
        Path::new(codebase_root).join(rel_path)
    };
    std::fs::read_to_string(&full)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

struct DirInfo {
    has_files: bool,
    file_count: usize,
}

fn parent_dir(path: &str) -> Option<&str> {
    path.rfind('/').map(|i| &path[..i])
}

fn build_edges(modules: &[Module], deps: &[Dependency], _audit: &AuditResult) -> Vec<EdgeEntry> {
    let id_to_path: HashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();
    let mut weights: BTreeMap<(String, String), usize> = BTreeMap::new();
    for d in deps {
        let from = id_to_path.get(d.from_module_id.as_str());
        let to = id_to_path.get(d.to_module_id.as_str());
        if let (Some(f), Some(t)) = (from, to) {
            *weights.entry((f.to_string(), t.to_string())).or_insert(0) += 1;
        }
    }
    weights
        .into_iter()
        .map(|((from, to), weight)| EdgeEntry {
            from,
            to,
            weight,
            violates_rule: None,
        })
        .collect()
}

fn build_cycles(audit_result: &AuditResult) -> Vec<CycleEntry> {
    audit_result
        .violations
        .iter()
        .filter(|v| v.is_circular)
        .enumerate()
        .map(|(i, v)| {
            // `cycle_hop_counts[k]` is the import count crossing the k-th
            // hop. `break_cost` already names the minimum; `vs_weight` is
            // the heaviest *other* hop — what the user pays *not* to cut.
            // For 2-node cycles this is exactly "the other way"; for
            // longer cycles it's the strongest remaining segment, which
            // is the comparison that makes the cut feel justified.
            let vs_weight = v
                .cycle_hop_counts
                .iter()
                .copied()
                .filter(|&c| c != v.break_cost)
                .max()
                .unwrap_or(v.break_cost);
            let minimum_cut = v
                .weakest_link
                .as_ref()
                .and_then(|wl| parse_weakest_link(wl))
                .map(|(f, t)| {
                    vec![CutEdge {
                        from: f,
                        to: t,
                        weight: v.break_cost,
                        vs_weight,
                    }]
                })
                .unwrap_or_default();
            CycleEntry {
                id: format!("cycle-{}", i + 1),
                size: v.cycle_order,
                members: v.cycle_path.clone(),
                minimum_cut,
            }
        })
        .collect()
}

fn parse_weakest_link(s: &str) -> Option<(String, String)> {
    // Format: "dir_a -> dir_b (N imports)"
    let arrow_idx = s.find(" -> ")?;
    let from = s[..arrow_idx].trim().to_string();
    let rest = &s[arrow_idx + 4..];
    let paren_idx = rest.find(" (").unwrap_or(rest.len());
    let to = rest[..paren_idx].trim().to_string();
    Some((from, to))
}

fn build_violations(audit_result: &AuditResult) -> Vec<ViolationEntry> {
    audit_result
        .violations
        .iter()
        .map(|v| ViolationEntry {
            rule: EdgeRef {
                from: v.dir_a.clone(),
                to: v.dir_b.clone(),
            },
            edge: EdgeRef {
                from: v.from_module.clone(),
                to: v.to_module.clone(),
            },
            severity: severity_band(v.severity),
            introduced_in: None,
        })
        .collect()
}

/// Fold the optional LLM enrichment entries into each node's
/// `metrics.llm` block. Skipped entries leave the metrics object
/// unchanged. Issue #280 — the Composition view picks up the
/// `summary` field as the labeled card text when present.
fn merge_enrichment(
    mut nodes: Vec<NodeEntry>,
    enrichment: &[crate::ModuleEnrichmentEntry],
) -> Vec<NodeEntry> {
    if enrichment.is_empty() {
        return nodes;
    }
    let by_path: std::collections::HashMap<&str, &crate::ModuleEnrichmentEntry> = enrichment
        .iter()
        .map(|e| (e.module_path.as_str(), e))
        .collect();
    for n in &mut nodes {
        if let Some(e) = by_path.get(n.id.as_str()) {
            // Attach an `llm` sub-object under the node's metrics —
            // additive and discoverable by the Composition view.
            let llm = serde_json::json!({
                "summary": e.summary,
                "responsibility": e.responsibility,
                "tags": e.tags,
                "generated_at": e.generated_at,
                "model": e.model,
            });
            if let Some(obj) = n.metrics.as_object_mut() {
                obj.insert("llm".into(), llm);
            }
        }
    }
    nodes
}

fn build_score_breakdown(audit_result: &AuditResult) -> ScoreBreakdown {
    let issues = audit_result.issues();
    let points_lost = (100.0 - audit_result.score).clamp(0.0, 100.0);
    let by_kind = IssueKind::ALL
        .iter()
        .map(|k| KindPoints {
            kind: k.id(),
            kind_name: k.name(),
            points: issues
                .iter()
                .filter(|i| i.kind() == *k)
                .map(Issue::score_impact)
                .sum(),
        })
        .filter(|row| row.points > 0.0)
        .collect();

    let mut scoring: Vec<&Issue> = issues.iter().filter(|i| i.score_impact() > 0.0).collect();
    scoring.sort_by(|a, b| {
        b.score_impact()
            .partial_cmp(&a.score_impact())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top_contributors = scoring
        .into_iter()
        .take(5)
        .map(|i| ScoreContributor {
            kind: i.kind().id(),
            kind_name: i.kind().name(),
            subject: i.subject().to_string(),
            focus_id: participants_of(i, audit_result)
                .into_iter()
                .next()
                .unwrap_or_default(),
            points: i.score_impact(),
            fingerprint: i.fingerprint(),
        })
        .collect();

    ScoreBreakdown {
        total_modules: audit_result.total_modules,
        points_lost,
        by_kind,
        top_contributors,
    }
}

/// The node ids an Issue is about: what focus mode expands, highlights
/// and scopes to. Edge-shaped kinds list both ends; a Cycle its ring; a
/// Gravity Well the well plus every module pulling on it (#333); the
/// directory-shaped kinds their directory. Exhaustive on purpose.
fn participants_of(issue: &Issue, audit_result: &AuditResult) -> Vec<String> {
    let mut out: Vec<String> = match &issue.detail {
        IssueDetail::CouplingViolation(v) => vec![v.from_module.clone(), v.to_module.clone()],
        IssueDetail::Cycle(v) => {
            // cycle_path closes back on its first member; members are the
            // directories, but the canvas keys on files when hops are known.
            let mut members: Vec<String> = v.cycle_path.clone();
            if members.len() > 1 && members.first() == members.last() {
                members.pop();
            }
            if !v.cycle_hop_files.is_empty() {
                members = v
                    .cycle_hop_files
                    .iter()
                    .map(|(from, _, _)| from.clone())
                    .collect();
            }
            members
        }
        IssueDetail::RuleViolation(r) => vec![r.from_module.clone(), r.to_module.clone()],
        IssueDetail::LayerViolation(l) => vec![l.from_module.clone(), l.to_module.clone()],
        IssueDetail::GravityWell(g) => {
            let mut ids = vec![g.module_path.clone()];
            for v in audit_result
                .violations
                .iter()
                .chain(audit_result.coupling_metrics.iter())
            {
                if v.from_module == g.module_path {
                    ids.push(v.to_module.clone());
                } else if v.to_module == g.module_path {
                    ids.push(v.from_module.clone());
                }
            }
            ids
        }
        IssueDetail::RedFlag(f) => f.modules.clone(),
        IssueDetail::StabilityViolation(s) => vec![s.from_dir.clone(), s.to_dir.clone()],
        IssueDetail::ZoneFlag(d) => vec![d.dir.clone()],
        IssueDetail::LowCohesion(c) => vec![c.dir.clone()],
    };
    out.retain(|p| !p.is_empty());
    let mut seen = HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

fn severity_band(s: f64) -> &'static str {
    if s >= 0.5 {
        "high"
    } else if s >= 0.2 {
        "medium"
    } else {
        "low"
    }
}

fn build_dependency_rules(rules: &[DependencyRule]) -> Vec<DependencyRuleEntry> {
    rules
        .iter()
        .map(|r| DependencyRuleEntry {
            from: r.from.clone(),
            to: r.to.clone(),
            allow: r.allow,
            message: r.message.clone(),
        })
        .collect()
}

fn build_effective_rules(
    layers: &[Layer],
    rules: &[DependencyRule],
    audit_result: &AuditResult,
) -> Vec<EffectiveRuleEntry> {
    let mut out: Vec<EffectiveRuleEntry> = rules
        .iter()
        .map(|r| EffectiveRuleEntry {
            from: r.from.clone(),
            to: r.to.clone(),
            allow: r.allow,
            message: r.message.clone(),
            source: "dependency_rule",
            current_violation_count: audit_result
                .rule_violations
                .iter()
                .filter(|rv| rv.message == r.message)
                .count(),
        })
        .collect();

    // Layer-order implicit rules: a layer at index j may not depend on any layer at index i < j.
    for (j, lower) in layers.iter().enumerate() {
        for (i, upper) in layers.iter().enumerate().take(j) {
            out.push(EffectiveRuleEntry {
                from: lower.pattern.clone(),
                to: upper.pattern.clone(),
                allow: false,
                message: format!(
                    "Layer '{}' (index {}) may not depend on layer '{}' (index {}) — layers flow downward.",
                    lower.name, j, upper.name, i
                ),
                source: "layer_order",
                current_violation_count: audit_result
                    .layer_violations
                    .iter()
                    .filter(|lv| lv.from_layer == lower.name && lv.to_layer == upper.name)
                    .count(),
            });
        }
    }

    out
}

fn build_layers(layers: &[Layer], modules: &[Module], deps: &[Dependency]) -> Vec<LayerEntry> {
    let id_to_path: HashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    let compiled: Vec<Option<globset::GlobMatcher>> = layers
        .iter()
        .map(|l| Glob::new(&l.pattern).ok().map(|g| g.compile_matcher()))
        .collect();

    let layer_of = |path: &str| -> Option<usize> {
        compiled
            .iter()
            .enumerate()
            .find_map(|(i, m)| m.as_ref().and_then(|mm| mm.is_match(path).then_some(i)))
    };

    let file_counts: Vec<usize> = layers
        .iter()
        .enumerate()
        .map(|(i, _)| {
            modules
                .iter()
                .filter(|m| matches!(m.module_type, ModuleType::File))
                .filter(|m| layer_of(&m.path) == Some(i))
                .count()
        })
        .collect();

    let mut afferent = vec![0usize; layers.len()];
    let mut efferent = vec![0usize; layers.len()];
    for d in deps {
        let from = id_to_path
            .get(d.from_module_id.as_str())
            .and_then(|p| layer_of(p));
        let to = id_to_path
            .get(d.to_module_id.as_str())
            .and_then(|p| layer_of(p));
        if let (Some(f), Some(t)) = (from, to) {
            if f != t {
                efferent[f] += 1;
                afferent[t] += 1;
            }
        }
    }

    layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let ca = afferent[i];
            let ce = efferent[i];
            let instability = if ca + ce == 0 {
                None
            } else {
                Some(ce as f64 / (ca + ce) as f64)
            };
            LayerEntry {
                name: l.name.clone(),
                pattern: l.pattern.clone(),
                allow_sibling: l.allow_sibling,
                index: i,
                file_count: file_counts[i],
                afferent: ca,
                efferent: ce,
                instability,
            }
        })
        .collect()
}

fn build_codebase(
    modules: &[Module],
    dependencies: &[Dependency],
    audit_result: &AuditResult,
    snapshot: &Snapshot,
) -> Codebase {
    let files: Vec<&Module> = modules
        .iter()
        .filter(|m| matches!(m.module_type, ModuleType::File))
        .collect();

    let mut buckets: BTreeMap<String, usize> = BTreeMap::new();
    for m in &files {
        if let Some(ext) = m.path.rsplit('.').next() {
            *buckets.entry(ext.to_string()).or_insert(0) += 1;
        }
    }
    let mut language_distribution: Vec<LanguageEntry> = buckets
        .into_iter()
        .map(|(language, file_count)| LanguageEntry {
            language,
            file_count,
        })
        .collect();
    language_distribution.sort_by(|a, b| {
        b.file_count
            .cmp(&a.file_count)
            .then_with(|| a.language.cmp(&b.language))
    });

    Codebase {
        path: snapshot.root_path.clone(),
        module_count: audit_result.total_modules,
        file_count: files.len(),
        edge_count: dependencies.len(),
        language_distribution,
    }
}

#[cfg(test)]
mod builder_tests {
    //! Per-stage isolation tests for `ContractBuilder`. Each test
    //! constructs a builder and asks for one stage — the previous
    //! end-to-end style had to run all sixteen stages through
    //! `render()` to assert on any single field.

    use super::*;
    use noupling_core::analyzer::{AuditResultBuilder, GravityWell, RedFlag, RedFlagType};
    use noupling_core::core::Snapshot;

    fn default_inputs() -> (Settings, Snapshot) {
        let settings: Settings = serde_json::from_str("{}").expect("default settings");
        let snapshot = Snapshot {
            id: "snap".into(),
            timestamp: "2026-06-05T00:00:00".into(),
            root_path: "/tmp".into(),
        };
        (settings, snapshot)
    }

    #[test]
    fn summary_counts_isolates_one_stage_without_running_the_rest() {
        let (settings, snapshot) = default_inputs();
        let audit = AuditResultBuilder::new()
            .with_gravity_wells(vec![GravityWell {
                module_path: "infra/db".into(),
                total_rri: 12.0,
                relationship_count: 4,
                downward_rri: 0.0,
                sibling_rri: 0.0,
                upward_rri: 12.0,
                circular_rri: 0.0,
                direction_count: 1,
            }])
            .with_red_flags(vec![RedFlag {
                flag_type: RedFlagType::FusedSibling,
                modules: vec!["a".into(), "b".into()],
                dir_a: String::new(),
                dir_b: String::new(),
                rri: 5.0,
                imports: 0,
                median_density: 0.0,
                recommendation: "merge".into(),
            }])
            .build();
        let options = RenderOptions::default();
        let builder = ContractBuilder::new(&[], &[], &audit, &settings, &snapshot, &options);

        // No call to .build() — just one stage in isolation.
        let counts = builder.summary_counts();
        assert_eq!(counts.gravity_wells, 1);
        assert_eq!(counts.red_flags, 1);
        assert_eq!(counts.cycles, 0);
    }

    #[test]
    fn history_stage_emits_empty_when_include_history_false() {
        let (settings, snapshot) = default_inputs();
        let audit = AuditResultBuilder::new().build();
        let options = RenderOptions {
            include_history: false,
            history: vec![crate::HistoryEntry {
                snapshot_id: "s1".into(),
                taken_at: "t".into(),
                health_score: 80.0,
            }],
            ..Default::default()
        };
        let builder = ContractBuilder::new(&[], &[], &audit, &settings, &snapshot, &options);

        assert!(builder.history().is_empty());
    }

    #[test]
    fn report_options_stage_passes_editor_and_title_through() {
        let (settings, snapshot) = default_inputs();
        let audit = AuditResultBuilder::new().build();
        let options = RenderOptions {
            editor: Some("vscode".into()),
            title: Some("acme".into()),
            ..Default::default()
        };
        let builder = ContractBuilder::new(&[], &[], &audit, &settings, &snapshot, &options);

        let ro = builder.report_options();
        assert_eq!(ro.editor.as_deref(), Some("vscode"));
        assert_eq!(ro.title.as_deref(), Some("acme"));
    }
}
