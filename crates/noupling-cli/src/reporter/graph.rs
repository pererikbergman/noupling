//! Dependency graph generation in Mermaid and Graphviz DOT formats.

use std::collections::{BTreeMap, BTreeSet};

use crate::reporter::VERSION;
use noupling_core::analyzer::{parent_dir, AuditResult, CouplingViolation, IssueDetail, IssueKind};
use noupling_core::core::Module;

/// One accented edge on a graph drawing: an edge-shaped Issue projected
/// onto a `from → to` pair of paths (directories for Coupling, Cycle and
/// Stability; files for Rule and Layer). Shared by mermaid, dot and bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IssueEdge {
    pub from: String,
    pub to: String,
    pub kind: IssueKind,
    pub baselined: bool,
}

/// Every edge of every edge-shaped Issue: each hop of a Cycle ring, and
/// each Coupling, Rule, Layer and Stability Violation. Node-shaped kinds
/// contribute nothing (`CONTEXT.md` § Graph format). Exhaustive so a new
/// kind must say whether it draws.
pub(super) fn issue_edges(result: &AuditResult) -> Vec<IssueEdge> {
    let mut edges = Vec::new();
    for issue in result.issues() {
        let mut push = |from: &str, to: &str| {
            edges.push(IssueEdge {
                from: from.to_string(),
                to: to.to_string(),
                kind: issue.kind(),
                baselined: issue.baselined,
            })
        };
        match &issue.detail {
            IssueDetail::CouplingViolation(v) => push(&v.dir_a, &v.dir_b),
            IssueDetail::Cycle(v) => {
                for hop in v.cycle_path.windows(2) {
                    push(&hop[0], &hop[1]);
                }
            }
            IssueDetail::RuleViolation(r) => push(&r.from_module, &r.to_module),
            IssueDetail::LayerViolation(l) => push(&l.from_module, &l.to_module),
            IssueDetail::StabilityViolation(s) => push(&s.from_dir, &s.to_dir),
            IssueDetail::GravityWell(_)
            | IssueDetail::RedFlag(_)
            | IssueDetail::ZoneFlag(_)
            | IssueDetail::LowCohesion(_) => {}
        }
    }
    edges
}

/// The edge-shaped kinds a graph format draws, in legend order.
pub(super) const EDGE_KINDS: [IssueKind; 5] = [
    IssueKind::Cycle,
    IssueKind::RuleViolation,
    IssueKind::LayerViolation,
    IssueKind::CouplingViolation,
    IssueKind::StabilityViolation,
];

/// Muted colour for baselined edges of any kind.
const MUTED: &str = "#94a3b8";

/// Drawing style per edge-shaped kind: (mermaid arrow, dot attributes).
/// Colour is `IssueKind::accent_color`. Exhaustive so a new kind must say
/// how it is drawn; node-shaped kinds never reach a drawing (`issue_edges`
/// skips them) but must still name a style so the match stays total.
fn edge_style(kind: IssueKind) -> (&'static str, &'static str) {
    match kind {
        IssueKind::Cycle => ("-.->", "style=dashed"),
        IssueKind::RuleViolation => ("--x", "style=bold, arrowhead=box"),
        IssueKind::LayerViolation => ("==>", "penwidth=2, arrowhead=tee"),
        IssueKind::CouplingViolation => ("-->", "style=solid"),
        IssueKind::StabilityViolation => ("--o", "style=dotted"),
        IssueKind::GravityWell
        | IssueKind::RedFlag
        | IssueKind::ZoneFlag
        | IssueKind::LowCohesion => ("-->", "style=invis"),
    }
}

/// A drawn directory-level edge, one per (from, to, kind): how many
/// Issues of that kind it carries and whether every one is baselined.
/// A pair that is both a Coupling and a Stability Violation gets two
/// parallel edges, one per accent — every edge-shaped Issue is drawn.
struct EdgeInfo {
    count: usize,
    kind: IssueKind,
    baselined: bool,
}

