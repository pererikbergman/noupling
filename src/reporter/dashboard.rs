//! Interactive Technical Leader Dashboard.

use crate::analyzer::AuditResult;
use crate::core::{Dependency, Module};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize)]
pub struct DashboardData {
    pub score: f64,
    pub total_modules: usize,
    pub total_violations: usize,
    pub total_xs: usize,
    pub max_depth: usize,
    pub critical_path: Vec<String>,
    pub violation_age: ViolationAge,
    pub lowest_independence: Option<IndependenceEntry>,
    pub highest_blast: Option<BlastEntry>,
    pub modules_table: Vec<ModuleRow>,
    pub risk_matrix: Vec<RiskDot>,
    pub violation_types: ViolationTypes,
    pub quick_wins: Vec<QuickWin>,
    pub sunburst_tree: serde_json::Value,
    pub sunburst_deps: Vec<DepEdge>,
    pub sunburst_violation_deps: Vec<DepEdge>,
    pub top_actions: Vec<DashboardAction>,
    pub projected_score: f64,
}

#[derive(Serialize)]
pub struct DashboardAction {
    pub title: String,
    pub detail: String,
    pub action: String,
    pub cost: usize,
    pub category: String,
}

#[derive(Serialize)]
pub struct ViolationAge {
    pub new_count: usize,
    pub recent_count: usize,
    pub chronic_count: usize,
}

#[derive(Serialize)]
pub struct IndependenceEntry {
    pub name: String,
    pub score: f64,
}

#[derive(Serialize)]
pub struct BlastEntry {
    pub name: String,
    pub blast_radius: usize,
}

#[derive(Serialize)]
pub struct ModuleRow {
    pub name: String,
    pub score: f64,
    pub file_count: usize,
    pub violations: usize,
    pub circular: usize,
    pub xs: usize,
    pub independence: f64,
    pub instability: f64,
}

#[derive(Serialize)]
pub struct RiskDot {
    pub name: String,
    pub instability: f64,
    pub blast_radius: usize,
    pub file_count: usize,
    pub score: f64,
}

#[derive(Serialize)]
pub struct ViolationTypes {
    pub coupling: usize,
    pub circular: usize,
}

#[derive(Serialize)]
pub struct QuickWin {
    pub from: String,
    pub to: String,
    pub cost: usize,
    pub is_circular: bool,
}

#[derive(Serialize)]
pub struct DepEdge {
    pub from: String,
    pub to: String,
}

pub fn generate_dashboard(
    modules: &[Module],
    dependencies: &[Dependency],
    result: &AuditResult,
    output_path: &std::path::Path,
) -> anyhow::Result<()> {
    let data = build_dashboard_data(modules, dependencies, result);
    let json = serde_json::to_string(&data)?;

    let html = format!(
        include_str!("dashboard_template.html"),
        json = json,
        version = super::VERSION,
    );

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, html)?;
    Ok(())
}

fn common_parent(a: &str, b: &str) -> String {
    let a_parts: Vec<&str> = a.split('/').collect();
    let b_parts: Vec<&str> = b.split('/').collect();
    let mut len = 0;
    for (x, y) in a_parts.iter().zip(b_parts.iter()) {
        if x == y {
            len += 1;
        } else {
            break;
        }
    }
    if len > 0 {
        a_parts[..len].join("/")
    } else {
        String::new()
    }
}

