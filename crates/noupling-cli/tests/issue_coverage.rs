//! Issue-kind coverage across every report format (#339, epic #338).
//!
//! Runs the CLI end-to-end over `tests/fixtures/every_issue_kind`, a
//! project built so the audit produces at least one of every Issue kind
//! (see `CONTEXT.md` § Issue kinds), then records which kinds each
//! format's output mentions.
//!
//! `EXPECTED` pins the matrix. It starts at today's coverage so it is
//! green on day one; a format ticket in #338 flips its row as it
//! migrates to `issues()`, and the contract ticket (#350) replaces the
//! table with the format-class rule from `CONTEXT.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Issue kinds and how to spot them in an output ────────────────────────
//
// Markers are lowercase substrings; a kind counts as "mentioned" if any
// marker appears in the lowercased output. They deliberately accept the
// current per-format wording (text headings, JSON keys, explorer
// contract fields). Once Issue cards exist (#340) every listing format
// prints the glossary kind name, and these can tighten to that.

const KINDS: &[(&str, &[&str])] = &[
    ("coupling_violation", &["sibling", "\u{2194}"]),
    ("cycle", &["circular", "cycle", "\u{21bb}"]),
    (
        "rule_violation",
        &["rule violation", "rule_violation", "dependency rule"],
    ),
    ("layer_violation", &["layer violation", "layer_violation"]),
    (
        "gravity_well",
        &["gravity well", "gravity_well", "gravitywell"],
    ),
    (
        "red_flag",
        &[
            "red flag",
            "red_flag",
            "redflag",
            "fused sibling",
            "fused_sibling",
        ],
    ),
    (
        "stability_violation",
        &["stability violation", "stability_violation"],
    ),
    (
        "zone_flag",
        &[
            "zone of pain",
            "zone_of_pain",
            "zone of uselessness",
            "zone_of_uselessness",
        ],
    ),
    ("low_cohesion", &["low cohesion", "low_cohesion"]),
];

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

// ── Expected coverage per format ─────────────────────────────────────────
//
// One row per format. Flip a row to `ALL_KINDS` when its ticket lands.

const EXPECTED: &[(&str, &[&str])] = &[
    ("text", ALL_KINDS),
    (
        "json",
        &[
            "coupling_violation",
            "cycle",
            "gravity_well",
            "red_flag",
            "stability_violation",
            "zone_flag",
        ],
    ),
    ("xml", &["coupling_violation", "cycle"]),
    ("sonar", &["cycle"]),
    ("md", &["coupling_violation", "cycle"]),
    (
        "html",
        &[
            "coupling_violation",
            "cycle",
            "stability_violation",
            "zone_flag",
        ],
    ),
    (
        "dashboard",
        &["cycle", "rule_violation", "layer_violation", "zone_flag"],
    ),
    ("bundle", &["coupling_violation", "cycle"]),
    (
        "pr",
        &[
            "coupling_violation",
            "cycle",
            "rule_violation",
            "layer_violation",
            "red_flag",
        ],
    ),
    ("briefing", &["cycle", "rule_violation", "layer_violation"]),
    ("mermaid", &["cycle"]),
    ("dot", &[]),
    ("strategy", &[]),
    (
        "explorer",
        &["coupling_violation", "cycle", "gravity_well", "red_flag"],
    ),
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

fn noupling(args: &[&str]) -> String {
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

fn read_tree(dir: &Path) -> String {
    let mut buf = String::new();
    if !dir.exists() {
        return buf;
    }
    for entry in std::fs::read_dir(dir).expect("read report dir") {
        let entry = entry.expect("dir entry");
        if entry.file_type().expect("file type").is_dir() {
            buf.push_str(&read_tree(&entry.path()));
        } else {
            buf.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
        }
    }
    buf
}

/// Scan + audit + every format. Returns format name → output text.
fn render_all_formats(project: &Path) -> BTreeMap<&'static str, String> {
    let root = project.to_str().unwrap();
    noupling(&["scan", root]);
    let text = noupling(&["audit", root]);
    noupling(&["report", "--format", "all", root]);
    noupling(&["report", "--format", "explorer", root]);

    let out = project.join(".noupling");
    let file = |name: &str| std::fs::read_to_string(out.join(name)).unwrap_or_default();

    let mut outputs = BTreeMap::new();
    outputs.insert("text", text);
    outputs.insert("json", file("report.json"));
    outputs.insert("xml", file("report.xml"));
    outputs.insert("sonar", file("noupling-sonar.json"));
    outputs.insert("md", read_tree(&out.join("report-md")));
    outputs.insert("html", read_tree(&out.join("report")));
    outputs.insert("dashboard", file("dashboard.html"));
    outputs.insert("bundle", file("bundle.html"));
    outputs.insert("pr", file("pr.md"));
    outputs.insert("briefing", file("briefing.md"));
    outputs.insert("mermaid", file("report.mermaid"));
    outputs.insert("dot", file("report.dot"));
    outputs.insert("strategy", file("strategy.html"));
    outputs.insert("explorer", explorer_contract(&file("explorer.html")));
    outputs
}

/// The Explorer bundles its field guide and help prose, which name every
/// Issue kind regardless of what the audit found. Match only the embedded
/// Data Contract so the row reflects data, not documentation.
fn explorer_contract(html: &str) -> String {
    let open = r#"<script id="noupling-data" type="application/json">"#;
    let start = html.find(open).map(|i| i + open.len());
    match start {
        Some(i) => {
            let end = html[i..]
                .find("</script>")
                .map(|j| i + j)
                .unwrap_or(html.len());
            html[i..end].to_string()
        }
        None => String::new(),
    }
}

fn kinds_mentioned(output: &str) -> Vec<&'static str> {
    let lower = output.to_lowercase();
    KINDS
        .iter()
        .filter(|(_, markers)| markers.iter().any(|m| lower.contains(m)))
        .map(|(kind, _)| *kind)
        .collect()
}

