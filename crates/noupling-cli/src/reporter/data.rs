//! The canonical `Report` shape — `JsonReport` and its sub-structs,
//! plus the audit-result-to-report builder. Every reporter format
//! reads this shape; nothing here knows about output formatting.
//!
//! Carved out of `reporter/mod.rs` per issue #319 so format adapters
//! (xml, sonar, text, pr, briefing, md, html, dashboard, …) sit
//! sibling-to-sibling instead of nested inside one 2000-line file.

use std::collections::BTreeMap;

use noupling_core::analyzer::{common_parent_dir, AuditResult, CouplingViolation, IssueCard};
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
    /// Every Issue as an Issue card, in `issues()` order (ADR 0002). The
    /// per-kind arrays below are kept until #350 removes them.
    pub issues: Vec<IssueCard>,
    pub critical_violations: usize,
    pub total_circular: usize,
    pub total_coupling: usize,
    pub circular_dependencies: BTreeMap<String, Vec<JsonCircularViolation>>,
    pub coupling_violations: Vec<JsonCouplingViolation>,
    pub hotspots: Vec<JsonHotspot>,
    pub gravity_wells: Vec<JsonGravityWell>,
    pub red_flags: Vec<JsonRedFlag>,
    pub directory_tree: Vec<JsonDirectory>,
    pub abstractness: Vec<JsonAbstractness>,
    pub instability: Vec<JsonInstability>,
    pub stability_violations: Vec<JsonStabilityViolation>,
    pub distance: Vec<JsonDistance>,
    pub cohesion: Vec<JsonCohesion>,
}

#[derive(Serialize)]
pub struct JsonRedFlag {
    pub flag_type: String,
    pub modules: Vec<String>,
    pub rri: f64,
    pub recommendation: String,
}

#[derive(Serialize)]
pub struct JsonGravityWell {
    pub module_path: String,
    pub total_rri: f64,
    pub relationship_count: usize,
    pub direction_count: usize,
    pub direction_breakdown: JsonDirectionBreakdown,
}

#[derive(Serialize)]
pub struct JsonDirectionBreakdown {
    pub downward: f64,
    pub sibling: f64,
    pub upward: f64,
    pub circular: f64,
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
pub struct JsonCohesion {
    pub dir: String,
    /// "Container" or "Package".
    pub kind: &'static str,
    pub n_children: usize,
    pub internal_deps: usize,
    /// `null` for Containers; numeric for Packages.
    pub cohesion: Option<f64>,
}

#[derive(Serialize)]
pub struct JsonStabilityViolation {
    pub from_dir: String,
    pub to_dir: String,
    pub from_instability: f64,
    pub to_instability: f64,
}

#[derive(Serialize)]
pub struct JsonDistance {
    pub dir: String,
    pub abstractness: f64,
    pub instability: f64,
    pub distance: f64,
    pub zone: &'static str,
}

#[derive(Serialize)]
pub struct JsonCircularViolation {
    pub severity: f64,
    pub cycle_order: usize,
    pub cycle_path: Vec<String>,
    pub cycle_short_path: Vec<String>,
    pub hop_files: Vec<JsonHopFile>,
    pub weakest_link: Option<String>,
    pub break_cost: usize,
}

#[derive(Serialize)]
pub struct JsonHopFile {
    pub from_dir: String,
    pub from_file: String,
    pub to_file: String,
}

#[derive(Serialize)]
pub struct JsonCouplingViolation {
    pub severity: f64,
    pub weight: usize,
    pub rri: f64,
    pub direction: String,
    pub from_module: String,
    pub to_module: String,
    pub dir_a: String,
    pub dir_b: String,
    pub depth: i32,
}

#[derive(Serialize)]
pub struct JsonDirectory {
    pub path: String,
    pub name: String,
    pub module_count: usize,
    pub score: f64,
    pub has_violations: bool,
    pub children: Vec<String>,
    pub files: Vec<String>,
    pub violations_count: usize,
    pub circular_count: usize,
}

impl JsonReport {
    pub fn from_audit(modules: &[Module], result: &AuditResult, snapshot_id: &str) -> Self {
        let critical_violations = result
            .violations
            .iter()
            .filter(|v| v.severity >= 0.5)
            .count();

        let circular: Vec<&CouplingViolation> =
            result.violations.iter().filter(|v| v.is_circular).collect();

        let coupling: Vec<&CouplingViolation> = result
            .violations
            .iter()
            .filter(|v| !v.is_circular)
            .collect();

        // Group circular by order
        let mut circular_by_order: BTreeMap<String, Vec<JsonCircularViolation>> = BTreeMap::new();
        for v in &circular {
            let label = match v.cycle_order {
                2 => "Mutual Dependencies (Order 2)".to_string(),
                3 => "Triangular Cycles (Order 3)".to_string(),
                n => format!("Cycles of Order {}", n),
            };
            let short_path: Vec<String> = v
                .cycle_path
                .iter()
                .map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or(p)
                        .to_string()
                })
                .collect();

