use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

use noupling_core::analyzer::{common_parent_dir, AuditResult, Issue, IssueDetail, IssueKind};
use noupling_core::core::Module;
use noupling_core::settings::Settings;

/// A node in the directory tree used for HTML navigation.
#[derive(Debug)]
struct DirNode {
    name: String,
    #[allow(dead_code)]
    path: String,
    children_dirs: Vec<String>,
    files: Vec<String>,
    violations_here: Vec<ViolationInfo>,
    coupling_metrics_here: Vec<ViolationInfo>,
    has_deep_violations: bool,
    score: f64,
    module_count: usize,
}

/// A violation (or informational coupling metric) filed under a
/// directory. Since #346 the Issue cards carry the detail; this keeps
/// only what the directory tables and the Coupling Metrics section read.
#[derive(Debug, Clone)]
struct ViolationInfo {
    from_module: String,
    to_module: String,
    severity: f64,
}

struct ReportData {
    dirs: BTreeMap<String, DirNode>,
    root_path: String,
    snapshot_id: String,
    total_score: f64,
    total_modules: usize,
    total_violations: usize,
    total_tri: f64,
    total_xs: usize,
    score_green: f64,
    score_yellow: f64,
    abstractness: Vec<noupling_core::analyzer::AbstractnessMetric>,
    instability: Vec<noupling_core::analyzer::InstabilityMetric>,
    distance: Vec<noupling_core::analyzer::DistanceMetric>,
    /// Every Issue in `issues()` order; the root page lists them all.
    issues: Vec<Issue>,
    /// Indices into `issues`, keyed by the directory each Issue is anchored
    /// under (`Issue::anchor_dir`, falling back to the root).
    issues_per_dir: BTreeMap<String, Vec<usize>>,
    baseline_applied: bool,
}

/// Generate static HTML report files in the given output directory.
pub fn generate_html_report(
    modules: &[Module],
    result: &AuditResult,
    snapshot_id: &str,
    output_dir: &Path,
    settings: &Settings,
) -> Result<()> {
    let data = build_report_data(modules, result, snapshot_id, settings);

    super::clear_generated_pages(output_dir, "index.html")?;
    std::fs::create_dir_all(output_dir)?;

    // Generate root index.html
    let root_html = render_page(&data, &data.root_path);
    std::fs::write(output_dir.join("index.html"), root_html)?;

    // Generate a page for each directory
    for dir_path in data.dirs.keys() {
        if dir_path == &data.root_path {
            continue;
        }
        let rel = dir_path
            .strip_prefix(&format!("{}/", data.root_path))
            .unwrap_or(dir_path);
        let page_dir = output_dir.join(rel);
        std::fs::create_dir_all(&page_dir)?;
        let html = render_page(&data, dir_path);
        std::fs::write(page_dir.join("index.html"), html)?;
    }

    Ok(())
}

