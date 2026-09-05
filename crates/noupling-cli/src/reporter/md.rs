use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

use noupling_core::analyzer::{AuditResult, Issue, IssueDetail, IssueKind};
use noupling_core::core::Module;

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

    // Every Issue, filed under the deepest directory that contains its
    // subject. The root page lists all of them; directory pages only
    // their own.
    let issues = result.issues();
    let mut issues_per_dir: BTreeMap<String, Vec<&Issue>> = BTreeMap::new();
    for issue in &issues {
        let mut anchor = issue.anchor_dir();
        // Anchors above the report root (or outside the tree) file at root.
        if !dir_map.contains_key(&anchor) {
            anchor = root_path.clone();
        }
        issues_per_dir.entry(anchor).or_default().push(issue);
    }

    // Generate root README.md
    let root_md = render_dir_page(
        &root_path,
        &dir_map,
        &issues,
        &issues_per_dir,
        &report,
        snapshot_id,
        true,
        result.baseline.is_some(),
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
            &issues,
            &issues_per_dir,
            &report,
            snapshot_id,
            false,
            result.baseline.is_some(),
        );
        std::fs::write(page_dir.join("README.md"), md)?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_dir_page(
    dir_path: &str,
    dir_map: &BTreeMap<String, &super::JsonDirectory>,
    all_issues: &[Issue],
    issues_per_dir: &BTreeMap<String, Vec<&Issue>>,
    report: &JsonReport,
    snapshot_id: &str,
    is_root: bool,
    baseline_applied: bool,
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
    let violations = dir.violations_count;
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

    // Issues: root lists everything (with a kind-count summary), a
    // directory page only the Issues anchored under it.
    let page_issues: Vec<&Issue> = if is_root {
        all_issues.iter().collect()
    } else {
        issues_per_dir.get(dir_path).cloned().unwrap_or_default()
    };
    md.push_str(&render_issue_section(
        &page_issues,
        is_root,
        baseline_applied,
    ));

    md
}