fn render_matrix(actual: &BTreeMap<&str, Vec<&str>>) -> String {
    let mut s = String::from("format      ");
    for k in ALL_KINDS {
        s.push_str(&format!("{:<10}", &k[..k.len().min(9)]));
    }
    s.push('\n');
    for (fmt, kinds) in actual {
        s.push_str(&format!("{:<12}", fmt));
        for k in ALL_KINDS {
            s.push_str(if kinds.contains(k) {
                "X         "
            } else {
                ".         "
            });
        }
        s.push('\n');
    }
    s
}

// ── Tests ────────────────────────────────────────────────────────────────

/// The fixture's contract: `audit` surfaces every Issue kind.
#[test]
fn fixture_audit_surfaces_every_issue_kind() {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_dir(&fixture_source(), tmp.path());
    let root = tmp.path().to_str().unwrap();
    noupling(&["scan", root]);
    let text = noupling(&["audit", root]);

    let found = kinds_mentioned(&text);
    let missing: Vec<_> = ALL_KINDS.iter().filter(|k| !found.contains(k)).collect();
    assert!(
        missing.is_empty(),
        "audit output is missing Issue kinds {:?}\n--- audit output ---\n{}",
        missing,
        text
    );
}

/// The coverage matrix: which Issue kinds each format mentions.
#[test]
fn every_format_matches_the_expected_issue_kind_coverage() {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_dir(&fixture_source(), tmp.path());
    let outputs = render_all_formats(tmp.path());

    let actual: BTreeMap<&str, Vec<&str>> = outputs
        .iter()
        .map(|(fmt, out)| (*fmt, kinds_mentioned(out)))
        .collect();

    let mut mismatches = Vec::new();
    for (fmt, expected) in EXPECTED {
        let got = actual
            .get(fmt)
            .unwrap_or_else(|| panic!("format {fmt} was not rendered"));
        let mut expected_sorted: Vec<&str> = expected.to_vec();
        expected_sorted.sort_unstable();
        let mut got_sorted = got.clone();
        got_sorted.sort_unstable();
        if expected_sorted != got_sorted {
            mismatches.push(format!(
                "{fmt}: expected {expected_sorted:?}, got {got_sorted:?}"
            ));
        }
    }
    let rendered: Vec<&str> = EXPECTED.iter().map(|(f, _)| *f).collect();
    let extra: Vec<&&str> = actual.keys().filter(|k| !rendered.contains(k)).collect();
    assert!(
        extra.is_empty(),
        "formats rendered but missing from EXPECTED: {extra:?}"
    );
    assert!(
        mismatches.is_empty(),
        "Issue-kind coverage drifted from EXPECTED:\n{}\n\nactual matrix:\n{}",
        mismatches.join("\n"),
        render_matrix(&actual)
    );
}
