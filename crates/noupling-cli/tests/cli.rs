use std::path::Path;
use std::process::{Command, Output};

// ── shared helpers ────────────────────────────────────────────────────────

fn noupling_bin() -> &'static str {
    env!("CARGO_BIN_EXE_noupling")
}

fn run_noupling(args: &[&str]) -> Output {
    Command::new(noupling_bin())
        .args(args)
        .output()
        .expect("run noupling")
}

/// Create a tiny, well-formed Rust project in a tempdir.
/// Imports flow downward (main -> modules/helper) so it scores 100/100.
fn create_clean_fixture() -> tempfile::TempDir {
    let fixture = tempfile::tempdir().expect("tempdir");
    let project = fixture.path();

    let src = project.join("src");
    std::fs::create_dir_all(src.join("modules")).expect("create src/modules");
    std::fs::write(
        src.join("main.rs"),
        "mod modules;\nfn main() { modules::helper::greet(); }\n",
    )
    .expect("write main.rs");
    std::fs::write(src.join("modules").join("mod.rs"), "pub mod helper;\n").expect("write mod.rs");
    std::fs::write(
        src.join("modules").join("helper.rs"),
        "pub fn greet() { println!(\"hello\"); }\n",
    )
    .expect("write helper.rs");

    fixture
}

