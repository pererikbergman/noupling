use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

use crate::analyzer::{AuditResult, CouplingViolation};
use crate::core::Module;

use super::JsonReport;

/// Generate multi-file Markdown report mirroring the HTML structure.
pub fn generate_markdown_report(
    modules: &[Module],
    result: &AuditResult,
    snapshot_id: &str,
    output_dir: &Path,
) -> Result<()> {
    let report = JsonReport::from_audit(modules, result, snapshot_id);

    std::fs::create_dir_all(output_dir)?;

    // Build a lookup: dir path -> (children dirs, files, violations, circular)
    let dir_map: BTreeMap<String, &super::JsonDirectory> = report
        .directory_tree
        .iter()
        .map(|d| (d.path.clone(), d))
        .collect();

    // Find root directory (shortest path)
    let root_path = report
        .directory_tree
        .iter()
        .min_by_key(|d| d.path.len())
        .map(|d| d.path.clone())
        .unwrap_or_default();

    // Collect violations per directory (same logic as HTML)
    let mut violations_per_dir: BTreeMap<String, Vec<&CouplingViolation>> = BTreeMap::new();
    for v in &result.violations {
        let parent = if v.is_circular && !v.cycle_path.is_empty() {
            find_common_ancestor(&v.cycle_path)
        } else {
            common_parent(&v.dir_a, &v.dir_b)
        };
        violations_per_dir.entry(parent).or_default().push(v);
    }

    // Generate root README.md
    let root_md = render_dir_page(
        &root_path,
        &dir_map,
        &violations_per_dir,
        &report,
        snapshot_id,
        true,
    );
    std::fs::write(output_dir.join("README.md"), root_md)?;

    // Generate a page for each subdirectory
    for dir in &report.directory_tree {
        if dir.path == root_path {
            continue;
        }
        let rel = dir
            .path
            .strip_prefix(&format!("{}/", root_path))
            .unwrap_or(&dir.path);
        let page_dir = output_dir.join(rel);
        std::fs::create_dir_all(&page_dir)?;
        let md = render_dir_page(
            &dir.path,
            &dir_map,
            &violations_per_dir,
            &report,
            snapshot_id,
            false,
        );
        std::fs::write(page_dir.join("README.md"), md)?;
    }

    Ok(())
}

