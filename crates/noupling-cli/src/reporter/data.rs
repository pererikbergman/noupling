//! The canonical `Report` shape — `JsonReport` and its sub-structs,
//! plus the audit-result-to-report builder. Every reporter format
//! reads this shape; nothing here knows about output formatting.
//!
//! Carved out of `reporter/mod.rs` per issue #319 so format adapters
//! (xml, sonar, text, pr, briefing, md, html, dashboard, …) sit
//! sibling-to-sibling instead of nested inside one 2000-line file.

use std::collections::BTreeMap;

use noupling_core::analyzer::{common_parent_dir, AuditResult, IssueCard, IssueKind, SeverityBand};
use noupling_core::core::Module;
use serde::Serialize;

use super::VERSION;

#[derive(Serialize)]
pub struct JsonReport {
    pub generator: String,
    pub snapshot_id: String,
    pub score: f64,
    pub tri: f64,
    pub total_modules: usize,
    pub total_xs: usize,
    pub max_depth: usize,
    pub suppressed_count: usize,
    pub total_external_imports: usize,
    pub violation_age: JsonViolationAge,
    /// Every Issue as an Issue card, in `issues()` order (ADR 0002).
    pub issues: Vec<IssueCard>,
    pub critical_violations: usize,
    pub total_circular: usize,
    pub total_coupling: usize,
    // Metric arrays. Every Issue-bearing array (coupling_violations,
    // circular_dependencies, gravity_wells, red_flags, stability_violations,
    // distance, cohesion) was replaced by `issues` in 0.9.0 (ADR 0002, #350):
    // filter `issues` by `kind` instead.
    pub hotspots: Vec<JsonHotspot>,
    pub directory_tree: Vec<JsonDirectory>,
    pub abstractness: Vec<JsonAbstractness>,
    pub instability: Vec<JsonInstability>,
}

#[derive(Serialize)]
pub struct JsonViolationAge {
    pub new_count: usize,
    pub recent_count: usize,
    pub chronic_count: usize,
}

#[derive(Serialize)]
pub struct JsonHotspot {
    pub path: String,
    pub fan_in: usize,
    pub fan_out: usize,
}

#[derive(Serialize)]
pub struct JsonAbstractness {
    pub dir: String,
    pub abstract_count: usize,
    pub concrete_count: usize,
    pub abstractness: f64,
}

#[derive(Serialize)]
pub struct JsonInstability {
    pub dir: String,
    pub ca: usize,
    pub ce: usize,
    pub instability: f64,
}

#[derive(Serialize)]
pub struct JsonDirectory {
    pub path: String,
    pub name: String,
    /// `"Container"` (only subdirectories) or `"Package"` (has direct files),
    /// per `docs/dependency-graph.md`. Carried here since 0.9.0, when the
    /// `cohesion` array that used to expose it was replaced by `issues`.
    pub kind: &'static str,
    pub module_count: usize,
    pub score: f64,
    /// A violation is anchored here or anywhere below (rolled up).
    pub has_violations: bool,
    pub children: Vec<String>,
    pub files: Vec<String>,
    /// Coupling Violations and Cycles anchored at this directory **or any
    /// descendant** (since 0.9.1, #381; before that, this directory only).
    /// `score` stays local to the directory's own violations.
    pub violations_count: usize,
    /// The Cycles among `violations_count`, rolled up the same way.
    pub circular_count: usize,
}