fn build_dashboard_data(
    modules: &[Module],
    dependencies: &[Dependency],
    result: &AuditResult,
) -> DashboardData {
    let id_to_path: HashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    // Assign each violation to its common parent directory (matching HTML report)
    let violation_parents: Vec<String> = result
        .violations
        .iter()
        .map(|v| common_parent(&v.dir_a, &v.dir_b))
        .collect();

    // Find common prefix to get meaningful top-level directories (matching HTML report)
    let all_paths: Vec<&str> = modules.iter().map(|m| m.path.as_str()).collect();
    let common_root = find_full_common_prefix(&all_paths);

    // Module scorecard: per first directory level after common prefix
    let mut dir_files: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for m in modules {
        let rel = strip_prefix(&m.path, &common_root);
        if let Some(top) = rel.split('/').next() {
            if rel.contains('/') {
                dir_files
                    .entry(top.to_string())
                    .or_default()
                    .push(m.id.as_str());
            }
        }
    }

    // Also strip common prefix from violation parents
    let violation_parents: Vec<String> = violation_parents
        .iter()
        .map(|p| strip_prefix(p, &common_root))
        .collect();

    let mut modules_table: Vec<ModuleRow> = Vec::new();
    for (dir, file_ids) in &dir_files {
        // Count violations whose common parent is within this directory
        let prefix = format!("{}/", dir);
        let violations = violation_parents
            .iter()
            .zip(result.violations.iter())
            .filter(|(p, _)| *p == dir || p.starts_with(&prefix))
            .count();
        let circular = violation_parents
            .iter()
            .zip(result.violations.iter())
            .filter(|(p, v)| v.is_circular && (*p == dir || p.starts_with(&prefix)))
            .count();

        let dir_sev: f64 = violation_parents
            .iter()
            .zip(result.violations.iter())
            .filter(|(p, _)| *p == dir || p.starts_with(&prefix))
            .map(|(_, v)| v.severity)
            .sum();
        let score = (100.0 * (1.0 - dir_sev / file_ids.len().max(1) as f64)).max(0.0);

        let xs: usize = violation_parents
            .iter()
            .zip(result.violations.iter())
            .filter(|(p, _)| *p == dir || p.starts_with(&prefix))
            .map(|(_, v)| {
                if v.is_circular {
                    v.break_cost
                } else {
                    v.weight
                }
            })
            .sum();

        let independence = result
            .independence
            .iter()
            .find(|i| i.dir == *dir)
            .map(|i| i.independence)
            .unwrap_or(1.0);

        // Average instability for modules in this dir
        let instabilities: Vec<f64> = result
            .hotspots
            .iter()
            .filter(|h| h.path.starts_with(dir.as_str()))
            .map(|h| h.instability)
            .collect();
        let avg_instability = if instabilities.is_empty() {
            0.5
        } else {
            instabilities.iter().sum::<f64>() / instabilities.len() as f64
        };

        modules_table.push(ModuleRow {
            name: dir.clone(),
            score,
            file_count: file_ids.len(),
            violations,
            circular,
            xs,
            independence,
            instability: avg_instability,
        });
    }
    modules_table.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());

    // Risk matrix: one dot per top-level directory
    let risk_matrix: Vec<RiskDot> = modules_table
        .iter()
        .map(|m| {
            let max_blast = result
                .hotspots
                .iter()
                .filter(|h| h.path.starts_with(&m.name))
                .map(|h| h.blast_radius)
                .max()
                .unwrap_or(0);
            RiskDot {
                name: m.name.clone(),
                instability: m.instability,
                blast_radius: max_blast,
                file_count: m.file_count,
                score: m.score,
            }
        })
        .collect();

    // Quick wins: violations with lowest cost
    let mut quick_wins: Vec<QuickWin> = result
        .violations
        .iter()
        .map(|v| {
            let cost = if v.is_circular {
                v.break_cost
            } else {
                v.weight
            };
            QuickWin {
                from: short_name(&v.from_module),
                to: short_name(&v.to_module),
                cost,
                is_circular: v.is_circular,
            }
        })
        .collect();
    quick_wins.sort_by_key(|w| w.cost);
    quick_wins.truncate(5);

    // Lowest independence
    let lowest_independence = result.independence.first().map(|i| IndependenceEntry {
        name: i.dir.clone(),
        score: i.independence,
    });

    // Highest blast radius
    let highest_blast = result
        .hotspots
        .iter()
        .max_by_key(|h| h.blast_radius)
        .map(|h| BlastEntry {
            name: short_name(&h.path),
            blast_radius: h.blast_radius,
        });

    // Violation types
    let coupling = result.violations.iter().filter(|v| !v.is_circular).count();
    let circular_count = result.violations.iter().filter(|v| v.is_circular).count();

    // Sunburst data (reuse bundle logic)
    let sunburst_tree = build_sunburst_tree(modules, result);
    let sunburst_deps = build_sunburst_deps(modules, dependencies, &id_to_path);

    // Violation-specific edges for the sunburst
    let paths_list: Vec<&str> = modules.iter().map(|m| m.path.as_str()).collect();
    let common_prefix = find_common_prefix(&paths_list);
    let mut sunburst_violation_deps = Vec::new();
    let mut v_seen = std::collections::HashSet::new();
    for v in &result.violations {
        if !v.is_circular {
            let f = strip_prefix(&v.from_module, &common_prefix);
            let t = strip_prefix(&v.to_module, &common_prefix);
            if v_seen.insert((f.clone(), t.clone())) {
                sunburst_violation_deps.push(DepEdge { from: f, to: t });
            }
        } else {
            for (from_file, to_file, _) in &v.cycle_hop_files {
                if !from_file.is_empty() && !to_file.is_empty() {
                    let f = strip_prefix(from_file, &common_prefix);
                    let t = strip_prefix(to_file, &common_prefix);
                    if v_seen.insert((f.clone(), t.clone())) {
                        sunburst_violation_deps.push(DepEdge { from: f, to: t });
                    }
                }
            }
        }
    }

    // Compute overall score as weighted average of module scores (matching HTML report)
    let total_files: usize = modules_table.iter().map(|m| m.file_count).sum();
    let weighted_score: f64 = modules_table
        .iter()
        .map(|m| m.score * m.file_count as f64)
        .sum::<f64>();
    let dashboard_score = if total_files > 0 {
        weighted_score / total_files as f64
    } else {
        result.score
    };
    let dashboard_violations: usize = modules_table.iter().map(|m| m.violations).sum();
    let dashboard_xs: usize = modules_table.iter().map(|m| m.xs).sum();

    // Top Actions — what to do
    let top_actions_raw = crate::analyzer::compute_top_actions(result, 5);
    let top_actions: Vec<DashboardAction> = top_actions_raw
        .iter()
        .map(|a| DashboardAction {
            title: a.title.clone(),
            detail: a.detail.clone(),
            action: a.action.clone(),
            cost: a.cost,
            category: a.category.clone(),
        })
        .collect();

    // Projected score if top 3 actions are completed.
    // Approximation: for circular/layer/rule violations, removing them eliminates their severity.
    let actionable_severity: f64 = top_actions_raw
        .iter()
        .take(3)
        .filter(|a| a.category != "hotspot")
        .map(|a| a.impact)
        .sum();
    let projected_score = if result.total_modules > 0 {
        let projected_sum =
            (100.0 - dashboard_score) * result.total_modules as f64 / 100.0 - actionable_severity;
        (100.0 * (1.0 - projected_sum.max(0.0) / result.total_modules as f64))
            .clamp(dashboard_score, 100.0)
    } else {
        dashboard_score
    };

    DashboardData {
        score: dashboard_score,
        total_modules: result.total_modules,
        total_violations: dashboard_violations,
        total_xs: dashboard_xs,
        max_depth: result.max_depth,
        critical_path: result.critical_path.iter().map(|p| short_name(p)).collect(),
        violation_age: ViolationAge {
            new_count: result.violation_age.new_count,
            recent_count: result.violation_age.recent_count,
            chronic_count: result.violation_age.chronic_count,
        },
        lowest_independence,
        highest_blast,
        modules_table,
        risk_matrix,
        violation_types: ViolationTypes {
            coupling,
            circular: circular_count,
        },
        quick_wins,
        sunburst_tree,
        sunburst_deps,
        sunburst_violation_deps,
        top_actions,
        projected_score,
    }
}