/// Drawn-edge key: `(from, to, kind id)`. Kind id keeps `BTreeMap` order
/// deterministic and readable.
type EdgeKey = (String, String, &'static str);

fn build_dir_graph(
    modules: &[Module],
    result: &AuditResult,
) -> (BTreeSet<String>, BTreeMap<EdgeKey, EdgeInfo>) {
    // Collect all directories
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    for module in modules {
        if let Some(parent) = std::path::Path::new(&module.path).parent() {
            let dir = parent.to_string_lossy().to_string();
            if !dir.is_empty() {
                dirs.insert(dir);
            }
        }
    }

    // Build edges from every edge-shaped Issue, keyed by short directory
    // names. Rule and Layer Violations are file edges; they draw between
    // the files' directories.
    let mut edges: BTreeMap<EdgeKey, EdgeInfo> = BTreeMap::new();
    for e in issue_edges(result) {
        let (from_dir, to_dir) = match e.kind {
            IssueKind::RuleViolation | IssueKind::LayerViolation => {
                (parent_dir(&e.from), parent_dir(&e.to))
            }
            _ => (e.from.as_str(), e.to.as_str()),
        };
        // Nodes are keyed by basename, so two distinct directories sharing a
        // basename collapse onto one node; an edge between them would draw
        // as a self-loop asserting a directory imports itself. Drop those
        // along with genuine self-edges.
        let (from, to) = (short_name(from_dir), short_name(to_dir));
        if from_dir == to_dir || from == to {
            continue;
        }
        let entry = edges.entry((from, to, e.kind.id())).or_insert(EdgeInfo {
            count: 0,
            kind: e.kind,
            baselined: true,
        });
        entry.count += 1;
        entry.baselined &= e.baselined;
    }

    (dirs, edges)
}

/// Classify a directory as healthy, coupled, or circular based on violations.
fn node_status(dir_name: &str, violations: &[CouplingViolation]) -> &'static str {
    for v in violations {
        if v.is_circular {
            for p in &v.cycle_path {
                if short_name(p) == dir_name {
                    return "circular";
                }
            }
        }
        if short_name(&v.dir_a) == dir_name || short_name(&v.dir_b) == dir_name {
            return "coupled";
        }
    }
    "healthy"
}

fn short_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Generate a Mermaid flowchart diagram.
pub fn format_mermaid(modules: &[Module], result: &AuditResult) -> String {
    let (dirs, edges) = build_dir_graph(modules, result);
    let mut out = String::new();

    out.push_str(&format!("%% Generated by {}\n", VERSION));
    out.push_str("flowchart LR\n");

    // Collect unique short names from edges
    let mut used_dirs: BTreeSet<String> = BTreeSet::new();
    for (from, to, _) in edges.keys() {
        used_dirs.insert(from.clone());
        used_dirs.insert(to.clone());
    }

    // Add nodes without edges too (from directory list)
    for dir in &dirs {
        let name = short_name(dir);
        if !name.is_empty() {
            used_dirs.insert(name);
        }
    }

    // Define nodes with styling
    for dir_name in &used_dirs {
        let status = node_status(dir_name, &result.violations);
        let style = match status {
            "circular" => format!("    {}[{}]:::circular", sanitize(dir_name), dir_name),
            "coupled" => format!("    {}[{}]:::coupled", sanitize(dir_name), dir_name),
            _ => format!("    {}[{}]:::healthy", sanitize(dir_name), dir_name),
        };
        out.push_str(&style);
        out.push('\n');
    }
    out.push('\n');

    // Define edges: one arrow shape per kind, colour via linkStyle by
    // index, muted grey when every Issue on the edge is baselined.
    let mut link_styles = Vec::new();
    for (idx, ((from, to, _), info)) in edges.iter().enumerate() {
        let label = if info.count > 1 {
            format!("|{}|", info.count)
        } else {
            String::new()
        };
        let (arrow, _) = edge_style(info.kind);
        let colour = info.kind.accent_color();
        out.push_str(&format!(
            "    {} {}{} {}\n",
            sanitize(from),
            arrow,
            label,
            sanitize(to)
        ));
        let stroke = if info.baselined { MUTED } else { colour };
        link_styles.push(format!(
            "    linkStyle {} stroke:{},stroke-width:{}\n",
            idx,
            stroke,
            if info.baselined { "1px" } else { "2px" }
        ));
    }
    out.push('\n');
    for ls in link_styles {
        out.push_str(&ls);
    }
    out.push('\n');

    // Legend
    out.push_str("    %% Legend — edge accents by Issue kind:\n");
    for kind in EDGE_KINDS {
        let (arrow, _) = edge_style(kind);
        let colour = kind.accent_color();
        out.push_str(&format!(
            "    %%   {}  {}  ({})\n",
            arrow,
            kind.name(),
            colour
        ));
    }
    out.push_str(&format!(
        "    %%   baselined Issues of any kind are drawn in {} (muted)\n",
        MUTED
    ));

    // Styles
    out.push_str("    classDef healthy fill:#dcfce7,stroke:#22c55e,color:#166534\n");
    out.push_str("    classDef coupled fill:#fef9c3,stroke:#eab308,color:#854d0e\n");
    out.push_str("    classDef circular fill:#fecaca,stroke:#ef4444,color:#991b1b\n");

    out
}