            let mut hop_files = Vec::new();
            for (i, dir) in v.cycle_path.iter().enumerate() {
                let dir_short = std::path::Path::new(dir)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(dir)
                    .to_string();
                if i < v.cycle_hop_files.len() {
                    let (from_file, to_file, _line) = &v.cycle_hop_files[i];
                    hop_files.push(JsonHopFile {
                        from_dir: dir_short,
                        from_file: from_file.clone(),
                        to_file: to_file.clone(),
                    });
                } else if i == v.cycle_path.len() - 1 && !v.cycle_hop_files.is_empty() {
                    let (_, to_file, _) = &v.cycle_hop_files[v.cycle_hop_files.len() - 1];
                    hop_files.push(JsonHopFile {
                        from_dir: dir_short,
                        from_file: to_file.clone(),
                        to_file: String::new(),
                    });
                }
            }

            circular_by_order
                .entry(label)
                .or_default()
                .push(JsonCircularViolation {
                    severity: v.severity,
                    cycle_order: v.cycle_order,
                    cycle_path: v.cycle_path.clone(),
                    cycle_short_path: short_path,
                    hop_files,
                    weakest_link: v.weakest_link.clone(),
                    break_cost: v.break_cost,
                });
        }

        let coupling_violations: Vec<JsonCouplingViolation> = coupling
            .iter()
            .map(|v| JsonCouplingViolation {
                severity: v.severity,
                weight: v.weight,
                rri: v.rri,
                direction: format!("{:?}", v.direction).to_lowercase(),
                from_module: v.from_module.clone(),
                to_module: v.to_module.clone(),
                dir_a: v.dir_a.clone(),
                dir_b: v.dir_b.clone(),
                depth: v.depth,
            })
            .collect();

        // Build directory tree
        let directory_tree = build_json_dir_tree(modules, result);

        // Hotspots
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
            total_circular: circular.len(),
            total_coupling: coupling.len(),
            circular_dependencies: circular_by_order,
            coupling_violations,
            hotspots,
            gravity_wells: result
                .gravity_wells
                .iter()
                .map(|g| JsonGravityWell {
                    module_path: g.module_path.clone(),
                    total_rri: g.total_rri,
                    relationship_count: g.relationship_count,
                    direction_count: g.direction_count,
                    direction_breakdown: JsonDirectionBreakdown {
                        downward: g.downward_rri,
                        sibling: g.sibling_rri,
                        upward: g.upward_rri,
                        circular: g.circular_rri,
                    },
                })
                .collect(),
            red_flags: result
                .red_flags
                .iter()
                .map(|f| JsonRedFlag {
                    flag_type: format!("{:?}", f.flag_type),
                    modules: f.modules.clone(),
                    rri: f.rri,
                    recommendation: f.recommendation.clone(),
                })
                .collect(),
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
            stability_violations: result
                .stability_violations
                .iter()
                .map(|v| JsonStabilityViolation {
                    from_dir: v.from_dir.clone(),
                    to_dir: v.to_dir.clone(),
                    from_instability: v.from_instability,
                    to_instability: v.to_instability,
                })
                .collect(),
            distance: result
                .distance
                .iter()
                .map(|d| JsonDistance {
                    dir: d.dir.clone(),
                    abstractness: d.abstractness,
                    instability: d.instability,
                    distance: d.distance,
                    zone: match d.zone {
                        noupling_core::analyzer::Zone::MainSequence => "main_sequence",
                        noupling_core::analyzer::Zone::Pain => "zone_of_pain",
                        noupling_core::analyzer::Zone::Uselessness => "zone_of_uselessness",
                    },
                })
                .collect(),
            cohesion: result
                .cohesion
                .iter()
                .map(|c| JsonCohesion {
                    dir: c.dir.clone(),
                    kind: match c.kind {
                        noupling_core::analyzer::DirectoryKind::Container => "Container",
                        noupling_core::analyzer::DirectoryKind::Package => "Package",
                    },
                    n_children: c.n_children,
                    internal_deps: c.internal_deps,
                    cohesion: c.cohesion,
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

    // Count violations per directory
    for v in &result.violations {
        // Same anchor rule as Issue::anchor_dir, so a violation is counted
        // under the directory whose page lists its Issue. Counts are raw
        // violations (ring hops count separately; see #358).
        let parent = if v.is_circular && !v.cycle_path.is_empty() {
            let members: Vec<&str> = v.cycle_path.iter().map(String::as_str).collect();
            common_parent_dir(&members)
        } else {
            common_parent_dir(&[&v.dir_a, &v.dir_b])
        };
        if let Some(dir) = dirs.get_mut(&parent) {
            dir.violations_count += 1;
            if v.is_circular {
                dir.circular_count += 1;
            }
            dir.has_violations = true;
        }
    }

    // Mark dirs with deep violations
    for path in &sorted_paths {
        let has_child_violations = {
            let prefix = format!("{}/", path);
            dir_paths.iter().any(|p| {
                p.starts_with(&prefix) && dirs.get(p).map(|d| d.has_violations).unwrap_or(false)
            })
        };
        if has_child_violations {
            if let Some(dir) = dirs.get_mut(path) {
                dir.has_violations = true;
            }
        }
    }

    // Sort children
    for dir in dirs.values_mut() {
        dir.children.sort();
        dir.files.sort();
    }

    dirs.into_values().collect()
}