fn build_report_data(
    modules: &[Module],
    result: &AuditResult,
    snapshot_id: &str,
    settings: &Settings,
) -> ReportData {
    // Find common root prefix from all module paths
    let root_path = find_common_root(modules);

    // Build directory set from module paths
    let mut dirs: BTreeMap<String, DirNode> = BTreeMap::new();

    // Collect all directories
    for module in modules {
        let path = std::path::Path::new(&module.path);
        let mut current = path.parent();
        while let Some(dir) = current {
            let dir_str = dir.to_string_lossy().to_string();
            if dir_str.is_empty() || dir_str.len() < root_path.len() {
                break;
            }
            if !dirs.contains_key(&dir_str) {
                let name = dir
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| dir_str.clone());
                dirs.insert(
                    dir_str.clone(),
                    DirNode {
                        name,
                        path: dir_str.clone(),
                        children_dirs: Vec::new(),
                        files: Vec::new(),
                        violations_here: Vec::new(),
                        coupling_metrics_here: Vec::new(),
                        has_deep_violations: false,
                        score: 100.0,
                        module_count: 0,
                    },
                );
            }
            current = dir.parent();
        }
    }

    // Ensure root exists
    if !dirs.contains_key(&root_path) {
        let name = std::path::Path::new(&root_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| root_path.clone());
        dirs.insert(
            root_path.clone(),
            DirNode {
                name,
                path: root_path.clone(),
                children_dirs: Vec::new(),
                files: Vec::new(),
                violations_here: Vec::new(),
                coupling_metrics_here: Vec::new(),
                has_deep_violations: false,
                score: 100.0,
                module_count: 0,
            },
        );
    }

    // Assign files to their parent directory
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

    // Build parent-child directory relationships
    let dir_paths: Vec<String> = dirs.keys().cloned().collect();
    for dir_path in &dir_paths {
        let parent = std::path::Path::new(dir_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if dirs.contains_key(&parent) && &parent != dir_path {
            let name = dirs.get(dir_path).unwrap().name.clone();
            if let Some(parent_dir) = dirs.get_mut(&parent) {
                if !parent_dir.children_dirs.contains(dir_path) {
                    parent_dir.children_dirs.push(dir_path.clone());
                }
            }
            let _ = name; // used above
        }
    }

    // Sort children
    for dir in dirs.values_mut() {
        dir.children_dirs.sort();
        dir.files.sort();
    }

    // Assign violations to directories
    // Ring hops are folded into their Cycle, as issues() does (#358).
    for violation in result.issue_violations() {
        // For circular violations, find the common ancestor of ALL dirs in the cycle
        // For coupling violations, find the parent where dir_a and dir_b are siblings
        // Same anchor rule as the Issue cards on the page (Issue::anchor_dir),
        // so a violation is filed under the directory whose page lists its
        // Issue. Counts still come from raw violations (a ring's hop edges
        // count separately; issues() folds them into one Cycle, see #358).
        let parent = if violation.is_circular && !violation.cycle_path.is_empty() {
            let members: Vec<&str> = violation.cycle_path.iter().map(String::as_str).collect();
            common_parent_dir(&members)
        } else {
            common_parent_dir(&[&violation.dir_a, &violation.dir_b])
        };

        let info = ViolationInfo {
            from_module: violation.from_module.clone(),
            to_module: violation.to_module.clone(),
            severity: violation.severity,
        };

        if let Some(dir) = dirs.get_mut(&parent) {
            dir.violations_here.push(info);
        }
    }

    // Distribute coupling metrics (informational, not violations) to directories
    for cm in &result.coupling_metrics {
        let parent = common_parent_dir(&[&cm.dir_a, &cm.dir_b]);
        let info = ViolationInfo {
            from_module: cm.from_module.clone(),
            to_module: cm.to_module.clone(),
            severity: cm.severity,
        };
        if let Some(dir) = dirs.get_mut(&parent) {
            dir.coupling_metrics_here.push(info);
        }
    }

    // Propagate module counts up
    let dir_paths_sorted: Vec<String> = {
        let mut paths: Vec<String> = dirs.keys().cloned().collect();
        paths.sort_by_key(|a| std::cmp::Reverse(a.len())); // deepest first
        paths
    };
    for dir_path in &dir_paths_sorted {
        let child_count: usize = {
            let dir = dirs.get(dir_path).unwrap();
            dir.children_dirs
                .iter()
                .filter_map(|c| dirs.get(c).map(|d| d.module_count))
                .sum()
        };
        if let Some(dir) = dirs.get_mut(dir_path) {
            dir.module_count += child_count;
        }
    }

    // Compute per-directory scores
    for dir_path in &dir_paths_sorted {
        let violation_severity: f64 = dirs
            .get(dir_path)
            .map(|d| d.violations_here.iter().map(|v| v.severity).sum())
            .unwrap_or(0.0);
        let module_count = dirs.get(dir_path).map(|d| d.module_count).unwrap_or(1);
        let score = if module_count > 0 {
            (100.0 * (1.0 - violation_severity / module_count as f64)).max(0.0)
        } else {
            100.0
        };
        if let Some(dir) = dirs.get_mut(dir_path) {
            dir.score = score;
        }
    }

    // Mark directories that have violations anywhere in their subtree
    for dir_path in &dir_paths_sorted {
        let has_violations = {
            let dir = dirs.get(dir_path).unwrap();
            !dir.violations_here.is_empty()
                || dir.children_dirs.iter().any(|c| {
                    dirs.get(c)
                        .map(|d| d.has_deep_violations || !d.violations_here.is_empty())
                        .unwrap_or(false)
                })
        };
        if let Some(dir) = dirs.get_mut(dir_path) {
            dir.has_deep_violations = has_violations;
        }
    }

    // File every Issue under the deepest directory containing its subject;
    // anchors outside the tree (or above the root) go to the root page.
    let issues = result.issues();
    let mut issues_per_dir: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, issue) in issues.iter().enumerate() {
        let mut anchor = issue.anchor_dir();
        if !dirs.contains_key(&anchor) {
            anchor = root_path.clone();
        }
        issues_per_dir.entry(anchor).or_default().push(idx);
    }

    ReportData {
        dirs,
        root_path,
        snapshot_id: snapshot_id.to_string(),
        total_score: result.score,
        total_modules: result.total_modules,
        total_violations: result.violation_count(),
        total_tri: result.tri,
        total_xs: result.total_xs,
        score_green: settings.thresholds.score_green,
        score_yellow: settings.thresholds.score_yellow,
        abstractness: result.abstractness.clone(),
        instability: result.instability.clone(),
        distance: result.distance.clone(),
        issues,
        issues_per_dir,
        baseline_applied: result.baseline.is_some(),
    }
}

fn find_common_root(modules: &[Module]) -> String {
    if modules.is_empty() {
        return String::new();
    }
    let first_parent = std::path::Path::new(&modules[0].path)
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .to_string_lossy()
        .to_string();

    let mut common = first_parent;
    for module in &modules[1..] {
        let parent = std::path::Path::new(&module.path)
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_string_lossy()
            .to_string();
        while !parent.starts_with(&common) && !common.is_empty() {
            common = std::path::Path::new(&common)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
        }
    }
    common
}

fn module_label(path: &str, current_dir: &str) -> String {
    let p = std::path::Path::new(path);
    let file = p.file_name().and_then(|f| f.to_str()).unwrap_or(path);
    let parent = p
        .parent()
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_default();

    // Strip the current_dir prefix so the displayed path is relative
    let relative = if !current_dir.is_empty() {
        let prefix = format!("{}/", current_dir);
        if let Some(stripped) = parent.strip_prefix(&prefix) {
            stripped.to_string()
        } else if parent == current_dir {
            String::new()
        } else {
            parent.clone()
        }
    } else {
        parent.clone()
    };

    if relative.is_empty() {
        file.to_string()
    } else {
        format!(
            "<span class=\"module-tag\" title=\"{}\">{}</span> {}",
            parent, relative, file
        )
    }
}

fn score_color(score: f64, green: f64, yellow: f64) -> &'static str {
    if score >= green {
        "#22c55e"
    } else if score >= yellow {
        "#eab308"
    } else {
        "#ef4444"
    }
}

