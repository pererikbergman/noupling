//! The format-class contract (#350, epic #338, `CONTEXT.md` § Report formats).
//!
//! Runs the CLI end-to-end over `tests/fixtures/every_issue_kind`, a
//! project built so the audit produces at least one of every Issue kind,
//! then asserts the rule every format must obey:
//!
//! - an **Issue-listing format** (text, json, xml, sonar, md, html,
//!   dashboard, pr, briefing, explorer) carries every kind that has members
//!   — here, all nine;
//! - a **graph format** (mermaid, dot, bundle) accents every edge-shaped
//!   kind (Coupling, Cycle, Rule, Layer, Stability) and may omit node-shaped
//!   ones;
//! - the **trend format** (strategy) has a series per kind with a recorded
//!   count for the fixture's snapshot.
//!
//! Every check is against *data*, not prose: a format that names all nine
//! kinds in a static legend but emits no Issue fails here.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const ALL_KINDS: &[&str] = &[
    "coupling_violation",
    "cycle",
    "rule_violation",
    "layer_violation",
    "gravity_well",
    "red_flag",
    "stability_violation",
    "zone_flag",
    "low_cohesion",
];

const EDGE_SHAPED_KINDS: &[&str] = &[
    "coupling_violation",
    "cycle",
    "rule_violation",
    "layer_violation",
    "stability_violation",
];

/// Glossary names, index-aligned with `ALL_KINDS`.
const KIND_NAMES: &[&str] = &[
    "Coupling Violation",
    "Cycle",
    "Rule Violation",
    "Layer Violation",
    "Gravity Well",
    "Red Flag",
    "Stability Violation",
    "Zone Flag",
    "Low Cohesion",
];

fn kind_name(id: &str) -> &'static str {
    KIND_NAMES[ALL_KINDS.iter().position(|k| *k == id).expect("known kind")]
}

// ── Harness ──────────────────────────────────────────────────────────────

fn fixture_source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("every_issue_kind")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create dir");
    for entry in std::fs::read_dir(from).expect("read fixture dir") {
        let entry = entry.expect("dir entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy fixture file");
        }
    }
}

fn run_noupling(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_noupling"))
        .args(args)
        .output()
        .expect("run noupling");
    assert!(
        out.status.success(),
        "noupling {:?} failed:\n{}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Read one report artifact, failing loudly if the format did not produce it.
fn read_artifact(path: &Path) -> String {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("report artifact {} was not produced: {e}", path.display()));
    assert!(
        !content.is_empty(),
        "report artifact {} is empty",
        path.display()
    );
    content
}

/// Concatenate every file under a directory-style report (md, html).
fn read_tree(dir: &Path) -> String {
    assert!(
        dir.is_dir(),
        "report directory {} was not produced",
        dir.display()
    );
    let mut buf = String::new();
    for entry in std::fs::read_dir(dir).expect("read report dir") {
        let entry = entry.expect("dir entry");
        if entry.file_type().expect("file type").is_dir() {
            buf.push_str(&read_tree(&entry.path()));
        } else {
            buf.push_str(&read_artifact(&entry.path()));
        }
    }
    assert!(
        !buf.is_empty(),
        "report directory {} is empty",
        dir.display()
    );
    buf
}

/// Copy the fixture into a fresh tempdir so scans never touch the tracked tree.
fn prepare_fixture() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_dir(&fixture_source(), tmp.path());
    tmp
}

/// The Explorer bundles its field guide and help prose, which name every
/// Issue kind regardless of what the audit found. Match only the embedded
/// Data Contract so the row reflects data, not documentation.
fn explorer_contract(html: &str) -> String {
    let open = r#"<script id="noupling-data" type="application/json">"#;
    let start = html
        .find(open)
        .map(|i| i + open.len())
        .expect("explorer.html carries an embedded noupling-data contract");
    let end = html[start..]
        .find("</script>")
        .map(|j| start + j)
        .expect("noupling-data script is closed");
    html[start..end].to_string()
}

/// The `D = {…};` / `rawData = {…};` block a single-file HTML report
/// embeds. CRLF-safe.
fn embedded_json(html: &str, assignment: &str) -> serde_json::Value {
    let start = html
        .find(assignment)
        .unwrap_or_else(|| panic!("no `{assignment}` block"))
        + assignment.len();
    let rest = &html[start..];
    let end = rest
        .find(";\r")
        .or_else(|| rest.find(";\n"))
        .expect("assignment terminated");
    serde_json::from_str(&rest[..end]).expect("embedded JSON parses")
}

/// Kinds present in an `issues` array of Issue cards.
fn kinds_in_issue_cards(issues: &serde_json::Value) -> BTreeSet<String> {
    issues
        .as_array()
        .expect("issues array")
        .iter()
        .map(|i| i["kind"].as_str().expect("kind").to_string())
        .collect()
}