fn render_dir_page(
    dir_path: &str,
    dir_map: &BTreeMap<String, &super::JsonDirectory>,
    violations_per_dir: &BTreeMap<String, Vec<&CouplingViolation>>,
    report: &JsonReport,
    snapshot_id: &str,
    is_root: bool,
) -> String {
    let dir = match dir_map.get(dir_path) {
        Some(d) => d,
        None => return "# Not Found\n".to_string(),
    };

    let mut md = String::new();

    // Title and breadcrumbs
    if is_root {
        md.push_str("# noupling Audit Report\n\n");
        md.push_str(&format!("**Snapshot:** `{}`\n\n", snapshot_id));
    } else {
        md.push_str(&format!("# {}\n\n", dir.name));
        md.push_str(&format!("`{}`\n\n", dir.path));
        md.push_str("[< Back to parent](../README.md)\n\n");
    }

    // Summary
    let violations = violations_per_dir
        .get(dir_path)
        .map(|v| v.len())
        .unwrap_or(0);
    md.push_str("## Summary\n\n");
    md.push_str("| Metric | Value |\n");
    md.push_str("| :--- | :--- |\n");
    if is_root {
        md.push_str(&format!("| Health Score | {:.1}/100 |\n", report.score));
        if report.tri > 0.0 {
            md.push_str(&format!("| Total Risk Index (TRI) | {:.1} |\n", report.tri));
        }
    }
    md.push_str(&format!("| Modules | {} |\n", dir.module_count));
    md.push_str(&format!("| Violations | {} |\n", violations));
    if is_root && report.total_xs > 0 {
        md.push_str(&format!(
            "| Total XS | {} import{} to remove |\n",
            report.total_xs,
            if report.total_xs == 1 { "" } else { "s" }
        ));
    }
    if is_root && report.max_depth > 0 {
        md.push_str(&format!(
            "| Max Dependency Depth | {} |\n",
            report.max_depth
        ));
    }
    if is_root && report.suppressed_count > 0 {
        md.push_str(&format!("| Suppressed | {} |\n", report.suppressed_count));
    }
    md.push('\n');

    if is_root {
        md.push_str("### Metrics Guide\n\n");
        md.push_str("| Metric | Description |\n");
        md.push_str("| :--- | :--- |\n");
        md.push_str("| **Health Score** | Overall codebase health (0-100). `100 × (1 - TRI / (modules × max_weight))` |\n");
        md.push_str(
            "| **TRI** | Total Risk Index — sum of all violation RRIs. Lower is better |\n",
        );
        md.push_str("| **RRI** | Relationship Risk Index — per-violation risk. `direction_weight × imports` |\n");
        md.push_str("| **Severity** | Legacy metric based on depth. Being replaced by RRI |\n\n");
        md.push_str("**Direction types:** ↓ Downward (weight 2) · ↔ Sibling (weight 4) · ↑ Upward (weight 6) · ↻ Circular (weight 10)\n\n");
    }

    // Contents: child directories
    if !dir.children.is_empty() || !dir.files.is_empty() {
        md.push_str("## Contents\n\n");
        md.push_str("| Name | Modules | Violations |\n");
        md.push_str("| :--- | :--- | :--- |\n");

        for child_name in &dir.children {
            // Find the child dir entry
            let child_path = format!("{}/{}", dir_path, child_name);
            let child_dir = dir_map.get(&child_path);
            let child_modules = child_dir.map(|d| d.module_count).unwrap_or(0);
            let child_violations = child_dir.map(|d| d.violations_count).unwrap_or(0);
            let warning = if child_dir.map(|d| d.has_violations).unwrap_or(false) {
                " !"
            } else {
                ""
            };
            md.push_str(&format!(
                "| [{}]({}/README.md){} | {} | {} |\n",
                child_name, child_name, warning, child_modules, child_violations,
            ));
        }

        for file in &dir.files {
            md.push_str(&format!("| {} | 1 | - |\n", file));
        }
        md.push('\n');
    }

    // Violations at this level
    if let Some(violations) = violations_per_dir.get(dir_path) {
        let circular: Vec<&&CouplingViolation> =
            violations.iter().filter(|v| v.is_circular).collect();
        let coupling: Vec<&&CouplingViolation> =
            violations.iter().filter(|v| !v.is_circular).collect();

        // Circular grouped by order
        if !circular.is_empty() {
            let mut by_order: BTreeMap<usize, Vec<&&CouplingViolation>> = BTreeMap::new();
            for v in &circular {
                by_order.entry(v.cycle_order).or_default().push(v);
            }

            md.push_str("## Circular Dependencies\n\n");
            md.push_str("Modules that depend on each other in a loop (weight 10). Break the weakest link to resolve.\n");
            for (order, cycles) in &by_order {
                let label = match order {
                    2 => "Mutual Dependencies (Order 2)".to_string(),
                    3 => "Triangular Cycles (Order 3)".to_string(),
                    n => format!("Cycles of Order {}", n),
                };
                md.push_str(&format!("\n### {} ({} found)\n\n", label, cycles.len()));

                for (idx, v) in cycles.iter().enumerate() {
                    // Short path
                    let short: Vec<String> = v
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

                    let mut display_parts = Vec::new();
                    for (i, name) in short.iter().enumerate() {
                        if i < v.cycle_hop_files.len() {
                            let (from_file, _, _) = &v.cycle_hop_files[i];
                            let file_short = std::path::Path::new(from_file)
                                .file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or(from_file);
                            display_parts.push(format!("**{}** ({})", name, file_short));
                        } else if i == v.cycle_path.len() - 1 && !v.cycle_hop_files.is_empty() {
                            let (_, to_file, _) = &v.cycle_hop_files[v.cycle_hop_files.len() - 1];
                            let file_short = std::path::Path::new(to_file)
                                .file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or(to_file);
                            display_parts.push(format!("**{}** ({})", name, file_short));
                        } else {
                            display_parts.push(format!("**{}**", name));
                        }
                    }
                    md.push_str(&format!(
                        "**Cycle {}** (severity {:.2}): {}\n\n",
                        idx + 1,
                        v.severity,
                        display_parts.join(" -> ")
                    ));

                    // Full paths
                    md.push_str("<details><summary>Full paths</summary>\n\n");
                    for (i, dir_p) in v.cycle_path.iter().enumerate() {
                        if i < v.cycle_hop_files.len() {
                            let (from_file, _, _) = &v.cycle_hop_files[i];
                            let dir_short = std::path::Path::new(dir_p)
                                .file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or(dir_p);
                            md.push_str(&format!("- **{}**: `{}`\n", dir_short, from_file));
                        }
                    }
                    md.push_str("\n</details>\n\n");

                    if let Some(ref wl) = v.weakest_link {
                        md.push_str(&format!(
                            "> **Weakest link:** {} (break cost: {} import{})\n\n",
                            wl,
                            v.break_cost,
                            if v.break_cost == 1 { "" } else { "s" }
                        ));
                    }
                }
            }
        }

        // Coupling violations
        if !coupling.is_empty() {
            md.push_str("## Coupling Violations\n\n");
            md.push_str("Sibling directories importing each other. **RRI** = direction weight × number of imports.\n\n");
            md.push_str("| Severity | RRI | Dir | From | To |\n");
            md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
            for v in &coupling {
                let from_short = std::path::Path::new(&v.from_module)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(&v.from_module);
                let to_short = std::path::Path::new(&v.to_module)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(&v.to_module);
                let dir_symbol = match v.direction {
                    crate::core::DependencyDirection::Downward => "↓",
                    crate::core::DependencyDirection::Sibling => "↔",
                    crate::core::DependencyDirection::Upward => "↑",
                    crate::core::DependencyDirection::External => "↗",
                    crate::core::DependencyDirection::Transitive => "⇝",
                    crate::core::DependencyDirection::Circular => "↻",
                };
                md.push_str(&format!(
                    "| {:.2} | {:.0} | {} | `{}` | `{}` |\n",
                    v.severity, v.rri, dir_symbol, from_short, to_short
                ));
            }
            md.push('\n');
        }
    }

    md
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
