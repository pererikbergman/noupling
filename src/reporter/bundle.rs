//! Zoomable sunburst with aggregated dependency edges.

use crate::analyzer::AuditResult;
use crate::core::{Dependency, Module};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Serialize)]
struct SunburstNode {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<usize>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    has_cycle_in_subtree: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<SunburstNode>,
}

#[derive(Serialize)]
struct DepEdge {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct BundleData {
    tree: SunburstNode,
    deps: Vec<DepEdge>,
    violation_deps: Vec<DepEdge>,
}

pub fn generate_bundle_report(
    modules: &[Module],
    dependencies: &[Dependency],
    result: &AuditResult,
    output_path: &std::path::Path,
) -> anyhow::Result<()> {
    let data = build_data(modules, dependencies, result);
    let json = serde_json::to_string(&data)?;

    let html = format!(
        include_str!("bundle_template.html"),
        json = json,
        version = super::VERSION,
        module_count = modules.len(),
        dep_count = dependencies.len(),
        violation_count = result.violations.len(),
        score = format!("{:.1}", result.score),
    );

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, html)?;
    Ok(())
}

fn build_data(modules: &[Module], dependencies: &[Dependency], result: &AuditResult) -> BundleData {
    let id_to_path: HashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    // Find common prefix to strip
    let paths: Vec<&str> = modules.iter().map(|m| m.path.as_str()).collect();
    let common = find_common_path_prefix(&paths);

    // Build hierarchy tree
    let mut dir_files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in modules {
        let rel = strip_path_prefix(&m.path, &common);
        let parent = std::path::Path::new(&rel)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let file_name = std::path::Path::new(&rel)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.clone());
        dir_files.entry(parent).or_default().push(file_name);
    }

    let tree = build_tree(&dir_files);

    // Compute per-directory scores from violations.
    // Each module is counted in every ancestor directory, and each violation
    // is assigned to the common parent of its two directories (matching the
    // HTML report's logic so colors are meaningful).
    let mut dir_violation_severity: HashMap<String, f64> = HashMap::new();
    let mut dir_module_count: HashMap<String, usize> = HashMap::new();
    for m in modules {
        let rel = strip_path_prefix(&m.path, &common);
        let parts: Vec<&str> = rel.split('/').collect();
        // Count this module in every ancestor directory
        for end in 1..parts.len() {
            let dir = parts[..end].join("/");
            *dir_module_count.entry(dir).or_insert(0) += 1;
        }
    }
    for v in &result.violations {
        let a = strip_path_prefix(&v.dir_a, &common);
        let b = strip_path_prefix(&v.dir_b, &common);
        let parent = common_parent_of(&a, &b);
        if !parent.is_empty() {
            *dir_violation_severity.entry(parent).or_insert(0.0) += v.severity;
        }
    }

    let mut tree = apply_scores(tree, &dir_violation_severity, &dir_module_count);

    // Mark every ancestor directory of a circular violation so the user can
    // visually trace which subtrees contain cycles, even when the score for
    // that directory is otherwise green.
    let mut cycle_ancestors: HashSet<String> = HashSet::new();
    for v in &result.violations {
        if !v.is_circular {
            continue;
        }
        let a = strip_path_prefix(&v.dir_a, &common);
        let b = strip_path_prefix(&v.dir_b, &common);
        for path in [a.as_str(), b.as_str()] {
            let parts: Vec<&str> = path.split('/').collect();
            for end in 1..=parts.len() {
                cycle_ancestors.insert(parts[..end].join("/"));
            }
        }
    }
    fn mark_cycles(node: &mut SunburstNode, path: &str, ancestors: &HashSet<String>) {
        let full_path = if path.is_empty() {
            node.name.clone()
        } else if node.name == "root" {
            String::new()
        } else {
            format!("{}/{}", path, node.name)
        };
        if ancestors.contains(&full_path) {
            node.has_cycle_in_subtree = true;
        }
        let child_path = if node.name == "root" {
            String::new()
        } else {
            full_path
        };
        for child in &mut node.children {
            mark_cycles(child, &child_path, ancestors);
        }
    }
    mark_cycles(&mut tree, "", &cycle_ancestors);

    // Build cross-directory dependency edges (file-level, stripped paths)
    let mut deps: Vec<DepEdge> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for dep in dependencies {
        let from = id_to_path.get(dep.from_module_id.as_str());
        let to = id_to_path.get(dep.to_module_id.as_str());
        if let (Some(&f), Some(&t)) = (from, to) {
            let f_rel = strip_path_prefix(f, &common);
            let t_rel = strip_path_prefix(t, &common);
            let f_dir = std::path::Path::new(&f_rel)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let t_dir = std::path::Path::new(&t_rel)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            if f_dir != t_dir && seen.insert((f_rel.clone(), t_rel.clone())) {
                deps.push(DepEdge {
                    from: f_rel,
                    to: t_rel,
                });
            }
        }
    }

    // Violation-specific edges
    let mut violation_deps: Vec<DepEdge> = Vec::new();
    let mut v_seen: HashSet<(String, String)> = HashSet::new();
    for v in &result.violations {
        if !v.is_circular {
            let f = strip_path_prefix(&v.from_module, &common);
            let t = strip_path_prefix(&v.to_module, &common);
            if v_seen.insert((f.clone(), t.clone())) {
                violation_deps.push(DepEdge { from: f, to: t });
            }
        } else {
            for (from_file, to_file, _) in &v.cycle_hop_files {
                if !from_file.is_empty() && !to_file.is_empty() {
                    let f = strip_path_prefix(from_file, &common);
                    let t = strip_path_prefix(to_file, &common);
                    if v_seen.insert((f.clone(), t.clone())) {
                        violation_deps.push(DepEdge { from: f, to: t });
                    }
                }
            }
        }
    }

    BundleData {
        tree,
        deps,
        violation_deps,
    }
}