/// The `## Issues` section: count line, kind-count table (root only), and
/// one card per Issue in `issues()` order. The match on `IssueDetail` is
/// exhaustive on purpose so a new kind fails to compile until handled.
fn render_issue_section(issues: &[&Issue], is_root: bool, baseline_applied: bool) -> String {
    let mut md = String::new();
    if issues.is_empty() {
        if is_root {
            md.push_str("## Issues (0)\n\nNo Issues found.\n\n");
        }
        return md;
    }
    let baselined = issues.iter().filter(|i| i.baselined).count();
    if baseline_applied {
        md.push_str(&format!(
            "## Issues ({}) — {} new, {} baselined\n\n",
            issues.len(),
            issues.len() - baselined,
            baselined
        ));
    } else {
        md.push_str(&format!("## Issues ({})\n\n", issues.len()));
    }

    if is_root {
        md.push_str("| Kind | Count |\n| :--- | :--- |\n");
        for kind in IssueKind::ALL {
            let n = issues.iter().filter(|i| i.kind() == kind).count();
            if n > 0 {
                md.push_str(&format!("| {} | {} |\n", kind, n));
            }
        }
        md.push('\n');
    }

    for issue in issues {
        md.push_str(&format!(
            "### [{}] {}: `{}`{}\n\n",
            issue.severity().name().to_uppercase(),
            issue.kind(),
            issue.subject(),
            if issue.baselined {
                " _(baselined)_"
            } else {
                ""
            }
        ));
        // Per-kind numbers the one-sentence reason does not carry.
        let extra: Option<String> = match &issue.detail {
            IssueDetail::CouplingViolation(v) => Some(format!(
                "`{}` <> `{}` — depth {}, line {}",
                v.dir_a, v.dir_b, v.depth, v.line_number
            )),
            IssueDetail::Cycle(v) => v
                .weakest_link
                .as_ref()
                .map(|wl| format!("Weakest link: {}", wl)),
            IssueDetail::RuleViolation(r) => Some(format!("line {}", r.line_number)),
            IssueDetail::LayerViolation(l) => Some(format!(
                "{} -> {} (line {})",
                l.from_layer, l.to_layer, l.line_number
            )),
            IssueDetail::GravityWell(g) => Some(format!(
                "RRI {:.0} across {} relationships",
                g.total_rri, g.relationship_count
            )),
            IssueDetail::RedFlag(f) => Some(format!("RRI {:.0}", f.rri)),
            IssueDetail::StabilityViolation(s) => Some(format!(
                "I={:.2} -> I={:.2}",
                s.from_instability, s.to_instability
            )),
            IssueDetail::ZoneFlag(d) => Some(format!(
                "D={:.2} (A={:.2}, I={:.2})",
                d.distance, d.abstractness, d.instability
            )),
            IssueDetail::LowCohesion(c) => Some(format!(
                "{} children, {} internal deps",
                c.n_children, c.internal_deps
            )),
        };
        if let Some(extra) = extra {
            md.push_str(&format!("{}\n\n", extra));
        }
        md.push_str(&format!("- **Reason:** {}\n", issue.reason()));
        md.push_str(&format!(
            "- **Recommendation:** {}\n",
            issue.recommendation()
        ));
        let impact = issue.score_impact();
        if impact > 0.0 {
            md.push_str(&format!("- **Score impact:** {:.1}\n\n", impact));
        } else {
            md.push_str("- **Score impact:** 0 (does not score)\n\n");
        }
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use noupling_core::analyzer::{
        AuditResultBuilder, CohesionMetrics, CouplingViolation, DependencyDirection, DirectoryKind,
        StabilityViolation,
    };
    use noupling_core::core::ModuleType;

    fn file(id: &str, path: &str) -> Module {
        Module {
            id: id.into(),
            snapshot_id: "snap".into(),
            parent_id: None,
            name: path.rsplit('/').next().unwrap().into(),
            path: path.into(),
            module_type: ModuleType::File,
            depth: path.matches('/').count() as i32,
        }
    }

    fn sibling(from: &str, to: &str, dir_a: &str, dir_b: &str) -> CouplingViolation {
        CouplingViolation {
            dir_a: dir_a.into(),
            dir_b: dir_b.into(),
            from_module: from.into(),
            to_module: to.into(),
            line_number: 2,
            depth: 2,
            weight: 1,
            severity: 0.33,
            direction: DependencyDirection::Sibling,
            rri: 4.0,
            is_circular: false,
            cycle_path: vec![],
            cycle_hop_files: vec![],
            cycle_order: 0,
            cycle_hop_counts: vec![],
            weakest_link: None,
            break_cost: 0,
            score_impact: 1.5,
        }
    }

    /// Root README lists every Issue with a kind-count summary; a directory
    /// page lists only the Issues anchored under it.
    #[test]
    fn root_lists_every_issue_and_directory_pages_only_their_own() {
        let modules = vec![
            file("a", "src/loose/x/x1.rs"),
            file("b", "src/loose/y/y1.rs"),
            file("c", "src/bag/a.rs"),
            file("d", "src/bag/b.rs"),
            file("e", "src/bag/c.rs"),
            file("f", "src/stable/s.rs"),
        ];
        let result = AuditResultBuilder::new()
            .with_total_modules(6)
            .with_violations(vec![sibling(
                "src/loose/x/x1.rs",
                "src/loose/y/y1.rs",
                "src/loose/x",
                "src/loose/y",
            )])
            .with_cohesion(vec![CohesionMetrics {
                dir: "src/bag".into(),
                kind: DirectoryKind::Package,
                n_children: 3,
                internal_deps: 0,
                cohesion: Some(0.0),
            }])
            .with_stability_violations(vec![StabilityViolation {
                from_dir: "src/stable".into(),
                to_dir: "src/loose".into(),
                from_instability: 0.2,
                to_instability: 0.8,
            }])
            .build();
        let out = tempfile::tempdir().unwrap();
        generate_markdown_report(&modules, &result, "snap-md", out.path()).unwrap();

        let root = std::fs::read_to_string(out.path().join("README.md")).unwrap();
        assert!(root.contains("## Issues (3)"), "{root}");
        assert!(root.contains("| Coupling Violation | 1 |"), "{root}");
        assert!(root.contains("| Stability Violation | 1 |"), "{root}");
        assert!(root.contains("| Low Cohesion | 1 |"), "{root}");
        assert!(
            root.contains(
                "### [HIGH] Coupling Violation: `src/loose/x/x1.rs -> src/loose/y/y1.rs`"
            ),
            "{root}"
        );
        assert!(
            root.contains("**Reason:** src/loose/x/x1.rs imports across sibling"),
            "{root}"
        );
        assert!(root.contains("**Recommendation:**"), "{root}");
        assert!(root.contains("**Score impact:** 1.5"), "{root}");
        // The old per-kind sections are gone from md.
        assert!(!root.contains("## Coupling Violations"), "{root}");

        let loose = std::fs::read_to_string(out.path().join("loose/README.md")).unwrap();
        assert!(loose.contains("## Issues (1)"), "{loose}");
        assert!(loose.contains("Coupling Violation:"), "{loose}");
        assert!(
            !loose.contains("Low Cohesion:"),
            "bag's Issue must not leak into loose: {loose}"
        );

        let bag = std::fs::read_to_string(out.path().join("bag/README.md")).unwrap();
        assert!(bag.contains("### [LOW] Low Cohesion: `src/bag`"), "{bag}");
        assert!(!bag.contains("Coupling Violation:"), "{bag}");
    }

    #[test]
    fn baselined_issues_are_marked() {
        use noupling_core::baseline::Baseline;
        let modules = vec![file("a", "src/x/a.rs"), file("b", "src/y/b.rs")];
        let mut result = AuditResultBuilder::new()
            .with_total_modules(2)
            .with_violations(vec![sibling("src/x/a.rs", "src/y/b.rs", "src/x", "src/y")])
            .build();
        result.apply_baseline(&Baseline {
            fingerprints: ["coupling_violation:src/x -> src/y".to_string()]
                .into_iter()
                .collect(),
            legacy_format: false,
        });
        let out = tempfile::tempdir().unwrap();
        generate_markdown_report(&modules, &result, "snap-md", out.path()).unwrap();
        let root = std::fs::read_to_string(out.path().join("README.md")).unwrap();
        assert!(
            root.contains("## Issues (1) — 0 new, 1 baselined"),
            "{root}"
        );
        assert!(root.contains("_(baselined)_"), "{root}");
    }
}
