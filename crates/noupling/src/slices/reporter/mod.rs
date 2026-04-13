mod html;

use std::collections::BTreeMap;
use serde::Serialize;

use crate::core::Module;
use crate::slices::analyzer::{AuditResult, CouplingViolation};

pub use html::generate_html_report;

// ── Comprehensive JSON Report ──

#[derive(Serialize)]
pub struct JsonReport {
    pub snapshot_id: String,
    pub score: f64,
    pub total_modules: usize,
    pub critical_violations: usize,
    pub total_circular: usize,
    pub total_coupling: usize,
    pub circular_dependencies: BTreeMap<String, Vec<JsonCircularViolation>>,
    pub coupling_violations: Vec<JsonCouplingViolation>,
    pub directory_tree: Vec<JsonDirectory>,
}

#[derive(Serialize)]
pub struct JsonCircularViolation {
    pub severity: f64,
    pub cycle_order: usize,
    pub cycle_path: Vec<String>,
    pub cycle_short_path: Vec<String>,
    pub hop_files: Vec<JsonHopFile>,
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
    pub fn from_audit(
        modules: &[Module],
        result: &AuditResult,
        snapshot_id: &str,
    ) -> Self {
        let critical_violations = result
            .violations
            .iter()
            .filter(|v| v.severity >= 0.5)
            .count();

        let circular: Vec<&CouplingViolation> = result
            .violations
            .iter()
            .filter(|v| v.is_circular)
            .collect();

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
            let short_path: Vec<String> = v.cycle_path.iter().map(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(p)
                    .to_string()
            }).collect();

