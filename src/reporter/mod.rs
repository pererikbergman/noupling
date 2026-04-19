mod bundle;
mod dashboard;
mod graph;
mod html;
mod md;
mod strategy;

use serde::Serialize;
use std::collections::BTreeMap;

use crate::analyzer::{AuditResult, CouplingViolation};
use crate::core::Module;

pub use bundle::generate_bundle_report;
pub use dashboard::generate_dashboard;
pub use graph::{format_dot, format_mermaid};
pub use html::generate_html_report;
pub use md::generate_markdown_report;
pub use strategy::generate_strategy_report;

/// The version string used across all report outputs.
pub const VERSION: &str = concat!("noupling v", env!("CARGO_PKG_VERSION"));

// ── Comprehensive JSON Report ──

#[derive(Serialize)]
pub struct JsonReport {
    pub generator: String,
    pub snapshot_id: String,
    pub score: f64,
    pub total_modules: usize,
    pub total_xs: usize,
    pub max_depth: usize,
    pub suppressed_count: usize,
    pub violation_age: JsonViolationAge,
    pub critical_violations: usize,
    pub total_circular: usize,
    pub total_coupling: usize,
    pub circular_dependencies: BTreeMap<String, Vec<JsonCircularViolation>>,
    pub coupling_violations: Vec<JsonCouplingViolation>,
    pub hotspots: Vec<JsonHotspot>,
    pub directory_tree: Vec<JsonDirectory>,
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
            total_modules: result.total_modules,
            total_xs: result.total_xs,
            max_depth: result.max_depth,
            suppressed_count: result.suppressed_count,
            violation_age: JsonViolationAge {
                new_count: result.violation_age.new_count,
                recent_count: result.violation_age.recent_count,
                chronic_count: result.violation_age.chronic_count,
            },
            critical_violations,
            total_circular: circular.len(),
            total_coupling: coupling.len(),
            circular_dependencies: circular_by_order,
            coupling_violations,
            hotspots,
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
        "<noupling-report generator=\"{}\" snapshot=\"{}\" score=\"{:.2}\" totalModules=\"{}\" totalXs=\"{}\" maxDepth=\"{}\" suppressedCount=\"{}\" violationAgeNew=\"{}\" violationAgeRecent=\"{}\" violationAgeChronic=\"{}\" criticalViolations=\"{}\" totalCircular=\"{}\" totalCoupling=\"{}\">\n",
        xml_escape(VERSION), xml_escape(&report.snapshot_id), report.score, report.total_modules,
        report.total_xs, report.max_depth, report.suppressed_count,
        report.violation_age.new_count, report.violation_age.recent_count, report.violation_age.chronic_count,
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
                let wl_attr = cycle
                    .weakest_link
                    .as_ref()
                    .map(|wl| {
                        format!(
                            " weakestLink=\"{}\" breakCost=\"{}\"",
                            xml_escape(wl),
                            cycle.break_cost
                        )
                    })
                    .unwrap_or_default();
                xml.push_str(&format!(
                    "      <cycle order=\"{}\" severity=\"{:.2}\"{}>\n",
                    cycle.cycle_order, cycle.severity, wl_attr
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

            let effort = if v.break_cost > 0 {
                v.break_cost as i32 * 15
            } else {
                (v.cycle_order as i32) * 30
            };
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
    if result.total_xs > 0 {
        output.push_str(&format!(
            "Total XS: {} import{} to remove\n",
            result.total_xs,
            if result.total_xs == 1 { "" } else { "s" }
        ));
    }
    if result.suppressed_count > 0 {
        output.push_str(&format!(
            "Suppressed: {} import{} via noupling:ignore\n",
            result.suppressed_count,
            if result.suppressed_count == 1 {
                ""
            } else {
                "s"
            }
        ));
    }

    // Top Actions — what to do
    let top_actions = crate::analyzer::compute_top_actions(result, 5);
    if !top_actions.is_empty() {
        output.push_str("\nTop Actions:\n");
        for (i, action) in top_actions.iter().enumerate() {
            output.push_str(&format!(
                "  {}. {} [{}]\n",
                i + 1,
                action.title,
                action.category
            ));
            output.push_str(&format!("     {}\n", action.detail));
            output.push_str(&format!(
                "     \u{2192} {} (cost: {} import{})\n",
                action.action,
                action.cost,
                if action.cost == 1 { "" } else { "s" }
            ));
        }
    }

    if !result.violations.is_empty() {
        output.push('\n');
        for v in &result.violations {
            let label = if v.is_circular {
                " CIRCULAR".to_string()
            } else if v.weight > 1 {
                format!(" x{}", v.weight)
            } else {
                String::new()
            };
            output.push_str(&format!(
                "  [{:.2}]{} {} -> {} (depth {})\n",
                v.severity, label, v.from_module, v.to_module, v.depth
            ));
            output.push_str(&format!("         {} <> {}\n", v.dir_a, v.dir_b));
            if let Some(ref wl) = v.weakest_link {
                output.push_str(&format!("         Weakest link: {}\n", wl));
            }
        }
    }

    // Hotspots (top 10 most-imported modules)
    let top_hotspots: Vec<_> = result
        .hotspots
        .iter()
        .filter(|h| h.fan_in > 0)
        .take(10)
        .collect();
    if !top_hotspots.is_empty() {
        output.push_str("\nHotspots (most imported):\n");
        for h in &top_hotspots {
            output.push_str(&format!(
                "  [{} in, {} out] {}\n",
                h.fan_in, h.fan_out, h.path
            ));
        }
    }

    // Zone of pain: stable modules (low instability) with high fan-in
    let zone_of_pain: Vec<_> = result
        .hotspots
        .iter()
        .filter(|h| h.instability < 0.3 && h.fan_in >= 5)
        .take(10)
        .collect();
    if !zone_of_pain.is_empty() {
        output.push_str("\nZone of Pain (stable, high fan-in):\n");
        for h in &zone_of_pain {
            output.push_str(&format!(
                "  I={:.2} [{} in, {} out] {}\n",
                h.instability, h.fan_in, h.fan_out, h.path
            ));
        }
    }

    // Highest blast radius
    let mut by_blast: Vec<_> = result
        .hotspots
        .iter()
        .filter(|h| h.blast_radius > 0)
        .collect();
    by_blast.sort_by_key(|h| std::cmp::Reverse(h.blast_radius));
    let top_blast: Vec<_> = by_blast.into_iter().take(10).collect();
    if !top_blast.is_empty() {
        output.push_str("\nHighest Blast Radius:\n");
        for h in &top_blast {
            output.push_str(&format!(
                "  [{}] {} ({} in, {} out)\n",
                h.blast_radius, h.path, h.fan_in, h.fan_out
            ));
        }
    }

    // Rule violations
    if !result.rule_violations.is_empty() {
        output.push_str(&format!(
            "\nRule Violations ({}):\n",
            result.rule_violations.len()
        ));
        for rv in &result.rule_violations {
            output.push_str(&format!(
                "  {} -> {} (line {})\n    {}\n",
                rv.from_module, rv.to_module, rv.line_number, rv.message
            ));
        }
    }

    // Layer violations
    if !result.layer_violations.is_empty() {
        output.push_str(&format!(
            "\nLayer Violations ({}):\n",
            result.layer_violations.len()
        ));
        for lv in &result.layer_violations {
            output.push_str(&format!(
                "  {} ({}) -> {} ({}) (line {})\n",
                lv.from_module, lv.from_layer, lv.to_module, lv.to_layer, lv.line_number
            ));
        }
    }

    // Cohesion (low cohesion directories)
    let low_cohesion: Vec<_> = result
        .cohesion
        .iter()
        .filter(|c| c.cohesion < 0.1 && c.file_count >= 3)
        .take(10)
        .collect();
    if !low_cohesion.is_empty() {
        output.push_str("\nLow Cohesion:\n");
        for c in &low_cohesion {
            output.push_str(&format!(
                "  {:.2} {} ({} files, {} internal deps)\n",
                c.cohesion, c.dir, c.file_count, c.internal_deps
            ));
        }
    }

    // Module independence
    let low_independence: Vec<_> = result
        .independence
        .iter()
        .filter(|m| m.independence < 0.7)
        .take(10)
        .collect();
    if !low_independence.is_empty() {
        output.push_str("\nLow Independence:\n");
        for m in &low_independence {
            output.push_str(&format!(
                "  {:.0}% {} ({} files, {} internal, {} external)\n",
                m.independence * 100.0,
                m.dir,
                m.file_count,
                m.internal_deps,
                m.external_deps
            ));
        }
    }

    // Dependency depth
    if result.max_depth > 0 {
        output.push_str(&format!(
            "\nDependency Depth: {} (longest chain)\n",
            result.max_depth
        ));
        if !result.critical_path.is_empty() {
            output.push_str("  Critical path: ");
            for (i, p) in result.critical_path.iter().enumerate() {
                if i > 0 {
                    output.push_str(" -> ");
                }
                let short = std::path::Path::new(p)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(p);
                output.push_str(short);
            }
            output.push('\n');
        }
    }

    // Violation age summary
    let age = &result.violation_age;
    if age.new_count > 0 || age.recent_count > 0 || age.chronic_count > 0 {
        output.push_str(&format!(
            "\nViolation Age: {} new, {} recent, {} chronic\n",
            age.new_count, age.recent_count, age.chronic_count
        ));
    }

    output.push_str(&format!("\n{}\n", VERSION));

    output
}

/// PR/code-review report: tight Markdown summary suitable for posting as
/// a PR comment. Length-bounded regardless of project size.
///
/// `previous_score` and `previous_violation_count` come from the previous
/// snapshot (or baseline), and are used to compute deltas. Both are
/// optional — when absent, only current state is shown.
pub fn format_pr(
    result: &AuditResult,
    previous_score: Option<f64>,
    previous_violation_count: Option<usize>,
    new_violations: Option<usize>,
    resolved_violations: Option<usize>,
) -> String {
    let mut out = String::new();

    let score_emoji = if result.score >= 90.0 {
        "\u{2705}"
    } else if result.score >= 70.0 {
        "\u{26a0}\u{fe0f}"
    } else {
        "\u{274c}"
    };

    out.push_str("## Architecture Check\n\n");

    // Score line with optional delta
    let score_line = match previous_score {
        Some(prev) => {
            let delta = result.score - prev;
            let arrow = if delta > 0.05 {
                format!(" (+{:.1} since previous)", delta)
            } else if delta < -0.05 {
                format!(" ({:.1} since previous)", delta)
            } else {
                String::new()
            };
            format!(
                "**Score:** {:.1}/100{} {}\n",
                result.score, arrow, score_emoji
            )
        }
        None => format!("**Score:** {:.1}/100 {}\n", result.score, score_emoji),
    };
    out.push_str(&score_line);
    out.push('\n');

    // Summary table
    out.push_str("### Summary\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("| :--- | :--- |\n");
    out.push_str(&format!(
        "| Violations | {}{} |\n",
        result.violations.len(),
        previous_violation_count
            .map(|p| {
                let d = result.violations.len() as i64 - p as i64;
                if d > 0 {
                    format!(" (+{})", d)
                } else if d < 0 {
                    format!(" ({})", d)
                } else {
                    String::new()
                }
            })
            .unwrap_or_default()
    ));
    out.push_str(&format!("| Total XS | {} imports |\n", result.total_xs));
    if let Some(n) = new_violations {
        out.push_str(&format!("| New violations | {} |\n", n));
    }
    if let Some(r) = resolved_violations {
        out.push_str(&format!("| Resolved violations | {} |\n", r));
    }
    out.push('\n');

    // Top actions
    let actions = crate::analyzer::compute_top_actions(result, 3);
    if !actions.is_empty() {
        out.push_str("### Action items\n\n");
        for (i, a) in actions.iter().enumerate() {
            out.push_str(&format!(
                "{}. **{}** [{}]\n   - {}\n   - {} _(cost: {} import{})_\n\n",
                i + 1,
                a.title,
                a.category,
                a.detail,
                a.action,
                a.cost,
                if a.cost == 1 { "" } else { "s" }
            ));
        }
    } else {
        out.push_str("### Action items\n\nNo violations to fix \u{1f389}\n\n");
    }

    out.push_str(&format!("---\n_{}_\n", VERSION));
    out
}

/// Sprint planning briefing: Markdown report with top 10 refactoring
/// opportunities ranked by ROI, with effort estimates and projected score.
pub fn format_briefing(result: &AuditResult) -> String {
    let mut out = String::new();

    let date = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86400)
        .unwrap_or(0);
    let years = 1970 + date / 365;
    let day_of_year = date % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    out.push_str(&format!(
        "# Architecture Briefing — {:04}-{:02}-{:02}\n\n",
        years, month, day
    ));

    let actions = crate::analyzer::compute_top_actions(result, 10);

    // Projected score after fixing top 3
    let actionable_severity: f64 = actions
        .iter()
        .take(3)
        .filter(|a| a.category != "hotspot")
        .map(|a| a.impact)
        .sum();
    let projected_score = if result.total_modules > 0 {
        let projected_sum =
            (100.0 - result.score) * result.total_modules as f64 / 100.0 - actionable_severity;
        (100.0 * (1.0 - projected_sum.max(0.0) / result.total_modules as f64))
            .clamp(result.score, 100.0)
    } else {
        result.score
    };
    let delta = projected_score - result.score;

    out.push_str(&format!("**Current score:** {:.1}/100  \n", result.score));
    if delta > 0.1 {
        out.push_str(&format!(
            "**If you fix the top 3 below, projected score:** {:.1} (+{:.1})\n\n",
            projected_score, delta
        ));
    } else {
        out.push('\n');
    }

    // Summary metrics
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("| :--- | :--- |\n");
    out.push_str(&format!("| Total modules | {} |\n", result.total_modules));
    out.push_str(&format!(
        "| Active violations | {} |\n",
        result.violations.len()
    ));
    out.push_str(&format!(
        "| Total XS (imports to remove) | {} |\n",
        result.total_xs
    ));
    if result.max_depth > 0 {
        out.push_str(&format!(
            "| Max dependency depth | {} |\n",
            result.max_depth
        ));
    }
    if result.coupling_metrics_count > 0 {
        out.push_str(&format!(
            "| Sibling coupling pairs (informational) | {} |\n",
            result.coupling_metrics_count
        ));
    }
    out.push('\n');

    if actions.is_empty() {
        out.push_str("## No Actionable Items\n\n");
        out.push_str("Architecture is healthy. \u{1F389}\n\n");
        out.push_str(&format!("---\n_{}_\n", VERSION));
        return out;
    }

    out.push_str("## Top Refactoring Opportunities\n\n");
    out.push_str(
        "Ranked by ROI (impact / effort). Each item includes effort, impact, and approach.\n\n",
    );

    for (i, action) in actions.iter().enumerate() {
        let effort_label = effort_estimate(action.cost);
        let impact_label = match action.category.as_str() {
            "circular" => "Resolves a cycle",
            "layer" => "Removes a layer violation",
            "rule" => "Resolves a rule violation",
            "cross-module" => "Removes a cross-module violation",
            "hotspot" => "Reduces blast radius",
            _ => "Improves architecture",
        };

        out.push_str(&format!("### {}. {}\n\n", i + 1, action.title));
        out.push_str(&format!(
            "- **Effort:** {} import{} to remove ({})\n",
            action.cost,
            if action.cost == 1 { "" } else { "s" },
            effort_label
        ));
        out.push_str(&format!(
            "- **Impact:** {} (score impact: ~{:.1})\n",
            impact_label, action.impact
        ));
        out.push_str(&format!("- **Detail:** `{}`\n", action.detail));
        out.push_str(&format!("- **Approach:** {}\n", action.action));
        out.push_str(&format!("- **Category:** {}\n\n", action.category));
    }

    out.push_str(&format!("---\n_{}_\n", VERSION));
    out
}

fn effort_estimate(cost: usize) -> &'static str {
    match cost {
        0..=1 => "5 minutes",
        2..=5 => "1-2 hours",
        6..=20 => "half a day",
        21..=50 => "1-2 days",
        _ => "1+ week",
    }
}

pub fn format_monorepo_text(monorepo: &crate::analyzer::MonorepoResult) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "Overall Score: {:.1}/100\n",
        monorepo.overall_score
    ));
    output.push_str(&format!("Total Modules: {}\n\n", monorepo.total_modules));
    output.push_str(&format!(
        "{:<20} {:>8} {:>10} {:>12}\n",
        "MODULE", "SCORE", "MODULES", "VIOLATIONS"
    ));
    output.push_str(&format!("{}\n", "-".repeat(52)));

    for (name, result) in &monorepo.module_results {
        output.push_str(&format!(
            "{:<20} {:>7.1} {:>10} {:>12}\n",
            name,
            result.score,
            result.total_modules,
            result.violations.len(),
        ));
    }

    if !monorepo.cross_module_violations.is_empty() {
        output.push_str(&format!(
            "\nCross-Module Violations ({}):\n",
            monorepo.cross_module_violations.len()
        ));
        for v in &monorepo.cross_module_violations {
            output.push_str(&format!(
                "  {} -> {} (not in depends_on)\n",
                v.from_config, v.to_config
            ));
            output.push_str(&format!(
                "    {} -> {} (line {})\n",
                v.from_file, v.to_file, v.line_number
            ));
        }
    }

    output.push_str(&format!("\n{}\n", VERSION));
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
    use crate::analyzer::ViolationAgeSummary;

    fn make_violation(from: &str, to: &str, severity: f64, depth: i32) -> CouplingViolation {
        CouplingViolation {
            dir_a: "dir_a".to_string(),
            dir_b: "dir_b".to_string(),
            from_module: from.to_string(),
            to_module: to.to_string(),
            depth,
            severity,
            direction: crate::core::DependencyDirection::Sibling,
            is_circular: false,
            cycle_path: Vec::new(),
            cycle_hop_files: Vec::new(),
            cycle_order: 0,
            cycle_hop_counts: Vec::new(),
            weakest_link: None,
            break_cost: 0,
            line_number: 0,
            weight: 0,
        }
    }

    #[test]
    fn json_report_has_required_fields() {
        let modules = vec![];
        let result = AuditResult {
            violations: vec![make_violation("a.rs", "b.rs", 1.0, 0)],
            score: 50.0,
            total_modules: 2,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            coupling_metrics: Vec::new(),
            suppressed_count: 0,
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
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            coupling_metrics: Vec::new(),
            suppressed_count: 0,
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
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            coupling_metrics: Vec::new(),
            suppressed_count: 0,
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
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            coupling_metrics: Vec::new(),
            suppressed_count: 0,
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
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            coupling_metrics: Vec::new(),
            suppressed_count: 0,
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
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            coupling_metrics: Vec::new(),
            suppressed_count: 0,
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
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            coupling_metrics: Vec::new(),
            suppressed_count: 0,
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
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            coupling_metrics: Vec::new(),
            suppressed_count: 0,
        };

        let md = _format_markdown_single(&modules, &result, "snap-4");
        assert!(md.contains("## Directory Tree"));
    }
}
