use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

use crate::analyzer::AuditResult;
use crate::core::Module;
use crate::settings::Settings;

/// A node in the directory tree used for HTML navigation.
#[derive(Debug)]
struct DirNode {
    name: String,
    #[allow(dead_code)]
    path: String,
    children_dirs: Vec<String>,
    files: Vec<String>,
    violations_here: Vec<ViolationInfo>,
    has_deep_violations: bool,
    score: f64,
    module_count: usize,
}

#[derive(Debug, Clone)]
struct ViolationInfo {
    from_module: String,
    to_module: String,
    severity: f64,
    is_circular: bool,
    #[allow(dead_code)]
    circular_direction: Option<String>,
    cycle_path: Vec<String>,
    cycle_hop_files: Vec<(String, String, i32)>,
    cycle_order: usize,
    cycle_hop_counts: Vec<usize>,
}

struct ReportData {
    dirs: BTreeMap<String, DirNode>,
    root_path: String,
    snapshot_id: String,
    #[allow(dead_code)]
    total_score: f64,
    #[allow(dead_code)]
    total_modules: usize,
    #[allow(dead_code)]
    total_violations: usize,
    score_green: f64,
    score_yellow: f64,
    critical_severity: f64,
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

    // Count circular dependency directions for direction indicator
    let mut circular_counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for v in &result.violations {
        if v.is_circular {
            let key = (v.dir_a.clone(), v.dir_b.clone());
            *circular_counts.entry(key).or_insert(0) += 1;
        }
    }