fn build_tree(dir_files: &BTreeMap<String, Vec<String>>) -> SunburstNode {
    let mut root = SunburstNode {
        name: "root".to_string(),
        score: None,
        has_cycle_in_subtree: false,
        value: None,
        children: Vec::new(),
    };

    // Collect all directory paths
    let mut all_dirs: HashSet<String> = HashSet::new();
    for dir in dir_files.keys() {
        if dir.is_empty() {
            continue;
        }
        let mut current = String::new();
        for part in dir.split('/') {
            if current.is_empty() {
                current = part.to_string();
            } else {
                current = format!("{}/{}", current, part);
            }
            all_dirs.insert(current.clone());
        }
    }

    // Build nodes for each directory
    fn ensure_dir<'a>(root: &'a mut SunburstNode, path: &str) -> &'a mut SunburstNode {
        if path.is_empty() {
            return root;
        }
        let parts: Vec<&str> = path.split('/').collect();
        let mut current = root;
        for part in parts {
            let idx = current.children.iter().position(|c| c.name == part);
            if let Some(i) = idx {
                current = &mut current.children[i];
            } else {
                current.children.push(SunburstNode {
                    name: part.to_string(),
                    score: None,
                    has_cycle_in_subtree: false,
                    value: None,
                    children: Vec::new(),
                });
                let len = current.children.len();
                current = &mut current.children[len - 1];
            }
        }
        current
    }

    for dir in &all_dirs {
        ensure_dir(&mut root, dir);
    }

    // Add files as leaves
    for (dir, files) in dir_files {
        let parent = ensure_dir(&mut root, dir);
        for file in files {
            parent.children.push(SunburstNode {
                name: file.clone(),
                score: None,
                has_cycle_in_subtree: false,
                value: Some(1),
                children: Vec::new(),
            });
        }
    }

    // Sort children at each level
    fn sort_tree(node: &mut SunburstNode) {
        node.children.sort_by(|a, b| a.name.cmp(&b.name));
        for child in &mut node.children {
            sort_tree(child);
        }
    }
    sort_tree(&mut root);

    root
}

fn apply_scores(
    mut tree: SunburstNode,
    severity_map: &HashMap<String, f64>,
    count_map: &HashMap<String, usize>,
) -> SunburstNode {
    fn apply(
        node: &mut SunburstNode,
        path: &str,
        sev: &HashMap<String, f64>,
        cnt: &HashMap<String, usize>,
    ) {
        let full_path = if path.is_empty() {
            node.name.clone()
        } else if node.name == "root" {
            String::new()
        } else {
            format!("{}/{}", path, node.name)
        };

        // Always assign a score for non-leaf nodes so the sunburst is fully colored
        if let Some(&modules) = cnt.get(&full_path) {
            let s = sev.get(&full_path).copied().unwrap_or(0.0);
            node.score = Some((100.0 * (1.0 - s / (modules as f64).max(1.0))).clamp(0.0, 100.0));
        }

        let child_path = if node.name == "root" {
            String::new()
        } else {
            full_path
        };
        for child in &mut node.children {
            apply(child, &child_path, sev, cnt);
        }
    }
    apply(&mut tree, "", severity_map, count_map);
    tree
}

fn find_common_path_prefix(paths: &[&str]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let first: Vec<&str> = paths[0].split('/').collect();
    let mut len = first.len().saturating_sub(1); // exclude file name
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

/// Return the deepest common parent directory of two paths.
/// e.g. ("a/b/c", "a/b/d") -> "a/b"
fn common_parent_of(a: &str, b: &str) -> String {
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

fn strip_path_prefix(path: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        return path.to_string();
    }
    let with_slash = format!("{}/", prefix);
    path.strip_prefix(&with_slash).unwrap_or(path).to_string()
}