fn short_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path)
        .to_string()
}

fn build_sunburst_tree(modules: &[Module], result: &AuditResult) -> serde_json::Value {
    #[derive(Serialize)]
    struct Node {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        score: Option<f64>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        children: Vec<Node>,
    }

    let paths: Vec<&str> = modules.iter().map(|m| m.path.as_str()).collect();
    let common = find_common_prefix(&paths);

    // Compute per-directory severity and file count
    let mut dir_severity: HashMap<String, f64> = HashMap::new();
    let mut dir_files: HashMap<String, usize> = HashMap::new();
    for m in modules {
        let rel = strip_prefix(&m.path, &common);
        // Accumulate to every ancestor directory
        let parts: Vec<&str> = rel.split('/').collect();
        for end in 1..parts.len() {
            let dir = parts[..end].join("/");
            *dir_files.entry(dir).or_insert(0) += 1;
        }
    }
    for v in &result.violations {
        // Assign violation to common parent of dir_a and dir_b (matching HTML report)
        let a = strip_prefix(&v.dir_a, &common);
        let b = strip_prefix(&v.dir_b, &common);
        let a_parts: Vec<&str> = a.split('/').collect();
        let b_parts: Vec<&str> = b.split('/').collect();
        let mut common_len = 0;
        for (x, y) in a_parts.iter().zip(b_parts.iter()) {
            if x == y {
                common_len += 1;
            } else {
                break;
            }
        }
        let parent = if common_len > 0 {
            a_parts[..common_len].join("/")
        } else {
            String::new()
        };
        if !parent.is_empty() {
            *dir_severity.entry(parent).or_insert(0.0) += v.severity;
        }
    }

    let mut root = Node {
        name: "root".to_string(),
        value: None,
        score: None,
        children: Vec::new(),
    };

    for m in modules {
        let rel = strip_prefix(&m.path, &common);
        let parts: Vec<&str> = rel.split('/').collect();
        let mut current = &mut root;

        for (i, part) in parts.iter().enumerate() {
            let is_leaf = i == parts.len() - 1;
            let idx = current.children.iter().position(|c| c.name == *part);
            if let Some(idx) = idx {
                current = &mut current.children[idx];
            } else {
                // Compute score for this directory level
                let dir_path = parts[..=i].join("/");
                let node_score = if !is_leaf {
                    let files = *dir_files.get(&dir_path).unwrap_or(&1) as f64;
                    let sev = *dir_severity.get(&dir_path).unwrap_or(&0.0);
                    Some((100.0 * (1.0 - sev / files.max(1.0))).clamp(0.0, 100.0))
                } else {
                    None
                };

                current.children.push(Node {
                    name: part.to_string(),
                    value: if is_leaf { Some(1) } else { None },
                    score: node_score,
                    children: Vec::new(),
                });
                let len = current.children.len();
                current = &mut current.children[len - 1];
            }
        }
    }

    serde_json::to_value(root).unwrap_or(serde_json::json!({}))
}

