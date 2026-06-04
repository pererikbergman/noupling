//! Data Contract serialization for the Explorer.
//!
//! Builds the JSON shape documented in `docs/noupling-explorer-prd.md` §6,
//! which the embedded template hydrates at startup. Fields are additive
//! across versions; the template treats unknown keys as optional.

use globset::Glob;
use noupling_core::analyzer::AuditResult;
use noupling_core::core::{Dependency, Module, ModuleType, Snapshot};
use noupling_core::settings::{DependencyRule, Layer, Settings};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

use crate::RenderOptions;

#[derive(Debug, Serialize)]
pub(crate) struct DataContract {
    pub format_version: u32,
    pub noupling_version: String,
    pub generated_at: String,
    pub report_options: ReportOptions,
    pub codebase: Codebase,
    pub health_score: f64,
    pub summary_counts: SummaryCounts,
    pub layers: Vec<LayerEntry>,
    pub dependency_rules: Vec<DependencyRuleEntry>,
    pub effective_rules: Vec<EffectiveRuleEntry>,
    pub nodes: Vec<NodeEntry>,
    pub edges: Vec<EdgeEntry>,
    pub cycles: Vec<CycleEntry>,
    pub violations: Vec<ViolationEntry>,
    pub history: Vec<HistoryEntry>,
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
    pub minimum_cut: Vec<EdgeRef>,
}

#[derive(Debug, Serialize)]
pub(crate) struct EdgeRef {
    pub from: String,
    pub to: String,
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
    DataContract {
        format_version: 1,
        noupling_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at: snapshot.timestamp.clone(),
        report_options: ReportOptions {
            editor: options.editor.clone(),
            title: options.title.clone(),
        },
        codebase: build_codebase(modules, dependencies, audit_result, snapshot),
        health_score: audit_result.score,
        summary_counts: SummaryCounts {
            violations: audit_result.violations.len(),
            cycles: audit_result
                .violations
                .iter()
                .filter(|v| v.is_circular)
                .count(),
            gravity_wells: audit_result.gravity_wells.len(),
            red_flags: audit_result.red_flags.len(),
        },
        layers: build_layers(&settings.layers, modules, dependencies),
        dependency_rules: build_dependency_rules(&settings.dependency_rules),
        effective_rules: build_effective_rules(
            &settings.layers,
            &settings.dependency_rules,
            audit_result,
        ),
        nodes: build_nodes(&settings.layers, modules, audit_result),
        edges: build_edges(modules, dependencies, audit_result),
        cycles: build_cycles(audit_result),
        violations: build_violations(audit_result),
        // `include_history` is plumbed for future use; for now the
        // history block is always empty because we don't yet read prior
        // snapshots from storage here. Wired in a later slice.
        history: {
            let _ = options.include_history;
            Vec::new()
        },
    }
}

fn build_nodes(layers: &[Layer], modules: &[Module], audit_result: &AuditResult) -> Vec<NodeEntry> {
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

    // File nodes
    for m in modules
        .iter()
        .filter(|m| matches!(m.module_type, ModuleType::File))
    {
        let parent = parent_dir(&m.path).map(str::to_string);
        nodes.push(NodeEntry {
            id: m.path.clone(),
            kind: "file",
            parent,
            layer: layer_of(&m.path),
            metrics: serde_json::json!({
                "afferent": 0,
                "efferent": 0,
                "instability": null,
                "loc": 0,
            }),
        });
    }

    // Directory nodes: package (has direct files) or container (only subdirs).
    let mut dirs: BTreeMap<String, DirInfo> = BTreeMap::new();
    for m in modules
        .iter()
        .filter(|m| matches!(m.module_type, ModuleType::File))
    {
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

        // Pull cohesion from the audit when this is a Package.
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

        nodes.push(NodeEntry {
            id: path.clone(),
            kind,
            parent,
            layer: layer_of(path),
            metrics: serde_json::json!({
                "afferent": 0,
                "efferent": 0,
                "instability": null,
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
        .map(|(i, v)| CycleEntry {
            id: format!("cycle-{}", i + 1),
            size: v.cycle_order,
            members: v.cycle_path.clone(),
            minimum_cut: v
                .weakest_link
                .as_ref()
                .and_then(|wl| parse_weakest_link(wl))
                .map(|(f, t)| vec![EdgeRef { from: f, to: t }])
                .unwrap_or_default(),
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