/// Scan a fixture project. Panics on failure.
fn scan(project: &Path) {
    let out = run_noupling(&["scan", project.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "scan failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ── tests ─────────────────────────────────────────────────────────────────

/// Smoke test: init → scan → audit against a tiny fixture Rust project.
#[test]
fn init_scan_audit_smoke() {
    let fixture = create_clean_fixture();
    let project = fixture.path();

    let init_out = run_noupling(&["init", project.to_str().unwrap()]);
    assert!(
        init_out.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );
    assert!(String::from_utf8_lossy(&init_out.stdout).contains("Created"));

    scan(project);
    let scan_out = run_noupling(&["scan", project.to_str().unwrap()]);
    assert!(String::from_utf8_lossy(&scan_out.stdout).contains("Scan complete"));

    let audit_out = run_noupling(&["audit", project.to_str().unwrap()]);
    assert!(
        audit_out.status.success(),
        "audit failed: {}",
        String::from_utf8_lossy(&audit_out.stderr)
    );
    assert!(String::from_utf8_lossy(&audit_out.stdout).contains("Score:"));
}

/// `--fail-below` should exit zero when the score is above the threshold.
#[test]
fn audit_fail_below_passes_when_score_above_threshold() {
    let fixture = create_clean_fixture();
    let project = fixture.path();
    scan(project);

    let out = run_noupling(&["audit", project.to_str().unwrap(), "--fail-below", "50"]);
    assert!(
        out.status.success(),
        "expected exit 0 (clean fixture, threshold 50); got {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `--fail-below` should exit non-zero when the score is below the threshold.
/// Uses an impossible threshold (101) on a clean fixture so this test is
/// independent of how violations score.
#[test]
fn audit_fail_below_fails_when_score_below_threshold() {
    let fixture = create_clean_fixture();
    let project = fixture.path();
    scan(project);

    let out = run_noupling(&["audit", project.to_str().unwrap(), "--fail-below", "101"]);
    assert!(
        !out.status.success(),
        "expected non-zero exit (threshold 101 > max score 100); got 0"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("below threshold"),
        "expected 'below threshold' in stderr, got: {}",
        stderr
    );
}

/// `report --format json` should produce parseable JSON with the documented top-level keys.
#[test]
fn report_json_produces_parseable_json() {
    let fixture = create_clean_fixture();
    let project = fixture.path();
    scan(project);

    let out = run_noupling(&["report", project.to_str().unwrap(), "--format", "json"]);
    assert!(
        out.status.success(),
        "report json failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report_path = project.join(".noupling").join("report.json");
    assert!(
        report_path.exists(),
        "expected report.json at {}",
        report_path.display()
    );

    let content = std::fs::read_to_string(&report_path).expect("read report.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("parseable JSON");

    // The contract: these top-level keys exist and are the right kind of value.
    assert!(
        parsed.get("score").and_then(|v| v.as_f64()).is_some(),
        "missing/wrong-type 'score'"
    );
    assert!(
        parsed
            .get("total_modules")
            .and_then(|v| v.as_u64())
            .is_some(),
        "missing/wrong-type 'total_modules'"
    );
    assert!(
        parsed.get("issues").and_then(|v| v.as_array()).is_some(),
        "missing/wrong-type 'issues' (the one Issue array, ADR 0002)"
    );
    assert!(
        parsed.get("coupling_violations").is_none(),
        "per-kind arrays were removed in 0.9.0 (#350)"
    );
    assert!(
        parsed.get("hotspots").and_then(|v| v.as_array()).is_some(),
        "missing/wrong-type 'hotspots'"
    );
}

/// `report --format all` should emit a file per format under .noupling/.
/// Guards against silent regressions in the multi-format pipeline.
#[test]
fn report_format_all_emits_files_for_each_format() {
    let fixture = create_clean_fixture();
    let project = fixture.path();
    scan(project);

    let out = run_noupling(&["report", project.to_str().unwrap(), "--format", "all"]);
    assert!(
        out.status.success(),
        "report all failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let dir = project.join(".noupling");
    // A representative subset of stable single-file formats. We don't
    // enumerate every output here to avoid coupling the test to the
    // exact format roster (markdown/html produce *directories*, not files).
    // explorer.html and strategy.html are the two bespoke arms; `all`
    // must include them because the help promises "every format" (#382).
    let expected = [
        "report.json",
        "report.xml",
        "report.dot",
        "noupling-sonar.json",
        "strategy.html",
        "explorer.html",
    ];
    for name in &expected {
        let path = dir.join(name);
        assert!(
            path.exists(),
            "expected {} after `report --format all`",
            path.display()
        );
    }
}

/// `report --format explorer` emits `.noupling/explorer.html` containing the
/// inlined Data Contract block.
#[test]
fn report_explorer_emits_self_contained_html() {
    let fixture = create_clean_fixture();
    let project = fixture.path();

    scan(project);

    let out = run_noupling(&["report", project.to_str().unwrap(), "--format", "explorer"]);
    assert!(
        out.status.success(),
        "report --format explorer failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let explorer = project.join(".noupling").join("explorer.html");
    assert!(explorer.exists(), "expected {}", explorer.display());

    let html = std::fs::read_to_string(&explorer).expect("read explorer.html");
    assert!(
        html.contains(r#"<script id="noupling-data" type="application/json">"#),
        "data block must be present"
    );
    assert!(
        html.contains(r#""format_version":3"#),
        "Data Contract must declare format_version: 3 (#396)"
    );
    // React mount point (the template renders into it from the injected data).
    assert!(
        html.contains(r#"id="root""#),
        "React mount point must be present"
    );
}

/// `--output <path>` redirects the explorer file away from the default
/// `.noupling/explorer.html`.
#[test]
fn report_explorer_honors_output_flag() {
    let fixture = create_clean_fixture();
    let project = fixture.path();
    let custom = project.join("out").join("custom.html");

    scan(project);

    let out = run_noupling(&[
        "report",
        project.to_str().unwrap(),
        "--format",
        "explorer",
        "--output",
        custom.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "report --format explorer --output failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        custom.exists(),
        "expected custom output at {}",
        custom.display()
    );
    assert!(
        !project.join(".noupling").join("explorer.html").exists(),
        "default path must NOT also be written"
    );
}

/// `--no-history` produces a Data Contract with an empty history array.
#[test]
fn report_explorer_no_history_strips_history_block() {
    let fixture = create_clean_fixture();
    let project = fixture.path();

    scan(project);

    let out = run_noupling(&[
        "report",
        project.to_str().unwrap(),
        "--format",
        "explorer",
        "--no-history",
    ]);
    assert!(out.status.success());

    let html =
        std::fs::read_to_string(project.join(".noupling").join("explorer.html")).expect("read");
    assert!(html.contains(r#""history":[]"#));
}

// ── One audit per snapshot (ADR 0001, #341) ───────────────────────────────

/// A Rust project with no `layers` configured whose directories follow
/// the ui / domain / data convention, so layer inference fires. One
/// import reaches upward (data → ui) and one sibling pair sits inside ui.
fn create_unlayered_by_settings_fixture() -> tempfile::TempDir {
    let fixture = tempfile::tempdir().expect("tempdir");
    let src = fixture.path().join("src");
    let write = |rel: &str, body: &str| {
        let p = src.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    };
    write(
        "main.rs",
        "mod ui; mod domain; mod data;\nfn main() { ui::home::show(); }\n",
    );
    write(
        "ui/mod.rs",
        "pub mod home; pub mod login; pub mod settings;\n",
    );
    write(
        "ui/home.rs",
        "use crate::ui::login;\nuse crate::domain::cart;\npub fn show() { login::form(); cart::total(); }\n",
    );
    write("ui/login.rs", "pub fn form() {}\n");
    write("ui/settings.rs", "pub fn page() {}\n");
    write(
        "domain/mod.rs",
        "pub mod cart; pub mod order; pub mod user;\n",
    );
    write(
        "domain/cart.rs",
        "use crate::data::cart_repo;\npub fn total() -> u32 { cart_repo::load() }\n",
    );
    write("domain/order.rs", "pub fn place() {}\n");
    write("domain/user.rs", "pub fn name() {}\n");
    write(
        "data/mod.rs",
        "pub mod cart_repo; pub mod order_repo; pub mod api;\n",
    );
    write(
        "data/cart_repo.rs",
        "use crate::ui::settings;\npub fn load() -> u32 { settings::page(); 1 }\n",
    );
    write("data/order_repo.rs", "pub fn save() {}\n");
    write("data/api.rs", "pub fn get() {}\n");
    fixture
}

fn health_score_line(text: &str) -> &str {
    text.lines()
        .find(|l| l.starts_with("Health Score:"))
        .unwrap_or_else(|| panic!("no Health Score line in:\n{text}"))
}

/// With no `layers` in settings, `audit` (the text format) and the
/// Explorer read one audit: same inferred layers, same score, and the
/// text output says the layers were inferred.
#[test]
fn unlayered_project_gets_the_same_inferred_audit_in_audit_text_and_explorer() {
    let fixture = create_unlayered_by_settings_fixture();
    let project = fixture.path();
    let root = project.to_str().unwrap();
    scan(project);

    let audit = run_noupling(&["audit", root]);
    assert!(
        audit.status.success(),
        "{}",
        String::from_utf8_lossy(&audit.stderr)
    );
    let audit_text = String::from_utf8_lossy(&audit.stdout).into_owned();
    assert!(
        audit_text.contains("Layers: inferred from path names (presentation, domain, data)"),
        "audit must say the layers were inferred:\n{audit_text}"
    );
    assert!(
        audit_text.contains("Layer Violations (1)") || audit_text.contains("Layer Violation"),
        "data → ui must surface as a Layer Violation against the inferred layers:\n{audit_text}"
    );

    let explorer = run_noupling(&["report", root, "--format", "explorer"]);
    assert!(
        explorer.status.success(),
        "{}",
        String::from_utf8_lossy(&explorer.stderr)
    );
    let html = std::fs::read_to_string(project.join(".noupling").join("explorer.html")).unwrap();
    let open = r#"<script id="noupling-data" type="application/json">"#;
    let start = html.find(open).expect("data block") + open.len();
    let end = start + html[start..].find("</script>").expect("closed");
    let contract: serde_json::Value = serde_json::from_str(&html[start..end]).unwrap();

    assert_eq!(contract["layers_auto_detected"], true);
    let layer_names: Vec<&str> = contract["layers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(layer_names, vec!["presentation", "domain", "data"]);
    let explorer_score = contract["health_score"].as_f64().unwrap();
    assert_eq!(
        health_score_line(&audit_text),
        format!("Health Score: {:.1}/100", explorer_score),
        "audit and explorer disagree on the score"
    );
}

// ── Baseline over every Issue kind (#343) ─────────────────────────────────

/// `baseline save` → `audit --baseline` passes; one new sibling import →
/// exit 1 naming exactly one new Issue; `report --format text --baseline`
/// marks the accepted Issues.
#[test]
fn baseline_round_trip_fails_only_on_new_issues() {
    let fixture = create_unlayered_by_settings_fixture();
    let project = fixture.path();
    let root = project.to_str().unwrap();
    // Explicit strict mode so the sibling pairs stay Issues.
    std::fs::create_dir_all(project.join(".noupling")).unwrap();
    std::fs::write(
        project.join(".noupling/settings.json"),
        r#"{"coupling_mode":"strict"}"#,
    )
    .unwrap();
    scan(project);

    let save = run_noupling(&["baseline", "save", root]);
    assert!(
        save.status.success(),
        "{}",
        String::from_utf8_lossy(&save.stderr)
    );
    let saved = String::from_utf8_lossy(&save.stdout).into_owned();
    assert!(saved.contains("Baseline saved with"), "{saved}");
    assert!(
        !saved.contains("with 0 issue"),
        "fixture must have Issues: {saved}"
    );

    let clean = run_noupling(&["audit", root, "--baseline"]);
    let clean_out = String::from_utf8_lossy(&clean.stdout).into_owned();
    assert!(clean.status.success(), "{clean_out}");
    assert!(clean_out.contains("Not in baseline: 0"), "{clean_out}");
    assert!(clean_out.contains("(baselined)"), "{clean_out}");

    let text = run_noupling(&["report", root, "--format", "text", "--baseline"]);
    assert!(
        text.status.success(),
        "{}",
        String::from_utf8_lossy(&text.stderr)
    );
    let report_txt = std::fs::read_to_string(project.join(".noupling/report.txt")).unwrap();
    assert!(report_txt.contains("0 not in baseline,"), "{report_txt}");
    assert!(report_txt.contains("(baselined)"), "{report_txt}");

    // A new sibling pair between two fresh directories outside the ring,
    // so instability of the existing directories is untouched and the
    // only new Issue is the Coupling Violation itself. `misc` exposes a
    // trait: abstract and stable sits on the main sequence, so it earns no
    // Zone Flag of its own (a concrete leaf would, since #413).
    std::fs::create_dir_all(project.join("src/tools")).unwrap();
    std::fs::create_dir_all(project.join("src/misc")).unwrap();
    std::fs::write(
        project.join("src/tools/a.rs"),
        "use crate::misc::b;\npub fn run(g: &dyn b::Go) { g.go(); }\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/misc/b.rs"),
        "pub trait Go { fn go(&self); }\n",
    )
    .unwrap();
    scan(project);
    let dirty = run_noupling(&["audit", root, "--baseline"]);
    let dirty_out = String::from_utf8_lossy(&dirty.stdout).into_owned();
    let dirty_err = String::from_utf8_lossy(&dirty.stderr).into_owned();
    assert!(
        !dirty.status.success(),
        "must fail on a new Issue:\n{dirty_out}"
    );
    assert!(dirty_out.contains("Not in baseline: 1"), "{dirty_out}");
    assert!(
        dirty_err.contains("1 issue(s) not in the baseline"),
        "{dirty_err}"
    );
}

/// A pre-0.9.0 baseline file warns once and treats nothing as baselined.
#[test]
fn old_format_baseline_warns_instead_of_crashing() {
    let fixture = create_clean_fixture();
    let project = fixture.path();
    let root = project.to_str().unwrap();
    scan(project);
    std::fs::write(
        project.join(".noupling/baseline.json"),
        r#"{"version":1,"timestamp":"0","violation_count":0,"fingerprints":[]}"#,
    )
    .unwrap();

    let out = run_noupling(&["audit", root, "--baseline"]);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "clean project has no new Issues: {stderr}"
    );
    assert_eq!(
        stderr.lines().filter(|l| l.contains("pre-0.9.0")).count(),
        1,
        "{stderr}"
    );
    assert!(stderr.contains("baseline save"), "{stderr}");
}

// ── Per-kind Issue counts are recorded at scan time (#349) ────────────────

/// Two scans → the strategy report carries a recorded count for every
/// kind on both snapshots (no gaps), without any `audit` in between.
#[test]
fn scan_records_issue_kind_counts_that_strategy_trends() {
    let fixture = create_unlayered_by_settings_fixture();
    let project = fixture.path();
    let root = project.to_str().unwrap();
    scan(project);
    scan(project);

    let out = run_noupling(&["report", root, "--format", "strategy"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = std::fs::read_to_string(project.join(".noupling/strategy.html")).unwrap();
    let start = html.find("const D = ").unwrap() + "const D = ".len();
    // `;` followed by a line break; the template may carry CRLF on Windows.
    let end = start
        + html[start..]
            .find(";\r")
            .or_else(|| html[start..].find(";\n"))
            .unwrap();
    let data: serde_json::Value = serde_json::from_str(&html[start..end]).unwrap();

    let series = data["issue_kind_series"].as_array().unwrap();
    assert_eq!(series.len(), 9);
    for s in series {
        let counts = s["counts"].as_array().unwrap();
        assert_eq!(counts.len(), 2, "{s}");
        assert!(
            counts.iter().all(|c| !c.is_null()),
            "scan must record counts: {s}"
        );
    }
    let cycle = series.iter().find(|s| s["kind"] == "cycle").unwrap();
    assert_eq!(
        cycle["counts"][0], 1,
        "the ring ui → domain → data → ui: {cycle}"
    );
}

// ── examples/ ────────────────────────────────────────────────────────────

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == ".noupling" {
            continue; // never carry a developer's local scan into the test
        }
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Scan a project under `examples/` in a scratch copy and return the Issue
/// kinds its audit produces.
fn example_issue_kinds(name: &str) -> std::collections::BTreeSet<String> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    let tmp = tempfile::tempdir().unwrap();
    copy_tree(&source, tmp.path());
    scan(tmp.path());
    let out = run_noupling(&["report", tmp.path().to_str().unwrap(), "--format", "json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(tmp.path().join(".noupling/report.json")).unwrap(),
    )
    .unwrap();
    json["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap().to_string())
        .collect()
}

/// The three `examples/` projects demonstrate what `examples/README.md`
/// says they do (#385): clean, a one-way sibling Coupling Violation with
/// no Cycle, and a three-package Cycle.
#[test]
fn examples_demonstrate_their_documented_issue_kinds() {
    let clean = example_issue_kinds("kotlin-clean");
    assert!(clean.is_empty(), "kotlin-clean must audit clean: {clean:?}");

    let coupled = example_issue_kinds("kotlin-coupled");
    assert!(
        coupled.contains("coupling_violation"),
        "kotlin-coupled must show a Coupling Violation: {coupled:?}"
    );
    assert!(
        !coupled.contains("cycle"),
        "kotlin-coupled must not be a Cycle (that is kotlin-circular's job): {coupled:?}"
    );

    let circular = example_issue_kinds("kotlin-circular");
    assert!(
        circular.contains("cycle"),
        "kotlin-circular must show a Cycle: {circular:?}"
    );
}

// ── monorepo mode (#357) ─────────────────────────────────────────────────

/// `audit` in monorepo mode audits each module through the same pipeline as
/// `report --module X`: same score, same Issues, layer rules applied, and
/// the summary renders each module's Issue cards.
#[test]
fn monorepo_audit_matches_report_module_and_renders_issue_cards() {
    let fixture = create_unlayered_by_settings_fixture();
    let project = fixture.path();
    let root = project.to_str().unwrap();
    scan(project);
    // One module covering src/, with declared layers so that the fixture's
    // data → ui import is an upward Layer Violation inside that module.
    let settings = r#"{
      "modules": [
        { "name": "app", "path": "src", "depends_on": [] }
      ],
      "layers": [
        { "name": "ui", "pattern": "**/ui/**" },
        { "name": "domain", "pattern": "**/domain/**" },
        { "name": "data", "pattern": "**/data/**" }
      ]
    }"#;
    std::fs::write(project.join(".noupling/settings.json"), settings).unwrap();

    let summary = run_noupling(&["audit", root]);
    assert!(
        summary.status.success(),
        "{}",
        String::from_utf8_lossy(&summary.stderr)
    );
    let summary_out = String::from_utf8_lossy(&summary.stdout).into_owned();
    assert!(summary_out.contains("Overall Score:"), "{summary_out}");
    assert!(
        summary_out.contains("Layer Violation:"),
        "monorepo summary must render the module's Issue cards, layers applied:\n{summary_out}"
    );

    let single = run_noupling(&["audit", root, "--module", "app"]);
    let single_out = String::from_utf8_lossy(&single.stdout).into_owned();
    let audit_score = health_score_line(&single_out).to_string();

    let report = run_noupling(&["report", root, "--module", "app", "--format", "json"]);
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(project.join(".noupling/report.json")).unwrap(),
    )
    .unwrap();
    let report_score = json["score"].as_f64().unwrap();
    assert!(
        audit_score.contains(&format!("{report_score:.1}")),
        "audit --module says `{audit_score}`, report --module says {report_score:.1}"
    );
    let report_issues = json["issues"].as_array().unwrap().len();
    assert!(
        single_out.contains(&format!("Issues ({report_issues})")),
        "audit --module must list the same {report_issues} Issues:\n{single_out}"
    );
}