/// Generate a Graphviz DOT diagram.
pub fn format_dot(modules: &[Module], result: &AuditResult) -> String {
    let (dirs, edges) = build_dir_graph(modules, result);
    let mut out = String::new();

    out.push_str(&format!("// Generated by {}\n", VERSION));
    out.push_str("digraph noupling {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=box, style=filled, fontname=\"Helvetica\"];\n\n");

    // Collect used dirs
    let mut used_dirs: BTreeSet<String> = BTreeSet::new();
    for (from, to, _) in edges.keys() {
        used_dirs.insert(from.clone());
        used_dirs.insert(to.clone());
    }
    for dir in &dirs {
        let name = short_name(dir);
        if !name.is_empty() {
            used_dirs.insert(name);
        }
    }

    // Nodes
    for dir_name in &used_dirs {
        let status = node_status(dir_name, &result.violations);
        let (fill, font) = match status {
            "circular" => ("#fecaca", "#991b1b"),
            "coupled" => ("#fef9c3", "#854d0e"),
            _ => ("#dcfce7", "#166534"),
        };
        out.push_str(&format!(
            "    {} [label=\"{}\", fillcolor=\"{}\", fontcolor=\"{}\"];\n",
            sanitize(dir_name),
            dir_name,
            fill,
            font
        ));
    }
    out.push('\n');

    // Edges: per-kind style; muted when every Issue on the edge is baselined.
    for ((from, to, _), info) in &edges {
        let (_, dot_style) = edge_style(info.kind);
        let colour = info.kind.accent_color();
        let mut attrs = vec![dot_style.to_string()];
        if info.count > 1 {
            attrs.push(format!("label=\"{}\"", info.count));
        }
        if info.baselined {
            attrs.push(format!("color=\"{}\"", MUTED));
            attrs.push("penwidth=0.7".to_string());
            attrs.push(format!("tooltip=\"{} (baselined)\"", info.kind.name()));
        } else {
            attrs.push(format!("color=\"{}\"", colour));
            attrs.push(format!("tooltip=\"{}\"", info.kind.name()));
        }
        out.push_str(&format!(
            "    {} -> {} [{}];\n",
            sanitize(from),
            sanitize(to),
            attrs.join(", ")
        ));
    }

    // Legend: one sample edge per kind, plus a muted baselined sample.
    out.push_str("\n    // Legend — edge accents by Issue kind\n");
    out.push_str("    subgraph cluster_legend {\n");
    out.push_str("        label=\"Legend\"; fontsize=10; color=\"#cbd5e1\";\n");
    out.push_str("        node [shape=plaintext, style=\"\", fontsize=9];\n");
    for (i, kind) in EDGE_KINDS.iter().enumerate() {
        let (_, dot_style) = edge_style(*kind);
        let colour = kind.accent_color();
        out.push_str(&format!(
            "        l{i}a [label=\"\"]; l{i}b [label=\"{}\"]; l{i}a -> l{i}b [{}, color=\"{}\"];\n",
            kind.name(),
            dot_style,
            colour
        ));
    }
    out.push_str(&format!(
        "        lma [label=\"\"]; lmb [label=\"baselined (any kind)\"]; lma -> lmb [color=\"{}\", penwidth=0.7];\n",
        MUTED
    ));
    out.push_str("    }\n");

    out.push_str("}\n");
    out
}

