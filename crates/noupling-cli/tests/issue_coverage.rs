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

use noupling_core::analyzer::IssueKind;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The kinds a graph format must accent (`CONTEXT.md` § Edge-shaped Issue).
const EDGE_SHAPED_KINDS: [IssueKind; 5] = [
    IssueKind::CouplingViolation,
    IssueKind::Cycle,
    IssueKind::RuleViolation,
    IssueKind::LayerViolation,
    IssueKind::StabilityViolation,
];

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

/// Copy the fixture into a fresh tempdir so scans never touch the tracked tree.
fn prepare_fixture() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_dir(&fixture_source(), tmp.path());
    tmp
}

/// The Explorer bundles its field guide and help prose, which name every
/// Issue kind regardless of what the audit found. Match only the embedded
/// Data Contract so the row reflects data, not documentation.
fn explorer_contract(html: &str) -> serde_json::Value {
    let open = r#"<script id="noupling-data" type="application/json">"#;
    let start = html
        .find(open)
        .map(|i| i + open.len())
        .expect("explorer.html carries an embedded noupling-data contract");
    let end = html[start..]
        .find("</script>")
        .map(|j| start + j)
        .expect("noupling-data script is closed");
    serde_json::from_str(&html[start..end]).expect("contract parses")
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

/// Kind ids present in an `issues` array of Issue cards.
fn kinds_in_issue_cards(issues: &serde_json::Value) -> BTreeSet<String> {
    issues
        .as_array()
        .expect("issues array")
        .iter()
        .map(|i| i["kind"].as_str().expect("kind").to_string())
        .collect()
}

/// Assert that `present(kind)` holds for every required kind.
fn assert_each_kind(format: &str, required: &[IssueKind], present: impl Fn(IssueKind) -> bool) {
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|k| !present(*k))
        .map(IssueKind::id)
        .collect();
    assert!(missing.is_empty(), "{format} must carry {missing:?}");
}

/// A card-rendering text format shows `<BAND><sep><Kind name>:` per card
/// (`[HIGH] Cycle:` in text / md / briefing, a band chip then the kind in
/// html). Only real cards match; legends and count tables do not.
fn has_card(output: &str, sep: &str, kind: IssueKind) -> bool {
    ["CRITICAL", "HIGH", "MEDIUM", "LOW"]
        .iter()
        .any(|band| output.contains(&format!("{band}{sep}{}:", kind.name())))
}

// ── Tests ────────────────────────────────────────────────────────────────