/// Kinds whose Issue card header appears in a card-rendering text format
/// (`[BAND] Kind:` in text/briefing, `### [BAND] Kind:` in md, a band chip
/// followed by the kind in html). Only real cards match; legends do not.
fn kinds_with_cards(output: &str, header_after_band: &str) -> BTreeSet<String> {
    ALL_KINDS
        .iter()
        .filter(|k| {
            ["CRITICAL", "HIGH", "MEDIUM", "LOW"]
                .iter()
                .any(|band| output.contains(&format!("{band}{header_after_band}{}:", kind_name(k))))
        })
        .map(|k| k.to_string())
        .collect()
}

fn assert_kinds(format: &str, got: &BTreeSet<String>, required: &[&str]) {
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|k| !got.contains(*k))
        .collect();
    assert!(
        missing.is_empty(),
        "{format} must carry {missing:?} (found {got:?})"
    );
}

// ── Tests ────────────────────────────────────────────────────────────────

/// The fixture's own contract: `audit` surfaces every Issue kind.
#[test]
fn fixture_audit_surfaces_every_issue_kind() {
    let tmp = prepare_fixture();
    let root = tmp.path().to_str().unwrap();
    run_noupling(&["scan", root]);
    let text = run_noupling(&["audit", root]);
    assert_kinds("text", &kinds_with_cards(&text, "] "), ALL_KINDS);
}

/// The format-class rule, asserted per format against its data.
#[test]
fn every_format_obeys_its_format_class_rule() {
    let tmp = prepare_fixture();
    let project = tmp.path();
    let root = project.to_str().unwrap();
    run_noupling(&["scan", root]);
    let text = run_noupling(&["audit", root]);
    run_noupling(&["report", "--format", "all", root]);
    run_noupling(&["report", "--format", "explorer", root]);
    let out = project.join(".noupling");
    let file = |name: &str| read_artifact(&out.join(name));

    // ── Issue-listing formats: every kind ──
    assert_kinds("text", &kinds_with_cards(&text, "] "), ALL_KINDS);

    let json: serde_json::Value = serde_json::from_str(&file("report.json")).unwrap();
    assert_kinds("json", &kinds_in_issue_cards(&json["issues"]), ALL_KINDS);
    for removed in [
        "coupling_violations",
        "circular_dependencies",
        "gravity_wells",
        "red_flags",
        "stability_violations",
        "distance",
        "cohesion",
    ] {
        assert!(
            json.get(removed).is_none(),
            "report.json must not carry the removed per-kind array `{removed}` (ADR 0002)"
        );
    }
    for kept in ["hotspots", "abstractness", "instability", "directory_tree"] {
        assert!(json.get(kept).is_some(), "Metric array `{kept}` stays");
    }

    let xml = file("report.xml");
    let xml_kinds: BTreeSet<String> = ALL_KINDS
        .iter()
        .filter(|k| xml.contains(&format!("<issue kind=\"{k}\"")))
        .map(|k| k.to_string())
        .collect();
    assert_kinds("xml", &xml_kinds, ALL_KINDS);

    let sonar: serde_json::Value = serde_json::from_str(&file("noupling-sonar.json")).unwrap();
    let sonar_kinds: BTreeSet<String> = sonar["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| {
            i["ruleId"]
                .as_str()?
                .strip_prefix("noupling:")
                .map(str::to_string)
        })
        .collect();
    assert_kinds("sonar", &sonar_kinds, ALL_KINDS);

    assert_kinds(
        "md",
        &kinds_with_cards(&read_tree(&out.join("report-md")), "] "),
        ALL_KINDS,
    );
    assert_kinds(
        "html",
        &kinds_with_cards(&read_tree(&out.join("report")), "</span> "),
        ALL_KINDS,
    );

    let dashboard = embedded_json(&file("dashboard.html"), "const D = ");
    assert_kinds(
        "dashboard",
        &kinds_in_issue_cards(&dashboard["issues"]),
        ALL_KINDS,
    );

    // pr summarises: every kind named with a non-zero count on the by-kind line.
    let pr = file("pr.md");
    let by_kind = pr
        .lines()
        .find(|l| l.starts_with("**By kind:**"))
        .expect("pr carries a by-kind line");
    let pr_kinds: BTreeSet<String> = ALL_KINDS
        .iter()
        .filter(|k| {
            by_kind
                .split(" · ")
                .any(|cell| cell.contains(kind_name(k)) && !cell.trim_end().ends_with(" 0"))
        })
        .map(|k| k.to_string())
        .collect();
    assert_kinds("pr", &pr_kinds, ALL_KINDS);

    // briefing: per-kind rows in the summary table.
    let briefing = file("briefing.md");
    let briefing_kinds: BTreeSet<String> = ALL_KINDS
        .iter()
        .filter(|k| briefing.contains(&format!("{} | ", kind_name(k))))
        .map(|k| k.to_string())
        .collect();
    assert_kinds("briefing", &briefing_kinds, ALL_KINDS);

    let explorer: serde_json::Value =
        serde_json::from_str(&explorer_contract(&file("explorer.html"))).unwrap();
    assert_kinds(
        "explorer",
        &kinds_in_issue_cards(&explorer["issues"]),
        ALL_KINDS,
    );
    assert!(explorer.get("gravity_wells").is_none() && explorer.get("red_flags").is_none());

    // ── Graph formats: every edge-shaped kind accented on the drawing ──
    // mermaid: one arrow shape per kind on edge lines (legend lines are `%%`).
    let mermaid = file("report.mermaid");
    let arrow = |k: &str| match k {
        "cycle" => " -.->",
        "rule_violation" => " --x",
        "layer_violation" => " ==>",
        "coupling_violation" => " -->",
        "stability_violation" => " --o",
        _ => unreachable!(),
    };
    let mermaid_kinds: BTreeSet<String> = EDGE_SHAPED_KINDS
        .iter()
        .filter(|k| {
            mermaid
                .lines()
                .any(|l| !l.trim_start().starts_with("%%") && l.contains(arrow(k)))
        })
        .map(|k| k.to_string())
        .collect();
    assert_kinds("mermaid", &mermaid_kinds, EDGE_SHAPED_KINDS);

    // dot: accented edges carry a tooltip naming the kind (legend edges do not).
    let dot = file("report.dot");
    let dot_kinds: BTreeSet<String> = EDGE_SHAPED_KINDS
        .iter()
        .filter(|k| dot.contains(&format!("tooltip=\"{}", kind_name(k))))
        .map(|k| k.to_string())
        .collect();
    assert_kinds("dot", &dot_kinds, EDGE_SHAPED_KINDS);

    let bundle = embedded_json(&file("bundle.html"), "const rawData = ");
    let bundle_kinds: BTreeSet<String> = bundle["issue_edges"]
        .as_array()
        .expect("issue_edges")
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect();
    assert_kinds("bundle", &bundle_kinds, EDGE_SHAPED_KINDS);

    // ── Trend format: a series per kind with a recorded, non-zero count ──
    let strategy = embedded_json(&file("strategy.html"), "const D = ");
    let strategy_kinds: BTreeSet<String> = strategy["issue_kind_series"]
        .as_array()
        .expect("issue_kind_series")
        .iter()
        .filter(|s| {
            s["counts"]
                .as_array()
                .map(|c| c.iter().any(|n| n.as_u64().unwrap_or(0) > 0))
                .unwrap_or(false)
        })
        .map(|s| s["kind"].as_str().unwrap().to_string())
        .collect();
    assert_kinds("strategy", &strategy_kinds, ALL_KINDS);
}