/// Sanitize a name for use as a Mermaid/DOT identifier.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '-' || c == '.' || c == ' ' {
                '_'
            } else {
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use noupling_core::analyzer::AuditResultBuilder;
    use noupling_core::core::ModuleType;

    fn make_module(path: &str) -> Module {
        Module {
            id: path.to_string(),
            snapshot_id: "snap".to_string(),
            parent_id: None,
            name: std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            path: path.to_string(),
            module_type: ModuleType::File,
            depth: 1,
        }
    }

    fn make_violation(dir_a: &str, dir_b: &str) -> CouplingViolation {
        CouplingViolation {
            dir_a: dir_a.to_string(),
            dir_b: dir_b.to_string(),
            from_module: format!("{}/mod.rs", dir_a),
            to_module: format!("{}/mod.rs", dir_b),
            line_number: 1,
            depth: 1,
            weight: 1,
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
        }
    }

    #[test]
    fn mermaid_generates_valid_output() {
        let modules = vec![
            make_module("src/scanner/mod.rs"),
            make_module("src/core/mod.rs"),
        ];
        let result = AuditResultBuilder::new()
            .with_violations(vec![make_violation("src/scanner", "src/core")])
            .with_score(75.0)
            .with_total_modules(2)
            .build();

        let mermaid = format_mermaid(&modules, &result);
        assert!(mermaid.contains("flowchart LR"));
        assert!(mermaid.contains("scanner"));
        assert!(mermaid.contains("core"));
        assert!(mermaid.contains("-->"));
        assert!(mermaid.contains("classDef healthy"));
    }

    #[test]
    fn dot_generates_valid_output() {
        let modules = vec![
            make_module("src/scanner/mod.rs"),
            make_module("src/core/mod.rs"),
        ];
        let result = AuditResultBuilder::new()
            .with_violations(vec![make_violation("src/scanner", "src/core")])
            .with_score(75.0)
            .with_total_modules(2)
            .build();

        let dot = format_dot(&modules, &result);
        assert!(dot.contains("digraph noupling"));
        assert!(dot.contains("scanner"));
        assert!(dot.contains("core"));
        assert!(dot.contains("->"));
        assert!(dot.contains("fillcolor"));
    }

    #[test]
    fn empty_violations_still_generates() {
        let modules = vec![make_module("src/scanner/mod.rs")];
        let result = AuditResultBuilder::new().with_total_modules(1).build();

        let mermaid = format_mermaid(&modules, &result);
        assert!(mermaid.contains("flowchart LR"));

        let dot = format_dot(&modules, &result);
        assert!(dot.contains("digraph noupling"));
    }

    /// One of each edge-shaped kind, with the coupling edge baselined.
    fn every_edge_kind() -> AuditResult {
        use noupling_core::analyzer::{LayerViolation, RuleViolation, StabilityViolation};
        use noupling_core::baseline::Baseline;
        let mut cycle = make_violation("src/ring/alpha", "src/ring/beta");
        cycle.is_circular = true;
        cycle.direction = noupling_core::analyzer::DependencyDirection::Circular;
        cycle.cycle_order = 2;
        cycle.cycle_path = vec![
            "src/ring/alpha".into(),
            "src/ring/beta".into(),
            "src/ring/alpha".into(),
        ];
        let mut result = AuditResultBuilder::new()
            .with_total_modules(8)
            .with_violations(vec![cycle, make_violation("src/loose/x", "src/loose/y")])
            .with_rule_violations(vec![RuleViolation {
                from_module: "src/plugins/exporter.rs".into(),
                to_module: "src/legacy/compat.rs".into(),
                line_number: 2,
                message: "no".into(),
            }])
            .with_layer_violations(vec![LayerViolation {
                from_module: "src/infra/db.rs".into(),
                to_module: "src/ui/screen.rs".into(),
                line_number: 2,
                from_layer: "infra".into(),
                to_layer: "ui".into(),
            }])
            .with_stability_violations(vec![StabilityViolation {
                from_dir: "src/stable".into(),
                to_dir: "src/volatile".into(),
                from_instability: 0.5,
                to_instability: 0.67,
            }])
            .build();
        result.apply_baseline(&Baseline {
            fingerprints: ["coupling_violation:src/loose/x -> src/loose/y".to_string()]
                .into_iter()
                .collect(),
            legacy_format: false,
        });
        result
    }

    /// The shared edge model: one accented edge per edge-shaped Issue,
    /// node-shaped kinds contribute nothing, baselined carried through.
    #[test]
    fn issue_edges_cover_every_edge_shaped_kind_and_skip_node_shaped_ones() {
        use noupling_core::analyzer::IssueKind;
        let edges = issue_edges(&every_edge_kind());
        let kind_of = |from: &str| {
            edges
                .iter()
                .find(|e| e.from == from)
                .unwrap_or_else(|| panic!("no edge from {from}: {edges:?}"))
        };
        assert_eq!(kind_of("src/ring/alpha").kind, IssueKind::Cycle);
        assert_eq!(
            kind_of("src/ring/beta").kind,
            IssueKind::Cycle,
            "every hop of the ring"
        );
        assert_eq!(kind_of("src/loose/x").kind, IssueKind::CouplingViolation);
        assert!(kind_of("src/loose/x").baselined);
        // Rule and Layer edges keep their file paths; renderers decide the level.
        assert_eq!(
            kind_of("src/plugins/exporter.rs").kind,
            IssueKind::RuleViolation
        );
        assert_eq!(kind_of("src/infra/db.rs").kind, IssueKind::LayerViolation);
        assert_eq!(kind_of("src/stable").kind, IssueKind::StabilityViolation);
        assert_eq!(edges.len(), 6);
    }

    /// Two directories sharing a basename collapse onto one node; an edge
    /// between them must not be drawn as a self-loop.
    #[test]
    fn edges_between_same_basename_directories_are_not_drawn_as_self_loops() {
        use noupling_core::analyzer::RuleViolation;
        let result = AuditResultBuilder::new()
            .with_total_modules(2)
            .with_rule_violations(vec![RuleViolation {
                from_module: "src/api/models/user.rs".into(),
                to_module: "src/db/models/user.rs".into(),
                line_number: 1,
                message: "no".into(),
            }])
            .build();
        let mermaid = format_mermaid(&[], &result);
        assert!(!mermaid.contains("models --x models"), "{mermaid}");
        let dot = format_dot(&[], &result);
        assert!(!dot.contains("models -> models"), "{dot}");
    }

    #[test]
    fn mermaid_accents_each_edge_kind_with_its_own_style_and_a_legend() {
        let mermaid = format_mermaid(&[], &every_edge_kind());
        assert!(mermaid.contains("%% Legend"), "{mermaid}");
        for kind in [
            "Cycle",
            "Coupling Violation",
            "Rule Violation",
            "Layer Violation",
            "Stability Violation",
        ] {
            assert!(mermaid.contains(kind), "legend must name {kind}: {mermaid}");
        }
        // Distinct arrow per kind.
        assert!(
            mermaid.contains("alpha -.-> beta"),
            "cycle dashed: {mermaid}"
        );
        assert!(
            mermaid.contains("plugins --x legacy"),
            "rule cross: {mermaid}"
        );
        assert!(mermaid.contains("infra ==> ui"), "layer thick: {mermaid}");
        assert!(
            mermaid.contains("stable --o volatile"),
            "stability circle: {mermaid}"
        );
        assert!(mermaid.contains("x --> y"), "coupling plain: {mermaid}");
        // Baselined edge is muted grey via linkStyle.
        assert!(
            mermaid.contains("stroke:#94a3b8"),
            "muted baselined edge: {mermaid}"
        );
    }

    #[test]
    fn dot_accents_each_edge_kind_with_its_own_style_and_a_legend() {
        let dot = format_dot(&[], &every_edge_kind());
        assert!(dot.contains("// Legend"), "{dot}");
        assert!(
            dot.contains("alpha -> beta [") && dot.contains("style=dashed"),
            "{dot}"
        );
        assert!(
            dot.contains("plugins -> legacy [") && dot.contains("arrowhead=box"),
            "{dot}"
        );
        assert!(
            dot.contains("infra -> ui [") && dot.contains("arrowhead=tee"),
            "{dot}"
        );
        assert!(
            dot.contains("stable -> volatile [") && dot.contains("style=dotted"),
            "{dot}"
        );
        assert!(
            dot.contains("x -> y [") && dot.contains("color=\"#94a3b8\""),
            "baselined muted: {dot}"
        );
        assert!(dot.contains("subgraph cluster_legend"), "{dot}");
    }
}