            let mut hop_files = Vec::new();
            for (i, dir) in v.cycle_path.iter().enumerate() {
                let dir_short = std::path::Path::new(dir)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(dir)
                    .to_string();
                if i < v.cycle_hop_files.len() {
                    let (from_file, to_file) = &v.cycle_hop_files[i];
                    hop_files.push(JsonHopFile {
                        from_dir: dir_short,
                        from_file: from_file.clone(),
                        to_file: to_file.clone(),
                    });
                } else if i == v.cycle_path.len() - 1 && !v.cycle_hop_files.is_empty() {
                    let (_, to_file) = &v.cycle_hop_files[v.cycle_hop_files.len() - 1];
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
                });
        }

        let coupling_violations: Vec<JsonCouplingViolation> = coupling
            .iter()
            .map(|v| JsonCouplingViolation {
                severity: v.severity,
                from_module: v.from_module.clone(),
                to_module: v.to_module.clone(),
                dir_a: v.dir_a.clone(),
                dir_b: v.dir_b.clone(),
                depth: v.depth,
            })
            .collect();

        // Build directory tree
        let directory_tree = build_json_dir_tree(modules, result);

        JsonReport {
            snapshot_id: snapshot_id.to_string(),
            score: result.score,
            total_modules: result.total_modules,
            critical_violations,
            total_circular: circular.len(),
            total_coupling: coupling.len(),
            circular_dependencies: circular_by_order,
            coupling_violations,
            directory_tree,
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
                let name = dir.file_name()
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
    sorted_paths.sort_by(|a, b| b.len().cmp(&a.len()));

    for path in &sorted_paths {
        let child_count: usize = {
            let dir = dirs.get(path).unwrap();
            let prefix = format!("{}/", path);
            dir_paths.iter()
                .filter(|p| p.starts_with(&prefix) && p.matches('/').count() == path.matches('/').count() + 1)
                .filter_map(|p| dirs.get(p).map(|d| d.module_count))
                .sum()
        };
        if let Some(dir) = dirs.get_mut(path) {
            dir.module_count += child_count;
        }
    }

    // Count violations per directory
    for v in &result.violations {
        // Find common parent for this violation
        let parent = if v.is_circular && !v.cycle_path.is_empty() {
            find_common_ancestor(&v.cycle_path)
        } else {
            common_parent(&v.dir_a, &v.dir_b)
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

fn find_common_ancestor(paths: &[String]) -> String {
    if paths.is_empty() { return String::new(); }
    let mut common = std::path::Path::new(&paths[0])
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .to_string_lossy()
        .to_string();
    for path in &paths[1..] {
        let p = std::path::Path::new(path)
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_string_lossy()
            .to_string();
        while !p.starts_with(&common) && !common.is_empty() {
            common = std::path::Path::new(&common)
                .parent()
                .map(|pp| pp.to_string_lossy().to_string())
                .unwrap_or_default();
        }
    }
    common
}

fn common_parent(a: &str, b: &str) -> String {
    let pa = std::path::Path::new(a);
    let pb = std::path::Path::new(b);
    if pa.parent() == pb.parent() {
        return pa.parent().unwrap_or(std::path::Path::new("")).to_string_lossy().to_string();
    }
    let mut current = pa.to_path_buf();
    loop {
        let s = current.to_string_lossy().to_string();
        if b.starts_with(&format!("{}/", s)) || b == s {
            return s;
        }
        match current.parent() {
            Some(p) if !p.as_os_str().is_empty() => current = p.to_path_buf(),
            _ => return String::new(),
        }
    }
}

pub fn format_text(result: &AuditResult) -> String {
    let mut output = String::new();

    output.push_str(&format!("Health Score: {:.1}/100\n", result.score));
    output.push_str(&format!("Total Modules: {}\n", result.total_modules));
    output.push_str(&format!("Violations: {}\n", result.violations.len()));

    if !result.violations.is_empty() {
        output.push('\n');
        for v in &result.violations {
            let label = if v.is_circular { " CIRCULAR" } else { "" };
            output.push_str(&format!(
                "  [{:.2}]{} {} -> {} (depth {})\n",
                v.severity, label, v.from_module, v.to_module, v.depth
            ));
            output.push_str(&format!(
                "         {} <> {}\n",
                v.dir_a, v.dir_b
            ));
        }
    }

    output
}

pub fn format_markdown(result: &AuditResult, snapshot_id: &str) -> String {
    let mut md = String::new();

    md.push_str("# noupling Audit Report\n\n");
    md.push_str(&format!("**Snapshot:** `{}`\n\n", snapshot_id));

    md.push_str("## Summary\n\n");
    md.push_str("| Metric | Value |\n");
    md.push_str("| :--- | :--- |\n");
    md.push_str(&format!("| Health Score | {:.1}/100 |\n", result.score));
    md.push_str(&format!("| Total Modules | {} |\n", result.total_modules));
    md.push_str(&format!("| Violations | {} |\n", result.violations.len()));

    let critical = result.violations.iter().filter(|v| v.severity >= 0.5).count();
    md.push_str(&format!("| Critical (severity >= 0.5) | {} |\n", critical));

    let circular = result.violations.iter().filter(|v| v.is_circular).count();
    if circular > 0 {
        md.push_str(&format!("| Structural Loops | {} |\n", circular));
    }

    if !result.violations.is_empty() {
        md.push_str("\n## Violations\n\n");
        md.push_str("| Severity | From | To | Depth | Type |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
        for v in &result.violations {
            let vtype = if v.is_circular { "Structural Loop" } else { "Coupling" };
            md.push_str(&format!(
                "| {:.2} | `{}` | `{}` | {} | {} |\n",
                v.severity, v.from_module, v.to_module, v.depth, vtype
            ));
        }
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_violation(from: &str, to: &str, severity: f64, depth: i32) -> CouplingViolation {
        CouplingViolation {
            dir_a: "dir_a".to_string(),
            dir_b: "dir_b".to_string(),
            from_module: from.to_string(),
            to_module: to.to_string(),
            depth,
            severity,
            is_circular: false,
            cycle_path: Vec::new(),
            cycle_hop_files: Vec::new(),
            cycle_order: 0,
        }
    }

    #[test]
    fn json_report_has_required_fields() {
        let modules = vec![];
        let result = AuditResult {
            violations: vec![make_violation("a.rs", "b.rs", 1.0, 0)],
            score: 50.0,
            total_modules: 2,
        };

        let report = JsonReport::from_audit(&modules, &result, "snap-1");
        let json = report.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["snapshot_id"], "snap-1");
        assert_eq!(parsed["score"], 50.0);
        assert_eq!(parsed["total_modules"], 2);
        assert_eq!(parsed["critical_violations"], 1);
        assert_eq!(parsed["total_coupling"], 1);
        assert_eq!(parsed["total_circular"], 0);
    }

    #[test]
    fn json_report_valid_json() {
        let modules = vec![];
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 5,
        };

        let report = JsonReport::from_audit(&modules, &result, "snap-2");
        let json = report.to_json().unwrap();
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn critical_violations_counts_high_severity() {
        let modules = vec![];
        let result = AuditResult {
            violations: vec![
                make_violation("a.rs", "b.rs", 1.0, 0),
                make_violation("c.rs", "d.rs", 0.5, 1),
                make_violation("e.rs", "f.rs", 0.25, 2),
            ],
            score: 42.0,
            total_modules: 6,
        };

        let report = JsonReport::from_audit(&modules, &result, "snap-3");
        assert_eq!(report.critical_violations, 2);
    }

    #[test]
    fn text_format_shows_score_and_violations() {
        let result = AuditResult {
            violations: vec![make_violation("scanner/mod.rs", "storage/mod.rs", 0.5, 1)],
            score: 75.0,
            total_modules: 4,
        };

        let text = format_text(&result);
        assert!(text.contains("Health Score: 75.0/100"));
        assert!(text.contains("Violations: 1"));
        assert!(text.contains("scanner/mod.rs"));
    }

    #[test]
    fn text_format_clean_when_no_violations() {
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 4,
        };

        let text = format_text(&result);
        assert!(text.contains("Health Score: 100.0/100"));
        assert!(text.contains("Violations: 0"));
    }

    #[test]
    fn markdown_has_heading_and_summary_table() {
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 5,
        };

        let md = format_markdown(&result, "snap-1");
        assert!(md.contains("# noupling Audit Report"));
        assert!(md.contains("| Health Score | 100.0/100 |"));
    }

    #[test]
    fn markdown_shows_structural_loop() {
        let mut v = make_violation("a.rs", "b.rs", 1.0, 0);
        v.is_circular = true;
        let result = AuditResult {
            violations: vec![v],
            score: 50.0,
            total_modules: 2,
        };

        let md = format_markdown(&result, "snap-3");
        assert!(md.contains("Structural Loop"));
    }
}