fn build_sunburst_deps(
    modules: &[Module],
    dependencies: &[Dependency],
    id_to_path: &HashMap<&str, &str>,
) -> Vec<DepEdge> {
    let paths: Vec<&str> = modules.iter().map(|m| m.path.as_str()).collect();
    let common = find_common_prefix(&paths);
    let mut seen = std::collections::HashSet::new();
    let mut deps = Vec::new();

    for dep in dependencies {
        let from = id_to_path.get(dep.from_module_id.as_str());
        let to = id_to_path.get(dep.to_module_id.as_str());
        if let (Some(&f), Some(&t)) = (from, to) {
            let f_rel = strip_prefix(f, &common);
            let t_rel = strip_prefix(t, &common);
            let f_dir = f_rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let t_dir = t_rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            if f_dir != t_dir && seen.insert((f_rel.clone(), t_rel.clone())) {
                deps.push(DepEdge {
                    from: f_rel,
                    to: t_rel,
                });
            }
        }
    }
    deps
}

/// Strip the full common directory prefix (all shared components).
fn find_full_common_prefix(paths: &[&str]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let first: Vec<&str> = paths[0].split('/').collect();
    let mut len = first.len().saturating_sub(1); // exclude filename
    for path in &paths[1..] {
        let parts: Vec<&str> = path.split('/').collect();
        let mut common = 0;
        for (a, b) in first.iter().zip(parts.iter()) {
            if a == b {
                common += 1;
            } else {
                break;
            }
        }
        len = len.min(common);
    }
    first[..len].join("/")
}

fn find_common_prefix(paths: &[&str]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let first: Vec<&str> = paths[0].split('/').collect();
    let mut len = first.len().saturating_sub(1);
    for path in &paths[1..] {
        let parts: Vec<&str> = path.split('/').collect();
        let mut common = 0;
        for (a, b) in first.iter().zip(parts.iter()) {
            if a == b {
                common += 1;
            } else {
                break;
            }
        }
        len = len.min(common);
    }
    if len > 1 {
        len -= 1;
    }
    first[..len].join("/")
}

fn strip_prefix(path: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        return path.to_string();
    }
    let with_slash = format!("{}/", prefix);
    path.strip_prefix(&with_slash).unwrap_or(path).to_string()
}