fn render_page(data: &ReportData, dir_path: &str) -> String {
    let dir = match data.dirs.get(dir_path) {
        Some(d) => d,
        None => return String::from("<html><body>Directory not found</body></html>"),
    };

    let breadcrumbs = build_breadcrumbs(dir_path, &data.root_path);
    let score_clr = score_color(dir.score, data.score_green, data.score_yellow);

    let is_root = dir_path == data.root_path;
    let project_banner = if is_root {
        let banner_clr = score_color(data.total_score, data.score_green, data.score_yellow);
        format!(
            "<div class=\"summary\" style=\"background:#f8fafc;border:1px solid #e2e8f0;border-radius:8px;padding:0.75rem 1rem;margin-bottom:1rem\">
                <div class=\"summary-card\" title=\"Overall project health, computed from all violations across the entire codebase. This is the canonical score reported by audit and used in CI gates.\"><div class=\"label\">Project Score <span class=\"info-icon\">&#9432;</span></div><div class=\"value\" style=\"color:{}\">{:.1}</div></div>
                <div class=\"summary-card\"><div class=\"label\">Total Modules</div><div class=\"value\">{}</div></div>
                <div class=\"summary-card\"><div class=\"label\">Total Violations</div><div class=\"value\">{}</div></div>
                <div class=\"summary-card\" title=\"Total Risk Index: sum of all violation RRIs (Relationship Risk Index). Lower is better.\"><div class=\"label\">TRI <span class=\"info-icon\">&#9432;</span></div><div class=\"value\">{:.0}</div></div>
                <div class=\"summary-card\" title=\"Total Excess: imports that need to be removed across all violations to reach a clean state.\"><div class=\"label\">Total XS <span class=\"info-icon\">&#9432;</span></div><div class=\"value\">{}</div></div>
            </div>
            <p class=\"score-hint\">The <strong>Project Score</strong> above is the overall codebase health. The <strong>Health Score</strong> in the cards below reflects only this directory &mdash; a directory can be 100/100 while the project score is lower because violations live in subdirectories.</p>
            <details class=\"metrics-guide\" style=\"margin-top:0.75rem;font-size:0.75rem;color:#475569\">
                <summary style=\"cursor:pointer;font-weight:600;color:#334155\">Metrics Guide</summary>
                <div style=\"margin-top:0.5rem;line-height:1.6\">
                    <p><strong>Project Score</strong> (0&ndash;100) &mdash; overall codebase health derived from the Total Risk Index. Higher is better. Formula: <code>100 &times; (1 &minus; TRI / (modules &times; max_weight))</code></p>
                    <p><strong>TRI</strong> (Total Risk Index) &mdash; sum of all violation RRIs. Lower is better. A TRI of 0 means no violations.</p>
                    <p><strong>RRI</strong> (Relationship Risk Index) &mdash; risk score for a single violation. <code>RRI = direction_weight &times; density</code>, where density is the number of imports between the two modules.</p>
                    <p><strong>Severity</strong> &mdash; legacy metric based on depth. Being replaced by RRI in future versions.</p>
                    <p><strong>Total XS</strong> (Excess) &mdash; total import statements that need to be removed to fix all violations.</p>
                    <p style=\"margin-top:0.5rem\"><strong>Direction types and weights:</strong></p>
                    <table style=\"font-size:0.72rem;border-collapse:collapse;margin:0.25rem 0\">
                        <tr><td style=\"padding:0.15rem 0.5rem\"><span style=\"color:#22c55e\">&darr;</span> <strong>Downward</strong></td><td style=\"padding:0.15rem 0.5rem\">Weight 2</td><td style=\"padding:0.15rem 0.5rem;color:#64748b\">Parent imports child. Normal architectural flow.</td></tr>
                        <tr><td style=\"padding:0.15rem 0.5rem\"><span style=\"color:#eab308\">&harr;</span> <strong>Sibling</strong></td><td style=\"padding:0.15rem 0.5rem\">Weight 4</td><td style=\"padding:0.15rem 0.5rem;color:#64748b\">Same-level directories import each other. Signals missing shared abstraction.</td></tr>
                        <tr><td style=\"padding:0.15rem 0.5rem\"><span style=\"color:#ef4444\">&uarr;</span> <strong>Upward</strong></td><td style=\"padding:0.15rem 0.5rem\">Weight 6</td><td style=\"padding:0.15rem 0.5rem;color:#64748b\">Child imports parent. Destroys module reusability.</td></tr>
                        <tr><td style=\"padding:0.15rem 0.5rem\"><span style=\"color:#dc2626\">&#8635;</span> <strong>Circular</strong></td><td style=\"padding:0.15rem 0.5rem\">Weight 10</td><td style=\"padding:0.15rem 0.5rem;color:#64748b\">Mutual or transitive cycle. Breaks builds and makes testing impossible.</td></tr>
                    </table>
                </div>
            </details>",
            banner_clr, data.total_score, data.total_modules, data.total_violations, data.total_tri, data.total_xs
        )
    } else {
        String::new()
    };

    let mut children_rows = String::new();
    for child_path in &dir.children_dirs {
        if let Some(child) = data.dirs.get(child_path) {
            let warning = if child.has_deep_violations || !child.violations_here.is_empty() {
                "<span class=\"warning\" title=\"Contains violations\">&#9888;</span>"
            } else {
                ""
            };
            let child_score_clr = score_color(child.score, data.score_green, data.score_yellow);
            // Use relative link: child name + /index.html
            children_rows.push_str(&format!(
                "<tr>
                    <td><a href=\"{}/index.html\" class=\"dir-link\">&#128193; {}</a> {}</td>
                    <td class=\"center\">{}</td>
                    <td class=\"center\"><span class=\"score-badge\" style=\"background:{}\">{:.1}</span></td>
                    <td class=\"center\">{}</td>
                </tr>\n",
                child.name,
                child.name,
                warning,
                child.module_count,
                child_score_clr,
                child.score,
                child.violations_here.len(),
            ));
        }
    }

    for file in &dir.files {
        children_rows.push_str(&format!(
            "<tr>
                <td class=\"file\">&#128196; {}</td>
                <td class=\"center\">1</td>
                <td class=\"center\"><span class=\"score-badge\" style=\"background:#22c55e\">-</span></td>
                <td class=\"center\">-</td>
            </tr>\n",
            file,
        ));
    }

    let mut violations_html = String::new();

    // Issues: root lists every Issue with a kind-count summary; a
    // directory page lists the Issues anchored under it.
    let page_issues: Vec<&Issue> = if is_root {
        data.issues.iter().collect()
    } else {
        data.issues_per_dir
            .get(dir_path)
            .map(|idxs| idxs.iter().map(|&i| &data.issues[i]).collect())
            .unwrap_or_default()
    };
    violations_html.push_str(&render_issue_section(
        &page_issues,
        is_root,
        data.baseline_applied,
    ));

    // Coupling Metrics — informational sibling coupling pairs (not violations)
    if !dir.coupling_metrics_here.is_empty() {
        violations_html.push_str(&format!(
            "<h2>Coupling Metrics <small style=\"font-weight:400;color:#64748b\">({} sibling coupling pair{})</small></h2>\n",
            dir.coupling_metrics_here.len(),
            if dir.coupling_metrics_here.len() == 1 { "" } else { "s" }
        ));
        violations_html.push_str("<p class=\"section-hint\">Sibling directories that import each other. Informational &mdash; not flagged as violations in actionable mode. Use to gauge coupling density.</p>\n");
        violations_html.push_str("<table class=\"violations\">\n");
        violations_html.push_str("<tr><th>Severity</th><th>From</th><th>To</th></tr>\n");
        let mut sorted_metrics = dir.coupling_metrics_here.clone();
        sorted_metrics.sort_by(|a, b| {
            b.severity
                .partial_cmp(&a.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for v in &sorted_metrics {
            let from_label = module_label(&v.from_module, dir_path);
            let to_label = module_label(&v.to_module, dir_path);
            violations_html.push_str(&format!(
                "<tr><td><span class=\"severity\" style=\"color:#94a3b8\">{:.2}</span></td><td title=\"{}\">{}</td><td title=\"{}\">{}</td></tr>\n",
                v.severity, v.from_module, from_label, v.to_module, to_label,
            ));
        }
        violations_html.push_str("</table>\n");
    }

    let is_root = dir_path == data.root_path;
    let title = if is_root {
        "noupling Report".to_string()
    } else {
        format!("{} - noupling Report", dir.name)
    };

    // Per-directory instability for the summary card. "—" when the directory
    // has no edges crossing its boundary (Ca + Ce == 0), where I is undefined.
    let instability_label = data
        .instability
        .iter()
        .find(|i| i.dir == dir_path)
        .map(|i| format!("{:.2}", i.instability))
        .unwrap_or_else(|| "—".to_string());

    // Root-page only: abstractness section (project-wide metric)
    if is_root && !data.abstractness.is_empty() {
        violations_html.push_str("<h2>Abstractness</h2>\n");
        violations_html.push_str("<p class=\"section-hint\">Per-directory Martin abstractness A = abstract / (abstract + concrete). 0.0 = all concrete, 1.0 = all abstract.</p>\n");
        violations_html.push_str("<table>\n");
        violations_html.push_str("<tr><th>Directory</th><th class=\"center\">A</th><th class=\"center\">Abstract</th><th class=\"center\">Concrete</th></tr>\n");
        for a in data.abstractness.iter().take(20) {
            violations_html.push_str(&format!(
                "<tr><td>{}</td><td class=\"center\">{:.2}</td><td class=\"center\">{}</td><td class=\"center\">{}</td></tr>\n",
                a.dir, a.abstractness, a.abstract_count, a.concrete_count,
            ));
        }
        violations_html.push_str("</table>\n");
    }

    // Root-page only: Distance from main sequence
    if is_root && !data.distance.is_empty() {
        use noupling_core::analyzer::Zone;
        violations_html.push_str("<h2>Distance from Main Sequence</h2>\n");
        violations_html.push_str("<p class=\"section-hint\">Martin's D = |A + I − 1|. 0.0 = on the main sequence (well-balanced). High D + low I = Zone of Pain (stable + concrete, rigid). High D + high I = Zone of Uselessness (abstract + unstable, speculative).</p>\n");
        violations_html.push_str("<table>\n");
        violations_html.push_str("<tr><th>Directory</th><th class=\"center\">D</th><th class=\"center\">A</th><th class=\"center\">I</th><th>Zone</th></tr>\n");
        for d in data.distance.iter().take(20) {
            let zone_label = match d.zone {
                Zone::MainSequence => "main sequence",
                Zone::Pain => "Zone of Pain",
                Zone::Uselessness => "Zone of Uselessness",
            };
            violations_html.push_str(&format!(
                "<tr><td>{}</td><td class=\"center\">{:.2}</td><td class=\"center\">{:.2}</td><td class=\"center\">{:.2}</td><td>{}</td></tr>\n",
                d.dir, d.distance, d.abstractness, d.instability, zone_label,
            ));
        }
        violations_html.push_str("</table>\n");
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #f8fafc; color: #1e293b; padding: 2rem; max-width: 960px; margin: 0 auto; }}
h1 {{ font-size: 1.5rem; margin-bottom: 0.5rem; }}
h2 {{ font-size: 1.2rem; margin: 1.5rem 0 0.75rem; color: #475569; }}
.breadcrumbs {{ font-size: 0.85rem; color: #64748b; margin-bottom: 1.5rem; }}
.breadcrumbs a {{ color: #3b82f6; text-decoration: none; }}
.breadcrumbs a:hover {{ text-decoration: underline; }}
.summary {{ display: flex; gap: 1.5rem; margin-bottom: 1.5rem; flex-wrap: wrap; }}
.summary-card {{ background: white; border: 1px solid #e2e8f0; border-radius: 8px; padding: 1rem 1.5rem; flex: 1; min-width: 140px; }}
.summary-card .label {{ font-size: 0.75rem; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.05em; }}
.summary-card .value {{ font-size: 1.75rem; font-weight: 700; margin-top: 0.25rem; }}
.score-big {{ color: {score_clr}; }}
table {{ width: 100%; border-collapse: collapse; background: white; border: 1px solid #e2e8f0; border-radius: 8px; overflow: hidden; }}
th {{ background: #f1f5f9; padding: 0.6rem 1rem; text-align: left; font-size: 0.8rem; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; }}
td {{ padding: 0.6rem 1rem; border-top: 1px solid #f1f5f9; font-size: 0.9rem; }}
tr:hover {{ background: #f8fafc; }}
.center {{ text-align: center; }}
.dir-link {{ color: #3b82f6; text-decoration: none; font-weight: 500; }}
.dir-link:hover {{ text-decoration: underline; }}
.file {{ color: #64748b; }}
.score-badge {{ display: inline-block; padding: 0.15rem 0.5rem; border-radius: 4px; color: white; font-weight: 600; font-size: 0.8rem; }}
.warning {{ color: #f59e0b; margin-left: 0.3rem; font-size: 1.1rem; }}
.severity {{ font-weight: 700; font-size: 0.95rem; }}
.circular {{ background: #fef2f2; color: #dc2626; padding: 0.15rem 0.4rem; border-radius: 3px; font-size: 0.8rem; font-weight: 600; }}
.circular-note {{ color: #dc2626; font-style: italic; }}
.cycle-path {{ display: inline-block; margin-top: 0.3rem; padding: 0.3rem 0.5rem; background: #fef2f2; border: 1px solid #fecaca; border-radius: 4px; font-size: 0.85rem; font-weight: 500; color: #991b1b; line-height: 1.6; }}
.hop-file {{ color: #6b7280; font-weight: 400; cursor: pointer; }}
.full-paths {{ margin-top: 0.4rem; padding: 0.4rem 0.6rem; background: #fff5f5; border-radius: 4px; font-size: 0.78rem; color: #64748b; line-height: 1.7; word-break: break-all; }}
.full-paths strong {{ color: #991b1b; }}
details summary {{ list-style: none; cursor: pointer; }}
details summary::marker {{ display: none; content: ''; }}
details summary::before {{ content: ''; }}
details summary.cycle-path::before {{ content: '\25B6'; font-size: 0.65rem; margin-right: 0.4rem; color: #94a3b8; transition: transform 0.15s; display: inline-block; }}
details[open] summary.cycle-path::before {{ transform: rotate(90deg); }}
.violations {{ margin-bottom: 1.5rem; }}
.snapshot {{ font-size: 0.75rem; color: #94a3b8; margin-top: 0.5rem; }}
.footer {{ margin-top: 2rem; padding-top: 1rem; border-top: 1px solid #e2e8f0; font-size: 0.75rem; color: #94a3b8; }}
.violations-promoted {{ margin-bottom: 1rem; }}
.section-hint {{ font-size: 0.75rem; color: #64748b; margin-bottom: 0.6rem; font-style: italic; }}
.module-tag {{ display: inline-block; background: #e0f2fe; color: #075985; font-size: 0.7rem; font-weight: 600; padding: 0.05rem 0.35rem; border-radius: 3px; margin-right: 0.25rem; vertical-align: middle; }}
.score-hint {{ font-size: 0.75rem; color: #64748b; margin-top: 0.5rem; line-height: 1.4; }}
.info-icon {{ color: #94a3b8; font-size: 0.7rem; cursor: help; margin-left: 0.15rem; }}
.summary-card[title] {{ cursor: help; }}
.issue-card {{ background: #fff; border: 1px solid #e2e8f0; border-left: 4px solid #94a3b8; border-radius: 6px; padding: 0.75rem 1rem; margin-bottom: 0.6rem; }}
.issue-card.band-critical {{ border-left-color: #dc2626; }}
.issue-card.band-high {{ border-left-color: #f97316; }}
.issue-card.band-medium {{ border-left-color: #eab308; }}
.issue-card.band-low {{ border-left-color: #94a3b8; }}
.issue-card.baselined {{ opacity: 0.6; }}
.issue-title {{ font-weight: 600; font-size: 0.95rem; }}
.issue-title code {{ font-weight: 500; font-size: 0.85rem; color: #334155; }}
.band {{ display: inline-block; font-size: 0.7rem; font-weight: 700; letter-spacing: 0.05em; padding: 0.1rem 0.4rem; border-radius: 3px; color: #fff; background: #94a3b8; margin-right: 0.4rem; vertical-align: middle; }}
.band-critical {{ background: #dc2626; }}
.band-high {{ background: #f97316; }}
.band-medium {{ background: #eab308; }}
.band-low {{ background: #94a3b8; }}
.issue-extra {{ font-size: 0.8rem; color: #64748b; margin-top: 0.2rem; }}
.issue-reason, .issue-recommendation, .issue-impact {{ font-size: 0.85rem; margin-top: 0.3rem; }}
.issue-reason strong, .issue-recommendation strong, .issue-impact strong {{ color: #475569; }}
.baselined-tag {{ font-size: 0.7rem; color: #64748b; margin-left: 0.4rem; font-weight: 500; }}
.violation-card {{ display: flex; align-items: center; gap: 1rem; background: #fff; border: 1px solid #fecaca; border-left: 4px solid #ef4444; border-radius: 6px; padding: 0.75rem 1rem; margin-bottom: 0.5rem; }}
.violation-sev {{ font-weight: 800; font-size: 1.05rem; min-width: 50px; text-align: right; }}
.violation-body {{ flex: 1; font-size: 0.85rem; }}
.violation-title {{ color: #1e293b; margin-bottom: 0.25rem; }}
.violation-detail {{ color: #6b7280; font-size: 0.7rem; word-break: break-all; }}
.violations-section {{ background: #f8fafc; border: 1px solid #e2e8f0; border-radius: 6px; padding: 0.5rem 1rem; margin-bottom: 0.75rem; }}
.violations-section > summary {{ list-style: revert; cursor: pointer; font-weight: 600; color: #475569; font-size: 0.9rem; padding: 0.25rem 0; user-select: none; }}
.violations-section[open] > summary {{ margin-bottom: 0.5rem; border-bottom: 1px solid #e2e8f0; padding-bottom: 0.5rem; }}
.violations-section > summary::before {{ content: ''; }}
.violations-section > summary::marker {{ display: revert; content: revert; }}
</style>
</head>
<body>
<div class="breadcrumbs">{breadcrumbs}</div>
<h1>{title}</h1>
<p class="snapshot">Snapshot: {snapshot_id}</p>

{project_banner}

<div class="summary">
    <div class="summary-card" title="Health Score for this directory only. Computed from violations originating here (not from subdirectories).">
        <div class="label">Health Score <span class="info-icon">&#9432;</span></div>
        <div class="value score-big">{score:.1}</div>
    </div>
    <div class="summary-card" title="Total source files in this directory and its subdirectories.">
        <div class="label">Modules <span class="info-icon">&#9432;</span></div>
        <div class="value">{modules}</div>
    </div>
    <div class="summary-card" title="Violations that have this directory as their common parent (i.e., the directory pair both belong to this subtree).">
        <div class="label">Violations <span class="info-icon">&#9432;</span></div>
        <div class="value">{violations}</div>
    </div>
    <div class="summary-card" title="Martin's instability I = Ce / (Ca + Ce). 0.0 = fully stable (depended on, doesn't depend), 1.0 = fully unstable (depends on, isn't depended on). '—' when this directory has no edges crossing its boundary.">
        <div class="label">Instability <span class="info-icon">&#9432;</span></div>
        <div class="value">{instability}</div>
    </div>
</div>

<h2>Contents</h2>
<table>
<tr><th>Name</th><th class="center">Modules</th><th class="center">Score</th><th class="center">Violations</th></tr>
{children_rows}
</table>

{violations_html}

<div class="footer">Generated by {version}</div>
</body>
</html>"#,
        title = title,
        breadcrumbs = breadcrumbs,
        snapshot_id = data.snapshot_id,
        project_banner = project_banner,
        score_clr = score_clr,
        score = dir.score,
        modules = dir.module_count,
        violations = dir.violations_here.len(),
        instability = instability_label,
        children_rows = children_rows,
        violations_html = violations_html,
        version = super::VERSION,
    )
}

/// The Issues section: heading with counts, kind-count table (root only),
/// and one card per Issue in `issues()` order. The match on `IssueDetail`
/// is exhaustive so a new kind fails to compile until handled.
fn render_issue_section(issues: &[&Issue], is_root: bool, baseline_applied: bool) -> String {
    let mut html = String::new();
    if issues.is_empty() {
        if is_root {
            html.push_str("<h2>Issues <small style=\"font-weight:400;color:#64748b\">(0)</small></h2>\n<p class=\"section-hint\">No Issues found.</p>\n");
        }
        return html;
    }
    let baselined = issues.iter().filter(|i| i.baselined).count();
    let counts = if baseline_applied {
        format!(
            "({} &middot; {} new &middot; {} baselined)",
            issues.len(),
            issues.len() - baselined,
            baselined
        )
    } else {
        format!("({})", issues.len())
    };
    html.push_str(&format!(
        "<h2>Issues <small style=\"font-weight:400;color:#64748b\">{}</small></h2>\n",
        counts
    ));
    html.push_str("<p class=\"section-hint\">Every Issue the audit found, in canonical order: severity band, then kind, then subject. Each card says why it exists and what to do.</p>\n");

    if is_root {
        html.push_str("<table style=\"margin-bottom:1rem\">\n<tr><th>Kind</th><th class=\"center\">Count</th></tr>\n");
        for kind in IssueKind::ALL {
            let n = issues.iter().filter(|i| i.kind() == kind).count();
            if n > 0 {
                html.push_str(&format!(
                    "<tr><td>{}</td><td class=\"center\">{}</td></tr>\n",
                    kind, n
                ));
            }
        }
        html.push_str("</table>\n");
    }

    for issue in issues {
        let band = issue.severity().name();
        let extra: Option<String> = match &issue.detail {
            IssueDetail::CouplingViolation(v) => Some(format!(
                "{} &lt;&gt; {} &mdash; depth {}, line {}",
                esc(&v.dir_a),
                esc(&v.dir_b),
                v.depth,
                v.line_number
            )),
            IssueDetail::Cycle(v) => v
                .weakest_link
                .as_ref()
                .map(|wl| format!("Weakest link: {}", esc(wl))),
            IssueDetail::RuleViolation(r) => Some(format!("line {}", r.line_number)),
            IssueDetail::LayerViolation(l) => Some(format!(
                "{} &rarr; {} (line {})",
                esc(&l.from_layer),
                esc(&l.to_layer),
                l.line_number
            )),
            IssueDetail::GravityWell(g) => Some(format!(
                "RRI {:.0} across {} relationships",
                g.total_rri, g.relationship_count
            )),
            IssueDetail::RedFlag(f) => Some(format!("RRI {:.0}", f.rri)),
            IssueDetail::StabilityViolation(s) => Some(format!(
                "I={:.2} &rarr; I={:.2}",
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
        html.push_str(&format!(
            "<div class=\"issue-card band-{band}{baselined}\">\n  <div class=\"issue-title\"><span class=\"band band-{band}\">{band_upper}</span> {kind}: <code>{subject}</code>{tag}</div>\n",
            band = band,
            band_upper = band.to_uppercase(),
            baselined = if issue.baselined { " baselined" } else { "" },
            kind = issue.kind(),
            subject = esc(&issue.subject().to_string()),
            tag = if issue.baselined {
                "<span class=\"baselined-tag\">baselined</span>"
            } else {
                ""
            },
        ));
        if let Some(extra) = extra {
            html.push_str(&format!("  <div class=\"issue-extra\">{}</div>\n", extra));
        }
        html.push_str(&format!(
            "  <div class=\"issue-reason\"><strong>Reason:</strong> {}</div>\n  <div class=\"issue-recommendation\"><strong>Recommendation:</strong> {}</div>\n",
            esc(&issue.reason()),
            esc(&issue.recommendation()),
        ));
        let impact = issue.score_impact();
        html.push_str(&format!(
            "  <div class=\"issue-impact\"><strong>Score impact:</strong> {}</div>\n</div>\n",
            if impact > 0.0 {
                format!("{:.1}", impact)
            } else {
                "0 (does not score)".to_string()
            }
        ));
    }
    html
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn build_breadcrumbs(current_path: &str, root_path: &str) -> String {
    if current_path == root_path {
        let name = std::path::Path::new(root_path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("root");
        return format!("<strong>{}</strong>", name);
    }

    // Calculate how many levels deep we are from root
    let relative = current_path
        .strip_prefix(&format!("{}/", root_path))
        .unwrap_or(current_path);
    let segments: Vec<&str> = relative.split('/').collect();
    let depth = segments.len();

    let mut parts = Vec::new();

    // Root link: go up `depth` levels
    let root_name = std::path::Path::new(root_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("root");
    let up = "../".repeat(depth);
    parts.push(format!("<a href=\"{}index.html\">{}</a>", up, root_name));

    // Intermediate segments: each one fewer ../
    for (i, seg) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            parts.push(format!("<strong>{}</strong>", seg));
        } else {
            let levels_up = depth - i - 1;
            let up = "../".repeat(levels_up);
            parts.push(format!("<a href=\"{}index.html\">{}</a>", up, seg));
        }
    }

    parts.join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use noupling_core::analyzer::AuditResultBuilder;
    use noupling_core::analyzer::CouplingViolation;
    use noupling_core::core::ModuleType;

    fn make_module(id: &str, path: &str) -> Module {
        Module {
            id: id.to_string(),
            snapshot_id: "snap".to_string(),
            parent_id: None,
            name: std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            path: path.to_string(),
            module_type: ModuleType::File,
            depth: std::path::Path::new(path).components().count() as i32,
        }
    }

    #[test]
    fn html_root_renders_stability_violation_as_an_issue_card() {
        use noupling_core::analyzer::StabilityViolation;
        let modules = vec![make_module("a", "src/api/mod.rs")];
        let result = AuditResultBuilder::new()
            .with_total_modules(1)
            .with_stability_violations(vec![StabilityViolation {
                from_dir: "src/stable".into(),
                to_dir: "src/unstable".into(),
                from_instability: 0.17,
                to_instability: 0.83,
            }])
            .build();
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        generate_html_report(&modules, &result, "snap-sv", dir.path(), &settings).unwrap();
        let html = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
        assert!(
            html.contains("<h2>Issues <small"),
            "missing Issues heading: {html}"
        );
        assert!(
            html.contains("<span class=\"band band-medium\">MEDIUM</span> Stability Violation"),
            "missing card header: {html}"
        );
        assert!(
            html.contains("src/stable -&gt; src/unstable"),
            "missing subject"
        );
        assert!(html.contains("class=\"issue-reason\""), "missing reason");
        assert!(
            html.contains("class=\"issue-recommendation\""),
            "missing recommendation"
        );
        // Kind-count summary on the root page.
        assert!(
            html.contains("<td>Stability Violation</td><td class=\"center\">1</td>"),
            "{html}"
        );
        // Old section is gone.
        assert!(!html.contains("<h2>Stability Violations</h2>"));
    }

    /// Directory pages list only the Issues anchored under them; the root
    /// lists everything.
    #[test]
    fn html_directory_pages_show_only_their_own_issues() {
        use noupling_core::analyzer::{CohesionMetrics, DirectoryKind};
        let modules = vec![
            make_module("a", "src/loose/x/x1.rs"),
            make_module("b", "src/loose/y/y1.rs"),
            make_module("c", "src/bag/a.rs"),
            make_module("d", "src/bag/b.rs"),
            make_module("e", "src/bag/c.rs"),
        ];
        let result = AuditResultBuilder::new()
            .with_total_modules(5)
            .with_violations(vec![CouplingViolation {
                dir_a: "src/loose/x".to_string(),
                dir_b: "src/loose/y".to_string(),
                from_module: "src/loose/x/x1.rs".to_string(),
                to_module: "src/loose/y/y1.rs".to_string(),
                depth: 2,
                severity: 0.33,
                direction: noupling_core::analyzer::DependencyDirection::Sibling,
                rri: 4.0,
                is_circular: false,
                cycle_path: Vec::new(),
                cycle_hop_files: Vec::new(),
                cycle_order: 0,
                cycle_hop_counts: Vec::new(),
                weakest_link: None,
                break_cost: 0,
                score_impact: 1.5,
                line_number: 2,
                weight: 1,
            }])
            .with_cohesion(vec![CohesionMetrics {
                dir: "src/bag".into(),
                kind: DirectoryKind::Package,
                n_children: 3,
                internal_deps: 0,
                cohesion: Some(0.0),
            }])
            .build();
        let dir = tempfile::tempdir().unwrap();
        generate_html_report(
            &modules,
            &result,
            "snap-d",
            dir.path(),
            &Settings::default(),
        )
        .unwrap();

        let root = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
        assert!(root.contains("Coupling Violation"), "{root}");
        assert!(root.contains("Low Cohesion"), "{root}");

        let loose = std::fs::read_to_string(dir.path().join("loose/index.html")).unwrap();
        assert!(
            loose.contains("band-high\">HIGH</span> Coupling Violation"),
            "{loose}"
        );
        assert!(
            !loose.contains("Low Cohesion"),
            "bag's Issue leaked into loose: {loose}"
        );

        let bag = std::fs::read_to_string(dir.path().join("bag/index.html")).unwrap();
        assert!(bag.contains("band-low\">LOW</span> Low Cohesion"), "{bag}");
        assert!(!bag.contains("Coupling Violation"), "{bag}");
    }

    /// Regenerating into the same directory removes pages for directories
    /// that no longer exist, and leaves files noupling did not write (#383).
    #[test]
    fn html_regeneration_removes_stale_pages() {
        let out = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        let before = vec![
            make_module("a", "src/old/a.rs"),
            make_module("b", "src/keep/b.rs"),
        ];
        let result = AuditResultBuilder::new().with_total_modules(2).build();
        generate_html_report(&before, &result, "s1", out.path(), &settings).unwrap();
        assert!(out.path().join("old/index.html").exists());
        std::fs::write(out.path().join("notes.txt"), "mine").unwrap();

        let after = vec![
            make_module("a", "src/new/a.rs"),
            make_module("b", "src/keep/b.rs"),
        ];
        generate_html_report(&after, &result, "s2", out.path(), &settings).unwrap();
        assert!(
            !out.path().join("old").exists(),
            "stale page must be removed"
        );
        assert!(out.path().join("new/index.html").exists());
        assert!(out.path().join("keep/index.html").exists());
        assert!(
            out.path().join("notes.txt").exists(),
            "files noupling did not write are left alone"
        );
    }

    #[test]
    fn html_per_directory_renders_instability_summary_card() {
        use noupling_core::analyzer::InstabilityMetric;
        let modules = vec![
            make_module("a", "src/app/main.rs"),
            make_module("b", "src/core/lib.rs"),
        ];
        let result = AuditResultBuilder::new()
            .with_total_modules(2)
            .with_instability(vec![InstabilityMetric {
                dir: "src/app".into(),
                ca: 0,
                ce: 1,
                instability: 1.0,
            }])
            .build();
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        generate_html_report(&modules, &result, "snap-i", dir.path(), &settings).unwrap();
        let html = std::fs::read_to_string(dir.path().join("app/index.html")).unwrap();
        assert!(
            html.contains("Instability"),
            "missing Instability label: {}",
            &html[..500.min(html.len())]
        );
        assert!(html.contains(">1.00<"), "missing I=1.00 value in card");
    }

    #[test]
    fn html_root_renders_abstractness_section() {
        use noupling_core::analyzer::AbstractnessMetric;
        let modules = vec![make_module("a", "src/api/mod.rs")];
        let result = AuditResultBuilder::new()
            .with_total_modules(1)
            .with_abstractness(vec![AbstractnessMetric {
                dir: "src/api".into(),
                abstract_count: 2,
                concrete_count: 3,
                abstractness: 0.4,
            }])
            .build();
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        generate_html_report(&modules, &result, "snap-x", dir.path(), &settings).unwrap();
        let html = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
        assert!(html.contains("<h2>Abstractness</h2>"), "missing header");
        assert!(html.contains("src/api"), "missing dir name");
        assert!(html.contains("0.40"), "missing A value");
    }

    #[test]
    fn generates_html_files() {
        let modules = vec![
            make_module("a", "src/scanner/mod.rs"),
            make_module("b", "src/storage/mod.rs"),
        ];
        let result = AuditResultBuilder::new().with_total_modules(2).build();

        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        generate_html_report(&modules, &result, "snap-1", dir.path(), &settings).unwrap();

        assert!(dir.path().join("index.html").exists());
    }

    #[test]
    fn html_contains_score() {
        let modules = vec![make_module("a", "src/mod.rs")];
        let result = AuditResultBuilder::new()
            .with_score(95.5)
            .with_total_modules(1)
            .build();

        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        generate_html_report(&modules, &result, "snap-1", dir.path(), &settings).unwrap();

        let html = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
        assert!(html.contains("noupling Report"));
        assert!(html.contains("snap-1"));
    }

    #[test]
    fn html_generates_subdirectory_pages() {
        let modules = vec![
            make_module("a", "src/scanner/parser.rs"),
            make_module("b", "src/scanner/resolver.rs"),
            make_module("c", "src/storage/db.rs"),
        ];
        let result = AuditResultBuilder::new().with_total_modules(3).build();

        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        generate_html_report(&modules, &result, "snap-1", dir.path(), &settings).unwrap();

        // Should have pages for scanner and storage subdirs
        assert!(dir.path().join("index.html").exists());
        assert!(dir.path().join("scanner/index.html").exists());
        assert!(dir.path().join("storage/index.html").exists());
    }

    #[test]
    fn html_shows_violations() {
        let modules = vec![
            make_module("a", "src/scanner/mod.rs"),
            make_module("b", "src/storage/mod.rs"),
        ];
        let result = AuditResultBuilder::new()
            .with_violations(vec![CouplingViolation {
                dir_a: "src/scanner".to_string(),
                dir_b: "src/storage".to_string(),
                from_module: "src/scanner/mod.rs".to_string(),
                to_module: "src/storage/mod.rs".to_string(),
                depth: 1,
                severity: 0.5,
                direction: noupling_core::analyzer::DependencyDirection::Sibling,
                rri: 0.0,
                is_circular: false,
                cycle_path: Vec::new(),
                cycle_hop_files: Vec::new(),
                cycle_order: 0,
                cycle_hop_counts: Vec::new(),
                weakest_link: None,
                break_cost: 0,
                score_impact: 0.0,
                line_number: 0,
                weight: 0,
            }])
            .with_score(75.0)
            .with_total_modules(2)
            .build();

        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        generate_html_report(&modules, &result, "snap-1", dir.path(), &settings).unwrap();

        let html = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
        assert!(html.contains("Coupling Violation"), "{html}");
        assert!(
            html.contains("src/scanner/mod.rs -&gt; src/storage/mod.rs"),
            "{html}"
        );
    }
}
