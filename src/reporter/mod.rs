mod html;
mod md;

use serde::Serialize;
use std::collections::BTreeMap;

use crate::core::Module;
use crate::analyzer::{AuditResult, CouplingViolation};

pub use html::generate_html_report;
pub use md::generate_markdown_report;

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
    if paths.is_empty() {
        return String::new();
    }
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
        return pa
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_string_lossy()
            .to_string();
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

pub fn format_xml(modules: &[Module], result: &AuditResult, snapshot_id: &str) -> String {
    let report = JsonReport::from_audit(modules, result, snapshot_id);
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<noupling-report snapshot=\"{}\" score=\"{:.2}\" totalModules=\"{}\" criticalViolations=\"{}\" totalCircular=\"{}\" totalCoupling=\"{}\">\n",
        xml_escape(&report.snapshot_id), report.score, report.total_modules,
        report.critical_violations, report.total_circular, report.total_coupling,
    ));

    // Circular dependencies
    if !report.circular_dependencies.is_empty() {
        xml.push_str("  <circular-dependencies>\n");
        for (label, cycles) in &report.circular_dependencies {
            xml.push_str(&format!(
                "    <group label=\"{}\" count=\"{}\">\n",
                xml_escape(label),
                cycles.len()
            ));
            for cycle in cycles {
                xml.push_str(&format!(
                    "      <cycle order=\"{}\" severity=\"{:.2}\">\n",
                    cycle.cycle_order, cycle.severity
                ));
                xml.push_str("        <path>\n");
                for dir in &cycle.cycle_path {
                    xml.push_str(&format!("          <dir>{}</dir>\n", xml_escape(dir)));
                }
                xml.push_str("        </path>\n");
                xml.push_str("        <short-path>\n");
                for dir in &cycle.cycle_short_path {
                    xml.push_str(&format!("          <dir>{}</dir>\n", xml_escape(dir)));
                }
                xml.push_str("        </short-path>\n");
                xml.push_str("        <hops>\n");
                for hop in &cycle.hop_files {
                    xml.push_str(&format!(
                        "          <hop fromDir=\"{}\" fromFile=\"{}\" toFile=\"{}\"/>\n",
                        xml_escape(&hop.from_dir),
                        xml_escape(&hop.from_file),
                        xml_escape(&hop.to_file),
                    ));
                }
                xml.push_str("        </hops>\n");
                xml.push_str("      </cycle>\n");
            }
            xml.push_str("    </group>\n");
        }
        xml.push_str("  </circular-dependencies>\n");
    }

    // Coupling violations
    if !report.coupling_violations.is_empty() {
        xml.push_str("  <coupling-violations>\n");
        for v in &report.coupling_violations {
            xml.push_str(&format!(
                "    <violation severity=\"{:.2}\" depth=\"{}\" fromModule=\"{}\" toModule=\"{}\" dirA=\"{}\" dirB=\"{}\"/>\n",
                v.severity, v.depth, xml_escape(&v.from_module), xml_escape(&v.to_module),
                xml_escape(&v.dir_a), xml_escape(&v.dir_b),
            ));
        }
        xml.push_str("  </coupling-violations>\n");
    }

    // Directory tree
    xml.push_str("  <directory-tree>\n");
    for dir in &report.directory_tree {
        xml.push_str(&format!(
            "    <directory path=\"{}\" name=\"{}\" modules=\"{}\" score=\"{:.2}\" violations=\"{}\" circular=\"{}\" hasViolations=\"{}\">\n",
            xml_escape(&dir.path), xml_escape(&dir.name), dir.module_count,
            dir.score, dir.violations_count, dir.circular_count, dir.has_violations,
        ));
        for child in &dir.children {
            xml.push_str(&format!("      <child>{}</child>\n", xml_escape(child)));
        }
        for file in &dir.files {
            xml.push_str(&format!("      <file>{}</file>\n", xml_escape(file)));
        }
        xml.push_str("    </directory>\n");
    }
    xml.push_str("  </directory-tree>\n");

    xml.push_str("</noupling-report>\n");
    xml
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn format_sonar(result: &AuditResult) -> String {
    let mut issues = Vec::new();

    for v in &result.violations {
        if v.is_circular {
            let (file_path, first_line) = if !v.cycle_hop_files.is_empty() {
                (v.cycle_hop_files[0].0.clone(), v.cycle_hop_files[0].2)
            } else {
                (v.from_module.clone(), 1)
            };

            let short_dirs: Vec<String> = v
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
            let cycle_desc = short_dirs.join(" -> ");

            let mut secondary = Vec::new();
            for (i, (from_file, _to_file, line)) in v.cycle_hop_files.iter().enumerate() {
                if i == 0 {
                    continue;
                }
                let dir_name = if i < v.cycle_path.len() {
                    std::path::Path::new(&v.cycle_path[i])
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("")
                } else {
                    ""
                };
                secondary.push(serde_json::json!({
                    "message": format!("Part of circular dependency chain ({})", dir_name),
                    "filePath": from_file,
                    "textRange": { "startLine": line, "endLine": line }
                }));
            }

            let effort = (v.cycle_order as i32) * 30;
            let mut issue = serde_json::json!({
                "engineId": "noupling",
                "ruleId": "noupling:circular-dependency",
                "severity": "CRITICAL",
                "type": "CODE_SMELL",
                "effortMinutes": effort,
                "primaryLocation": {
                    "message": format!("Circular dependency: {}", cycle_desc),
                    "filePath": file_path,
                    "textRange": { "startLine": first_line, "endLine": first_line }
                }
            });
            if !secondary.is_empty() {
                issue["secondaryLocations"] = serde_json::json!(secondary);
            }
            issues.push(issue);
        } else {
            let (sonar_severity, effort) = if v.severity >= 0.5 {
                ("CRITICAL", 20)
            } else if v.severity >= 0.2 {
                ("MAJOR", 10)
            } else {
                ("MINOR", 5)
            };

            let dir_a_short = std::path::Path::new(&v.dir_a)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.dir_a);
            let dir_b_short = std::path::Path::new(&v.dir_b)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.dir_b);

            issues.push(serde_json::json!({
                "engineId": "noupling",
                "ruleId": "noupling:coupling",
                "severity": sonar_severity,
                "type": "CODE_SMELL",
                "effortMinutes": effort,
                "primaryLocation": {
                    "message": format!("Coupling violation: {} depends on {} (severity {:.2})", dir_a_short, dir_b_short, v.severity),
                    "filePath": v.from_module,
                    "textRange": { "startLine": v.line_number, "endLine": v.line_number }
                },
                "secondaryLocations": [{
                    "message": format!("Coupled target in {}", dir_b_short),
                    "filePath": v.to_module,
                    "textRange": { "startLine": v.line_number, "endLine": v.line_number }
                }]
            }));
        }
    }

    let report = serde_json::json!({
        "issues": issues,
    });

    serde_json::to_string_pretty(&report).unwrap_or_default()
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
            output.push_str(&format!("         {} <> {}\n", v.dir_a, v.dir_b));
        }
    }

    output
}

