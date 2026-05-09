use std::process::Command;

/// Smoke test: init → scan → audit against a tiny fixture Rust project.
///
/// Asserts:
///   - `noupling init`  exits 0 and prints "Created"
///   - `noupling scan`  exits 0, prints "Scan complete"
///   - `noupling audit` exits 0 and prints a health score line ("Score:")
#[test]
fn init_scan_audit_smoke() {
    let fixture = tempfile::tempdir().expect("tempdir");
    let project = fixture.path();

    // Create a minimal Rust source tree inside the temp dir
    let src = project.join("src");
    std::fs::create_dir_all(src.join("modules")).expect("create src/modules");
    std::fs::write(
        src.join("main.rs"),
        "mod modules;\nfn main() { modules::helper::greet(); }\n",
    )
    .expect("write main.rs");
    std::fs::create_dir_all(src.join("modules")).expect("create modules dir");
    std::fs::write(src.join("modules").join("mod.rs"), "pub mod helper;\n").expect("write mod.rs");
    std::fs::write(
        src.join("modules").join("helper.rs"),
        "pub fn greet() { println!(\"hello\"); }\n",
    )
    .expect("write helper.rs");

    let bin = env!("CARGO_BIN_EXE_noupling");

    // --- init ---
    let init_out = Command::new(bin)
        .args(["init", project.to_str().expect("valid path")])
        .output()
        .expect("run noupling init");
    assert!(
        init_out.status.success(),
        "noupling init failed: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );
    let init_stdout = String::from_utf8_lossy(&init_out.stdout);
    assert!(
        init_stdout.contains("Created"),
        "expected 'Created' in init output, got: {}",
        init_stdout
    );

    // --- scan ---
    let scan_out = Command::new(bin)
        .args(["scan", project.to_str().expect("valid path")])
        .output()
        .expect("run noupling scan");
    assert!(
        scan_out.status.success(),
        "noupling scan failed: {}",
        String::from_utf8_lossy(&scan_out.stderr)
    );
    let scan_stdout = String::from_utf8_lossy(&scan_out.stdout);
    assert!(
        scan_stdout.contains("Scan complete"),
        "expected 'Scan complete' in scan output, got: {}",
        scan_stdout
    );

    // --- audit ---
    let audit_out = Command::new(bin)
        .args(["audit", project.to_str().expect("valid path")])
        .output()
        .expect("run noupling audit");
    assert!(
        audit_out.status.success(),
        "noupling audit failed (exit {:?}): {}",
        audit_out.status.code(),
        String::from_utf8_lossy(&audit_out.stderr)
    );
    let audit_stdout = String::from_utf8_lossy(&audit_out.stdout);
    assert!(
        audit_stdout.contains("Score:"),
        "expected 'Score:' in audit output, got: {}",
        audit_stdout
    );
}