impl JsonReport {
    pub fn from_audit(modules: &[Module], result: &AuditResult, snapshot_id: &str) -> Self {
        // Header counts agree with `issues`: ring hops belong to their Cycle
        // (#358), so they are not counted as violations of their own, and
        // "critical" is the card's band — assigned once by the audit — not a
        // second reading of the raw severity (#380, #379).
        let issue_violations = result.issue_violations();
        let critical_violations = result
            .issues()
            .iter()
            .filter(|i| {
                matches!(i.kind(), IssueKind::CouplingViolation | IssueKind::Cycle)
                    && i.severity() == SeverityBand::Critical
            })
            .count();
        let total_circular = issue_violations.iter().filter(|v| v.is_circular).count();
        let total_coupling = issue_violations.len() - total_circular;

        let hotspots: Vec<JsonHotspot> = result
            .hotspots
            .iter()
            .filter(|h| h.fan_in > 0)
            .map(|h| JsonHotspot {
                path: h.path.clone(),
                fan_in: h.fan_in,
                fan_out: h.fan_out,
            })
            .collect();

        let directory_tree = build_json_dir_tree(modules, result);

        JsonReport {
            generator: VERSION.to_string(),
            snapshot_id: snapshot_id.to_string(),
            score: result.score,
            tri: result.tri,
            total_modules: result.total_modules,
            total_xs: result.total_xs,
            max_depth: result.max_depth,
            suppressed_count: result.suppressed_count,
            total_external_imports: result.total_external_imports,
            violation_age: JsonViolationAge {
                new_count: result.violation_age.new_count,
                recent_count: result.violation_age.recent_count,
                chronic_count: result.violation_age.chronic_count,
            },
            issues: result.issues().iter().map(|i| i.to_card()).collect(),
            critical_violations,
            total_circular,
            total_coupling,
            hotspots,
            directory_tree,
            abstractness: result
                .abstractness
                .iter()
                .map(|a| JsonAbstractness {
                    dir: a.dir.clone(),
                    abstract_count: a.abstract_count,
                    concrete_count: a.concrete_count,
                    abstractness: a.abstractness,
                })
                .collect(),
            instability: result
                .instability
                .iter()
                .map(|i| JsonInstability {
                    dir: i.dir.clone(),
                    ca: i.ca,
                    ce: i.ce,
                    instability: i.instability,
                })
                .collect(),
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

fn build_json_dir_tree(modules: &[Module], result: &AuditResult) -> Vec<JsonDirectory> {
    let mut dirs: BTreeMap<String, JsonDirectory> = BTreeMap::new();

    // Collect directories from module paths
    for module in modules {
        let path = std::path::Path::new(&module.path);
        let mut current = path.parent();
        while let Some(dir) = current {
            let dir_str = dir.to_string_lossy().to_string();
            if dir_str.is_empty() {
                break;
            }
            dirs.entry(dir_str.clone()).or_insert_with(|| {
                let name = dir
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| dir_str.clone());
                JsonDirectory {
                    path: dir_str.clone(),
                    name,
                    kind: "Container",
                    module_count: 0,
                    score: 100.0,
                    has_violations: false,
                    children: Vec::new(),
                    files: Vec::new(),
                    violations_count: 0,
                    circular_count: 0,
                }
            });
            current = dir.parent();
        }
    }

    // Assign files
    for module in modules {
        let parent = std::path::Path::new(&module.path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Some(dir) = dirs.get_mut(&parent) {
            dir.files.push(module.name.clone());
            dir.module_count += 1;
            // A directory with at least one direct file is a Package.
            dir.kind = "Package";
        }
    }

    // Build children
    let dir_paths: Vec<String> = dirs.keys().cloned().collect();
    for dir_path in &dir_paths {
        let parent = std::path::Path::new(dir_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if dirs.contains_key(&parent) && &parent != dir_path {
            let name = dirs.get(dir_path).unwrap().name.clone();
            if let Some(parent_dir) = dirs.get_mut(&parent) {
                if !parent_dir.children.contains(&name) {
                    parent_dir.children.push(name);
                }
            }
        }
    }

    // Propagate module counts and count violations per dir
    let mut sorted_paths: Vec<String> = dirs.keys().cloned().collect();
    sorted_paths.sort_by_key(|a| std::cmp::Reverse(a.len()));

    for path in &sorted_paths {
        let child_count: usize = {
            let _dir = dirs.get(path).unwrap();
            let prefix = format!("{}/", path);
            dir_paths
                .iter()
                .filter(|p| {
                    p.starts_with(&prefix)
                        && p.matches('/').count() == path.matches('/').count() + 1
                })
                .filter_map(|p| dirs.get(p).map(|d| d.module_count))
                .sum()
        };
        if let Some(dir) = dirs.get_mut(path) {
            dir.module_count += child_count;
        }
    }

    // Count violations per directory. Same anchor rule as Issue::anchor_dir,
    // so a violation is counted under the directory whose page lists its
    // Issue; ring hops are folded into their Cycle so the count matches the
    // cards (#358). Counts roll up to every ancestor like `module_count`
    // does, so a directory flagged `has_violations` never shows 0 (#381).
    for v in result.issue_violations() {
        let anchor = if v.is_circular && !v.cycle_path.is_empty() {
            let members: Vec<&str> = v.cycle_path.iter().map(String::as_str).collect();
            common_parent_dir(&members)
        } else {
            common_parent_dir(&[&v.dir_a, &v.dir_b])
        };
        let mut current = Some(std::path::Path::new(&anchor));
        while let Some(dir_path) = current {
            let key = dir_path.to_string_lossy();
            if key.is_empty() {
                break;
            }
            if let Some(dir) = dirs.get_mut(key.as_ref()) {
                dir.violations_count += 1;
                if v.is_circular {
                    dir.circular_count += 1;
                }
                dir.has_violations = true;
            }
            current = dir_path.parent();
        }
    }

    // Sort children
    for dir in dirs.values_mut() {
        dir.children.sort();
        dir.files.sort();
    }

    dirs.into_values().collect()
}
