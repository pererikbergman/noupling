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
        parsed
            .get("coupling_violations")
            .and_then(|v| v.as_array())
            .is_some(),
        "missing/wrong-type 'coupling_violations'"
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
    // enumerate all 12 outputs here to avoid coupling the test to the
    // exact format roster (markdown/html produce *directories*, not files).
    let expected = [
        "report.json",
        "report.xml",
        "report.dot",
        "noupling-sonar.json",
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