/// The format-class rule, asserted per format against its data. The first
/// assertion doubles as the fixture's own contract: `audit` surfaces every
/// Issue kind.
#[test]
fn every_format_obeys_its_format_class_rule() {
    let tmp = prepare_fixture();
    let project = tmp.path();
    let root = project.to_str().unwrap();
    run_noupling(&["scan", root]);
    run_noupling(&["report", "--format", "all", root]);
    run_noupling(&["report", "--format", "explorer", root]);
    let out = project.join(".noupling");
    let file = |name: &str| read_artifact(&out.join(name));
    let all = &IssueKind::ALL;

    // ── Issue-listing formats: every kind ──
    // report.txt is the same text `audit` prints (the Text adapter).
    let text = file("report.txt");
    assert_each_kind("text", all, |k| has_card(&text, "] ", k));

    let json: serde_json::Value = serde_json::from_str(&file("report.json")).unwrap();
    let json_kinds = kinds_in_issue_cards(&json["issues"]);
    assert_each_kind("json", all, |k| json_kinds.contains(k.id()));
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
    // Header counts agree with the cards (#358, #380): a ring hop is part
    // of its Cycle, never a second critical violation, and "critical" means
    // the card's band. (The fixture's critical cards are critical by raw
    // severity too, so this guards the definition, not the hop folding —
    // the unit test in reporter/mod.rs does that.)
    let critical_cards = json["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| {
            matches!(i["kind"].as_str(), Some("coupling_violation" | "cycle"))
                && i["severity"] == "critical"
        })
        .count();
    assert_eq!(
        json["critical_violations"].as_u64().unwrap() as usize,
        critical_cards,
        "critical_violations header must equal the critical Coupling Violation + Cycle cards"
    );

    // A band never contradicts the score impact (#379): an Issue costing
    // ≥ 10 points is critical, ≥ 5 at least high, ≥ 1 at least medium.
    let rank = |b: &str| {
        ["low", "medium", "high", "critical"]
            .iter()
            .position(|x| *x == b)
            .unwrap()
    };
    for issue in json["issues"].as_array().unwrap() {
        let points = issue["score_impact"].as_f64().unwrap();
        let band = issue["severity"].as_str().unwrap();
        let floor = if points >= 10.0 {
            "critical"
        } else if points >= 5.0 {
            "high"
        } else if points >= 1.0 {
            "medium"
        } else {
            "low"
        };
        assert!(
            rank(band) >= rank(floor),
            "{} {} costs {points:.1} points but is banded {band} (floor {floor})",
            issue["kind"],
            issue["subject"]
        );
    }

    let xml = file("report.xml");
    assert_each_kind("xml", all, |k| {
        xml.contains(&format!("<issue kind=\"{}\"", k.id()))
    });

    let sonar: serde_json::Value = serde_json::from_str(&file("noupling-sonar.json")).unwrap();
    let rule_ids: BTreeSet<String> = sonar["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["ruleId"].as_str().map(str::to_string))
        .collect();
    assert_each_kind("sonar", all, |k| {
        rule_ids.contains(&format!("noupling:{}", k.id()))
    });

    // Directory-tree reports: the root page must carry every kind itself.
    let md = file("report-md/README.md");
    assert_each_kind("md", all, |k| has_card(&md, "] ", k));
    let html = file("report/index.html");
    assert_each_kind("html", all, |k| has_card(&html, "</span> ", k));

    let dashboard = embedded_json(&file("dashboard.html"), "const D = ");
    let dashboard_kinds = kinds_in_issue_cards(&dashboard["issues"]);
    assert_each_kind("dashboard", all, |k| dashboard_kinds.contains(k.id()));

    // pr summarises: every kind named with a non-zero count on the by-kind line.
    let pr = file("pr.md");
    let by_kind = pr
        .lines()
        .find(|l| l.starts_with("**By kind:**"))
        .expect("pr carries a by-kind line");
    assert_each_kind("pr", all, |k| {
        by_kind
            .split(" · ")
            .any(|cell| cell.contains(k.name()) && !cell.trim_end().ends_with(" 0"))
    });

    // briefing: per-kind rows in the summary table.
    let briefing = file("briefing.md");
    assert_each_kind("briefing", all, |k| {
        briefing.contains(&format!("{} | ", k.name()))
    });

    let explorer = explorer_contract(&file("explorer.html"));
    let explorer_kinds = kinds_in_issue_cards(&explorer["issues"]);
    assert_each_kind("explorer", all, |k| explorer_kinds.contains(k.id()));
    assert!(explorer.get("gravity_wells").is_none() && explorer.get("red_flags").is_none());

    // ── Graph formats: every edge-shaped kind accented on the drawing ──
    // mermaid: one arrow shape per kind on edge lines (legend lines are `%%`).
    let mermaid = file("report.mermaid");
    let arrow = |k: IssueKind| match k {
        IssueKind::Cycle => " -.->",
        IssueKind::RuleViolation => " --x",
        IssueKind::LayerViolation => " ==>",
        IssueKind::CouplingViolation => " -->",
        IssueKind::StabilityViolation => " --o",
        _ => unreachable!("node-shaped kinds are not drawn"),
    };
    assert_each_kind("mermaid", &EDGE_SHAPED_KINDS, |k| {
        mermaid
            .lines()
            .any(|l| !l.trim_start().starts_with("%%") && l.contains(arrow(k)))
    });

    // dot: accented edges carry a tooltip naming the kind (legend edges do not).
    let dot = file("report.dot");
    assert_each_kind("dot", &EDGE_SHAPED_KINDS, |k| {
        dot.contains(&format!("tooltip=\"{}", k.name()))
    });

    let bundle = embedded_json(&file("bundle.html"), "const rawData = ");
    let bundle_kinds: BTreeSet<String> = bundle["issue_edges"]
        .as_array()
        .expect("issue_edges")
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect();
    assert_each_kind("bundle", &EDGE_SHAPED_KINDS, |k| {
        bundle_kinds.contains(k.id())
    });

    // ── Trend format: a series per kind with a recorded, non-zero count ──
    let strategy = embedded_json(&file("strategy.html"), "const D = ");
    let series = strategy["issue_kind_series"]
        .as_array()
        .expect("issue_kind_series");
    assert_each_kind("strategy", all, |k| {
        series.iter().any(|s| {
            s["kind"] == k.id()
                && s["counts"]
                    .as_array()
                    .is_some_and(|c| c.iter().any(|n| n.as_u64().unwrap_or(0) > 0))
        })
    });
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
    let contract = explorer_contract(&html);

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