    // Assign violations to directories
    for violation in &result.violations {
        // For circular violations, find the common ancestor of ALL dirs in the cycle
        // For coupling violations, find the parent where dir_a and dir_b are siblings
        let parent = if violation.is_circular && !violation.cycle_path.is_empty() {
            find_common_ancestor_of_all(&violation.cycle_path)
        } else {
            find_common_parent(&violation.dir_a, &violation.dir_b)
        };

        let circular_direction = if violation.is_circular {
            let forward = circular_counts
                .get(&(violation.dir_a.clone(), violation.dir_b.clone()))
                .unwrap_or(&0);
            let reverse = circular_counts
                .get(&(violation.dir_b.clone(), violation.dir_a.clone()))
                .unwrap_or(&0);
            if forward < reverse {
                Some(format!(
                    "{} -> {} (likely wrong direction, fewer couplings)",
                    short_name(&violation.dir_a),
                    short_name(&violation.dir_b)
                ))
            } else if reverse < forward {
                Some(format!(
                    "{} -> {} (likely wrong direction, fewer couplings)",
                    short_name(&violation.dir_b),
                    short_name(&violation.dir_a)
                ))
            } else {
                Some("equal coupling in both directions".to_string())
            }
        } else {
            None
        };

        let info = ViolationInfo {
            from_module: violation.from_module.clone(),
            to_module: violation.to_module.clone(),
            severity: violation.severity,
            is_circular: violation.is_circular,
            circular_direction,
            cycle_path: violation.cycle_path.clone(),
            cycle_hop_files: violation.cycle_hop_files.clone(),
            cycle_order: violation.cycle_order,
            cycle_hop_counts: violation.cycle_hop_counts.clone(),
        };

        if let Some(dir) = dirs.get_mut(&parent) {
            dir.violations_here.push(info);
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

    ReportData {
        dirs,
        root_path,
        snapshot_id: snapshot_id.to_string(),
        total_score: result.score,
        total_modules: result.total_modules,
        total_violations: result.violations.len(),
        score_green: settings.thresholds.score_green,
        score_yellow: settings.thresholds.score_yellow,
        critical_severity: settings.thresholds.critical_severity,
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

/// Find the common ancestor directory of all paths in the list.
fn find_common_ancestor_of_all(paths: &[String]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    // Start with the parent of the first path
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
        // Shrink common until it's a prefix of this path's parent
        while !p.starts_with(&common) && !common.is_empty() {
            common = std::path::Path::new(&common)
                .parent()
                .map(|pp| pp.to_string_lossy().to_string())
                .unwrap_or_default();
        }
    }
    common
}

fn find_common_parent(path_a: &str, path_b: &str) -> String {
    let a = std::path::Path::new(path_a);
    let b = std::path::Path::new(path_b);
    let a_parent = a.parent().unwrap_or(std::path::Path::new(""));
    let b_parent = b.parent().unwrap_or(std::path::Path::new(""));
    if a_parent == b_parent {
        return a_parent.to_string_lossy().to_string();
    }
    // Walk up until we find common ancestor
    let mut current = a.to_path_buf();
    loop {
        let current_str = current.to_string_lossy().to_string();
        if path_b.starts_with(&format!("{}/", current_str)) || path_b == current_str {
            return current_str;
        }
        match current.parent() {
            Some(p) if !p.as_os_str().is_empty() => current = p.to_path_buf(),
            _ => return String::new(),
        }
    }
}

fn short_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path)
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

    // Separate circular and coupling violations
    let circular_violations: Vec<&ViolationInfo> = dir
        .violations_here
        .iter()
        .filter(|v| v.is_circular)
        .collect();
    let coupling_violations: Vec<&ViolationInfo> = dir
        .violations_here
        .iter()
        .filter(|v| !v.is_circular)
        .collect();

    // Group circular by order
    if !circular_violations.is_empty() {
        let mut by_order: std::collections::BTreeMap<usize, Vec<&ViolationInfo>> =
            std::collections::BTreeMap::new();
        for v in &circular_violations {
            by_order.entry(v.cycle_order).or_default().push(v);
        }

        violations_html.push_str("<h2>Circular Dependencies</h2>\n");
        for (order, violations) in &by_order {
            let label = match order {
                2 => "Mutual Dependencies (Order 2)".to_string(),
                3 => "Triangular Cycles (Order 3)".to_string(),
                _ => format!("Cycles of Order {}", order),
            };
            violations_html.push_str(&format!(
                "<h3>{} <small class=\"hop-file\">({} found)</small></h3>\n",
                label,
                violations.len()
            ));
            violations_html.push_str("<table class=\"violations\">\n");
            violations_html.push_str("<tr><th>Severity</th><th>Cycle</th></tr>\n");
            for v in violations {
                let sev_clr = "#ef4444";
                // Render cycle inline
                let cycle_content = render_cycle_details(v, data);
                violations_html.push_str(&format!(
                    "<tr><td><span class=\"severity\" style=\"color:{}\">{:.2}</span></td><td>{}</td></tr>\n",
                    sev_clr, v.severity, cycle_content,
                ));
            }
            violations_html.push_str("</table>\n");
        }
    }

    if !coupling_violations.is_empty() {
        violations_html.push_str("<h2>Coupling Violations</h2>\n<table class=\"violations\">\n");
        violations_html
            .push_str("<tr><th>Severity</th><th>From</th><th>To</th><th>Details</th></tr>\n");
        for v in &coupling_violations {
            let sev_clr = if v.severity >= data.critical_severity {
                "#ef4444"
            } else if v.severity >= 0.2 {
                "#eab308"
            } else {
                "#6b7280"
            };
            let from_short = std::path::Path::new(&v.from_module)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.from_module);
            let to_short = std::path::Path::new(&v.to_module)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.to_module);
            violations_html.push_str(&format!(
                "<tr>
                    <td><span class=\"severity\" style=\"color:{}\">{:.2}</span></td>
                    <td title=\"{}\">{}</td>
                    <td title=\"{}\">{}</td>
                    <td>Coupling</td>
                </tr>\n",
                sev_clr, v.severity, v.from_module, from_short, v.to_module, to_short,
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
</style>
</head>
<body>
<div class="breadcrumbs">{breadcrumbs}</div>
<h1>{title}</h1>
<p class="snapshot">Snapshot: {snapshot_id}</p>

<div class="summary">
    <div class="summary-card">
        <div class="label">Health Score</div>
        <div class="value score-big">{score:.1}</div>
    </div>
    <div class="summary-card">
        <div class="label">Modules</div>
        <div class="value">{modules}</div>
    </div>
    <div class="summary-card">
        <div class="label">Violations</div>
        <div class="value">{violations}</div>
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
        score_clr = score_clr,
        score = dir.score,
        modules = dir.module_count,
        violations = dir.violations_here.len(),
        children_rows = children_rows,
        violations_html = violations_html,
        version = super::VERSION,
    )
}

fn render_cycle_details(v: &ViolationInfo, _data: &ReportData) -> String {
    if v.cycle_path.is_empty() {
        return String::new();
    }

    // Short cycle display with file names
    let mut hops = String::new();
    for (i, dir) in v.cycle_path.iter().enumerate() {
        if i > 0 {
            hops.push_str(" &#8594; ");
        }
        let dir_short = std::path::Path::new(dir)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(dir);
        hops.push_str(&format!("<strong title=\"{}\">{}</strong>", dir, dir_short));
        if i < v.cycle_hop_files.len() {
            let (from_file, _, _) = &v.cycle_hop_files[i];
            let file_short = std::path::Path::new(from_file)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(from_file);
            hops.push_str(&format!(
                " <small class=\"hop-file\">({})</small>",
                file_short
            ));
        } else if i == v.cycle_path.len() - 1 && !v.cycle_hop_files.is_empty() {
            let (_, to_file, _) = &v.cycle_hop_files[v.cycle_hop_files.len() - 1];
            let file_short = std::path::Path::new(to_file)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(to_file);
            hops.push_str(&format!(
                " <small class=\"hop-file\">({})</small>",
                file_short
            ));
        }
    }

    // Full path details — one line per hop with XS count
    let mut full_paths = String::new();
    for (i, dir) in v.cycle_path.iter().enumerate() {
        let dir_short = std::path::Path::new(dir)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(dir);
        let xs_label = if i < v.cycle_hop_counts.len() {
            let count = v.cycle_hop_counts[i];
            format!(" <small class=\"hop-file\">(XS {})</small>", count)
        } else {
            String::new()
        };
        if i < v.cycle_hop_files.len() {
            let (from_file, _, _) = &v.cycle_hop_files[i];
            full_paths.push_str(&format!(
                "<strong>{}</strong>: {} &#8594;{}<br>",
                dir_short, from_file, xs_label
            ));
        } else if i == v.cycle_path.len() - 1 && !v.cycle_hop_files.is_empty() {
            let (_, to_file, _) = &v.cycle_hop_files[v.cycle_hop_files.len() - 1];
            full_paths.push_str(&format!("<strong>{}</strong>: {}<br>", dir_short, to_file));
        } else {
            full_paths.push_str(&format!("<strong>{}</strong><br>", dir_short));
        }
    }

    format!(
        "<details><summary class=\"cycle-path\">{}</summary>\
        <div class=\"full-paths\">{}</div></details>",
        hops, full_paths
    )
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
    use crate::analyzer::CouplingViolation;
    use crate::core::ModuleType;

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
    fn generates_html_files() {
        let modules = vec![
            make_module("a", "src/scanner/mod.rs"),
            make_module("b", "src/storage/mod.rs"),
        ];
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 2,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            suppressed_count: 0,
        };

        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        generate_html_report(&modules, &result, "snap-1", dir.path(), &settings).unwrap();

        assert!(dir.path().join("index.html").exists());
    }

    #[test]
    fn html_contains_score() {
        let modules = vec![make_module("a", "src/mod.rs")];
        let result = AuditResult {
            violations: vec![],
            score: 95.5,
            total_modules: 1,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            suppressed_count: 0,
        };

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
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 3,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            suppressed_count: 0,
        };

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
        let result = AuditResult {
            violations: vec![CouplingViolation {
                dir_a: "src/scanner".to_string(),
                dir_b: "src/storage".to_string(),
                from_module: "src/scanner/mod.rs".to_string(),
                to_module: "src/storage/mod.rs".to_string(),
                depth: 1,
                severity: 0.5,
                is_circular: false,
                cycle_path: Vec::new(),
                cycle_hop_files: Vec::new(),
                cycle_order: 0,
                cycle_hop_counts: Vec::new(),
                weakest_link: None,
                break_cost: 0,
                line_number: 0,
                weight: 0,
            }],
            score: 75.0,
            total_modules: 2,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            suppressed_count: 0,
        };

        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        generate_html_report(&modules, &result, "snap-1", dir.path(), &settings).unwrap();

        let html = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
        assert!(html.contains("Coupling Violations"));
        assert!(html.contains("mod.rs"));
    }
}