/// The Explorer's Issues are the same cards as every other format: the
/// Red Flag's band, reason and recommendation in the embedded contract
/// appear verbatim in the text report (#345).
#[test]
fn explorer_and_text_report_share_issue_wording() {
    let tmp = prepare_fixture();
    let root = tmp.path().to_str().unwrap();
    run_noupling(&["scan", root]);
    let text = run_noupling(&["audit", root]);
    run_noupling(&["report", "--format", "explorer", root]);
    let html = read_artifact(&tmp.path().join(".noupling").join("explorer.html"));
    let contract: serde_json::Value = serde_json::from_str(&explorer_contract(&html)).unwrap();

    let issues = contract["issues"].as_array().expect("issues array");
    assert_eq!(contract["format_version"], 2);
    let red_flag = issues
        .iter()
        .find(|i| i["kind"] == "red_flag")
        .expect("fixture has a Red Flag");
    let band = red_flag["severity"].as_str().unwrap().to_uppercase();
    let reason = red_flag["reason"].as_str().unwrap();
    let recommendation = red_flag["recommendation"].as_str().unwrap();
    assert!(
        text.contains(&format!("[{band}] Red Flag:")),
        "band differs:\n{text}"
    );
    assert!(
        text.contains(reason),
        "reason differs:\n{reason}\n---\n{text}"
    );
    assert!(
        text.contains(recommendation),
        "recommendation differs:\n{text}"
    );

    // Score-breakdown rows sum to the points lost, using score impact.
    let b = &contract["score_breakdown"];
    let sum: f64 = b["by_kind"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["points"].as_f64().unwrap())
        .sum();
    assert!(
        (sum - b["points_lost"].as_f64().unwrap()).abs() < 1e-6,
        "{b}"
    );
}