// Single-file markdown kept for backward compat in tests
fn _format_markdown_single(modules: &[Module], result: &AuditResult, snapshot_id: &str) -> String {
    let report = JsonReport::from_audit(modules, result, snapshot_id);
    let mut md = String::new();

    // Header
    md.push_str("# noupling Audit Report\n\n");
    md.push_str(&format!("**Snapshot:** `{}`\n\n", snapshot_id));

    // Summary
    md.push_str("## Summary\n\n");
    md.push_str("| Metric | Value |\n");
    md.push_str("| :--- | :--- |\n");
    md.push_str(&format!("| Health Score | {:.1}/100 |\n", report.score));
    md.push_str(&format!("| Total Modules | {} |\n", report.total_modules));
    md.push_str(&format!(
        "| Critical Violations | {} |\n",
        report.critical_violations
    ));
    md.push_str(&format!(
        "| Circular Dependencies | {} |\n",
        report.total_circular
    ));
    md.push_str(&format!(
        "| Coupling Violations | {} |\n",
        report.total_coupling
    ));

    // Circular dependencies grouped by order
    if !report.circular_dependencies.is_empty() {
        md.push_str("\n## Circular Dependencies\n");
        for (label, cycles) in &report.circular_dependencies {
            md.push_str(&format!("\n### {} ({} found)\n\n", label, cycles.len()));
            for (idx, cycle) in cycles.iter().enumerate() {
                // Short cycle path
                let short = cycle.cycle_short_path.join(" -> ");
                md.push_str(&format!(
                    "**Cycle {}** (severity: {:.2}): `{}`\n\n",
                    idx + 1,
                    cycle.severity,
                    short
                ));

                // Hop details table
                md.push_str("| Directory | File | Target |\n");
                md.push_str("| :--- | :--- | :--- |\n");
                for hop in &cycle.hop_files {
                    let from_short = std::path::Path::new(&hop.from_file)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or(&hop.from_file);
                    let to_short = if hop.to_file.is_empty() {
                        "-".to_string()
                    } else {
                        std::path::Path::new(&hop.to_file)
                            .file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or(&hop.to_file)
                            .to_string()
                    };
                    md.push_str(&format!(
                        "| {} | `{}` | `{}` |\n",
                        hop.from_dir, from_short, to_short
                    ));
                }
                md.push('\n');

                // Full paths
                md.push_str("<details><summary>Full paths</summary>\n\n");
                for hop in &cycle.hop_files {
                    if !hop.from_file.is_empty() {
                        md.push_str(&format!("- **{}**: `{}`\n", hop.from_dir, hop.from_file));
                    }
                }
                md.push_str("\n</details>\n\n");
            }
        }
    }

    // Coupling violations
    if !report.coupling_violations.is_empty() {
        md.push_str("## Coupling Violations\n\n");
        md.push_str("| Severity | From | To | Dir A | Dir B | Depth |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");
        for v in &report.coupling_violations {
            let from_short = std::path::Path::new(&v.from_module)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.from_module);
            let to_short = std::path::Path::new(&v.to_module)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.to_module);
            let dir_a_short = std::path::Path::new(&v.dir_a)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.dir_a);
            let dir_b_short = std::path::Path::new(&v.dir_b)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.dir_b);
            md.push_str(&format!(
                "| {:.2} | `{}` | `{}` | {} | {} | {} |\n",
                v.severity, from_short, to_short, dir_a_short, dir_b_short, v.depth
            ));
        }
        md.push('\n');
    }

    // Directory tree
    md.push_str("## Directory Tree\n\n");
    md.push_str("| Path | Modules | Score | Violations | Circular |\n");
    md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
    for dir in &report.directory_tree {
        let warning = if dir.has_violations { " !" } else { "" };
        md.push_str(&format!(
            "| `{}`{} | {} | {:.1} | {} | {} |\n",
            dir.path,
            warning,
            dir.module_count,
            dir.score,
            dir.violations_count,
            dir.circular_count,
        ));
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
            line_number: 0,
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
        let modules = vec![];
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 5,
        };

        let md = _format_markdown_single(&modules, &result, "snap-1");
        assert!(md.contains("# noupling Audit Report"));
        assert!(md.contains("| Health Score | 100.0/100 |"));
    }

    #[test]
    fn markdown_shows_circular_section() {
        let modules = vec![];
        let mut v = make_violation("a.rs", "b.rs", 1.0, 0);
        v.is_circular = true;
        v.cycle_order = 2;
        v.cycle_path = vec![
            "dir_a".to_string(),
            "dir_b".to_string(),
            "dir_a".to_string(),
        ];
        let result = AuditResult {
            violations: vec![v],
            score: 50.0,
            total_modules: 2,
        };

        let md = _format_markdown_single(&modules, &result, "snap-3");
        assert!(md.contains("## Circular Dependencies"));
        assert!(md.contains("Mutual Dependencies (Order 2)"));
    }

    #[test]
    fn markdown_has_directory_tree() {
        let modules = vec![];
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 3,
        };

        let md = _format_markdown_single(&modules, &result, "snap-4");
        assert!(md.contains("## Directory Tree"));
    }
}
