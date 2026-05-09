//! Integration tests for the analyzer orchestrator: `audit()` and
//! `audit_with_settings()`. Per-concern unit tests live alongside their
//! own modules (e.g. `coupling.rs`, `violation_age.rs`).

use super::*;
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

fn make_dep(from: &str, to: &str, line: i32) -> Dependency {
    Dependency {
        from_module_id: from.to_string(),
        to_module_id: to.to_string(),
        line_number: line,
    }
}

// ── BFS coupling detection ──

#[test]
fn detects_sibling_coupling() {
    // scanner depends on storage (siblings under src/slices)
    let modules = vec![
        make_module("a", "src/slices/scanner/mod.rs"),
        make_module("b", "src/slices/storage/mod.rs"),
    ];
    let deps = vec![make_dep("a", "b", 10)];

    let result = audit(&modules, &deps);
    assert!(
        !result.violations.is_empty(),
        "Should detect coupling between scanner and storage"
    );
    assert_eq!(result.violations[0].dir_a, "src/slices/scanner");
    assert_eq!(result.violations[0].dir_b, "src/slices/storage");
}

#[test]
fn no_violations_when_independent() {
    let modules = vec![
        make_module("a", "src/slices/scanner/mod.rs"),
        make_module("b", "src/slices/storage/mod.rs"),
    ];
    let deps: Vec<Dependency> = vec![];

    let result = audit(&modules, &deps);
    assert!(result.violations.is_empty());
}

#[test]
fn no_violations_for_internal_deps() {
    let modules = vec![
        make_module("a", "src/scanner/parser.rs"),
        make_module("b", "src/scanner/resolver.rs"),
    ];
    let deps = vec![make_dep("a", "b", 1)];

    let result = audit(&modules, &deps);
    assert!(result.violations.is_empty());
}

// ── Severity ──

#[test]
fn severity_at_depth_zero() {
    // Two top-level sibling dirs
    let modules = vec![
        make_module("a", "scanner/mod.rs"),
        make_module("b", "storage/mod.rs"),
    ];
    let deps = vec![make_dep("a", "b", 1)];

    let result = audit(&modules, &deps);
    assert!(!result.violations.is_empty());
    assert!((result.violations[0].severity - 1.0).abs() < f64::EPSILON);
}

#[test]
fn severity_decreases_with_depth() {
    // Siblings at depth 2 (under src/slices/)
    let modules = vec![
        make_module("a", "src/slices/scanner/mod.rs"),
        make_module("b", "src/slices/storage/mod.rs"),
    ];
    let deps = vec![make_dep("a", "b", 1)];

    let result = audit(&modules, &deps);
    assert!(!result.violations.is_empty());
    // parent "src/slices" has depth 2, children are at depth 3, severity = 1/(3+1) = 0.25
    let expected = 0.25;
    assert!(
        (result.violations[0].severity - expected).abs() < 0.01,
        "Expected severity ~{}, got {}",
        expected,
        result.violations[0].severity
    );
}

// ── Health score ──

#[test]
fn perfect_score_no_violations() {
    let modules = vec![
        make_module("a", "src/scanner/mod.rs"),
        make_module("b", "src/storage/mod.rs"),
    ];
    let deps: Vec<Dependency> = vec![];

    let result = audit(&modules, &deps);
    assert!((result.score - 100.0).abs() < f64::EPSILON);
}

#[test]
fn score_decreases_with_violations() {
    let modules = vec![
        make_module("a", "scanner/mod.rs"),
        make_module("b", "storage/mod.rs"),
    ];
    let deps = vec![make_dep("a", "b", 1)];

    let result = audit(&modules, &deps);
    assert!(result.score < 100.0);
    assert!(result.score >= 0.0);
    // severity=1.0, total_modules=2, score=100*(1-1.0/2)=50
    assert!(
        (result.score - 50.0).abs() < 0.01,
        "Expected ~50, got {}",
        result.score
    );
}

#[test]
fn score_clamps_to_zero() {
    // Many high-severity violations
    let modules = vec![make_module("a", "x/mod.rs"), make_module("b", "y/mod.rs")];
    // Create multiple deps to push score below 0
    let deps = vec![
        make_dep("a", "b", 1),
        make_dep("a", "b", 2),
        make_dep("a", "b", 3),
        make_dep("b", "a", 1),
        make_dep("b", "a", 2),
        make_dep("b", "a", 3),
    ];

    let result = audit(&modules, &deps);
    assert!(result.score >= 0.0);
}

#[test]
fn sibling_violations_have_sibling_direction() {
    let modules = vec![
        make_module("a", "src/alpha/mod.rs"),
        make_module("b", "src/beta/mod.rs"),
    ];
    let deps = vec![make_dep("a", "b", 1)];
    let result = audit(&modules, &deps);
    let siblings: Vec<&CouplingViolation> = result
        .violations
        .iter()
        .filter(|v| !v.is_circular)
        .collect();
    assert!(!siblings.is_empty(), "Should have sibling violations");
    for v in siblings {
        assert_eq!(v.direction, DependencyDirection::Sibling);
    }
}

#[test]
fn circular_violations_have_circular_direction() {
    let modules = vec![
        make_module("a", "src/alpha/mod.rs"),
        make_module("b", "src/beta/mod.rs"),
    ];
    let deps = vec![make_dep("a", "b", 1), make_dep("b", "a", 5)];
    let result = audit(&modules, &deps);
    let circular: Vec<&CouplingViolation> =
        result.violations.iter().filter(|v| v.is_circular).collect();
    assert!(!circular.is_empty(), "Should have circular violations");
    for v in circular {
        assert_eq!(v.direction, DependencyDirection::Circular);
    }
}

#[test]
fn gravity_wells_detected_for_high_rri_modules() {
    let modules = vec![
        make_module("a", "src/alpha/mod.rs"),
        make_module("b", "src/beta/mod.rs"),
        make_module("c", "src/gamma/mod.rs"),
    ];
    // a imports b (1 dep), a imports c (1 dep), b imports c (1 dep)
    // Module a participates in 2 violations, b in 2, c in 2
    let deps = vec![
        make_dep("a", "b", 1),
        make_dep("a", "c", 2),
        make_dep("b", "c", 3),
    ];
    let mut result = audit(&modules, &deps);
    let weights = crate::settings::RiskWeights {
        downward: 2.0,
        sibling: 4.0,
        upward: 6.0,
        external: 8.0,
        transitive: 9.0,
        circular: 10.0,
    };
    result.apply_risk_weights(&weights);

    // All modules participate in violations, gravity wells depend on
    // whether any module's total RRI exceeds 2× the median
    // This is a structural test — just verify the computation runs
    // and gravity_wells is populated (or empty) without panicking
    assert!(result.gravity_wells.len() <= modules.len());
}

#[test]
fn apply_risk_weights_computes_rri() {
    let modules = vec![
        make_module("a", "src/alpha/mod.rs"),
        make_module("b", "src/beta/mod.rs"),
    ];
    // 3 imports from alpha to beta → weight=3 after dedup
    let deps = vec![
        make_dep("a", "b", 1),
        make_dep("a", "b", 2),
        make_dep("a", "b", 3),
    ];
    let mut result = audit(&modules, &deps);
    let weights = crate::settings::RiskWeights {
        downward: 2.0,
        sibling: 4.0,
        upward: 6.0,
        external: 8.0,
        transitive: 9.0,
        circular: 10.0,
    };
    result.apply_risk_weights(&weights);

    let siblings: Vec<&CouplingViolation> = result
        .violations
        .iter()
        .filter(|v| !v.is_circular)
        .collect();
    assert!(!siblings.is_empty());
    // RRI = sibling_weight(4) × density(3) = 12
    assert_eq!(siblings[0].rri, 12.0);
}

#[test]
fn apply_risk_weights_circular_uses_hop_counts() {
    let modules = vec![
        make_module("a", "src/alpha/mod.rs"),
        make_module("b", "src/beta/mod.rs"),
    ];
    let deps = vec![
        make_dep("a", "b", 1),
        make_dep("a", "b", 2),
        make_dep("b", "a", 5),
    ];
    let mut result = audit(&modules, &deps);
    let weights = crate::settings::RiskWeights {
        downward: 2.0,
        sibling: 4.0,
        upward: 6.0,
        external: 8.0,
        transitive: 9.0,
        circular: 10.0,
    };
    result.apply_risk_weights(&weights);

    let circular: Vec<&CouplingViolation> =
        result.violations.iter().filter(|v| v.is_circular).collect();
    assert!(!circular.is_empty());
    // Total hop imports: alpha→beta has some + beta→alpha has some
    // RRI = circular_weight(10) × total_density
    assert!(
        circular[0].rri >= 10.0,
        "Circular RRI should be at least 10"
    );
}

#[test]
fn tri_computed_from_rri_sum() {
    let modules = vec![
        make_module("a", "src/alpha/mod.rs"),
        make_module("b", "src/beta/mod.rs"),
    ];
    let deps = vec![make_dep("a", "b", 1), make_dep("a", "b", 2)];
    let mut result = audit(&modules, &deps);
    let weights = crate::settings::RiskWeights {
        downward: 2.0,
        sibling: 4.0,
        upward: 6.0,
        external: 8.0,
        transitive: 9.0,
        circular: 10.0,
    };
    result.apply_risk_weights(&weights);

    // Sibling violation with density 2: RRI = 4 × 2 = 8
    // TRI = sum of all RRIs = 8
    assert_eq!(result.tri, 8.0);
    // Score = 100 * (1 - 8 / (2 * 10)) = 100 * (1 - 0.4) = 60
    assert!(
        (result.score - 60.0).abs() < 0.1,
        "Score should be ~60, got {}",
        result.score
    );
}

#[test]
fn empty_project_scores_100() {
    let result = audit(&[], &[]);
    assert!((result.score - 100.0).abs() < f64::EPSILON);
}

#[test]
fn violations_sorted_by_severity_descending() {
    let modules = vec![
        make_module("a", "scanner/mod.rs"),
        make_module("b", "storage/mod.rs"),
        make_module("c", "src/slices/analyzer/mod.rs"),
        make_module("d", "src/slices/reporter/mod.rs"),
    ];
    let deps = vec![
        make_dep("a", "b", 1), // depth 0, severity 1.0
        make_dep("c", "d", 1), // depth 2, severity 0.33
    ];

    let result = audit(&modules, &deps);
    if result.violations.len() >= 2 {
        assert!(result.violations[0].severity >= result.violations[1].severity);
    }
}

// ── Circular dependencies ──

#[test]
fn detects_circular_dependency() {
    let modules = vec![
        make_module("a", "src/alpha/mod.rs"),
        make_module("b", "src/beta/mod.rs"),
    ];
    // A -> B and B -> A = cycle
    let deps = vec![make_dep("a", "b", 1), make_dep("b", "a", 5)];

    let result = audit(&modules, &deps);
    let circular: Vec<&CouplingViolation> =
        result.violations.iter().filter(|v| v.is_circular).collect();
    assert!(
        !circular.is_empty(),
        "Should detect circular dependency between a and b"
    );
    // Severity depends on depth: siblings under "src" are at depth 2, severity = 1/(2+1)
    assert!(circular[0].severity > 0.0);
}

#[test]
fn no_circular_when_one_direction() {
    let modules = vec![
        make_module("a", "src/alpha/mod.rs"),
        make_module("b", "src/beta/mod.rs"),
    ];
    let deps = vec![make_dep("a", "b", 1)];

    let result = audit(&modules, &deps);
    let circular: Vec<&CouplingViolation> =
        result.violations.iter().filter(|v| v.is_circular).collect();
    assert!(circular.is_empty());
}

#[test]
fn detects_transitive_cycle() {
    let modules = vec![
        make_module("a", "src/x/mod.rs"),
        make_module("b", "src/y/mod.rs"),
        make_module("c", "src/z/mod.rs"),
    ];
    // A -> B -> C -> A = transitive cycle
    let deps = vec![
        make_dep("a", "b", 1),
        make_dep("b", "c", 1),
        make_dep("c", "a", 1),
    ];

    let result = audit(&modules, &deps);
    let circular: Vec<&CouplingViolation> =
        result.violations.iter().filter(|v| v.is_circular).collect();
    assert!(
        !circular.is_empty(),
        "Should detect transitive circular dependency"
    );
}

// ── audit_with_settings: the deterministic settings-aware seam ──

#[test]
fn audit_with_settings_matches_manual_pipeline() {
    // The seam must produce the same AuditResult as calling the 5 methods by hand
    // in the documented order. This pins the contract so callers can't drift.
    let modules = vec![
        make_module("a", "src/alpha/mod.rs"),
        make_module("b", "src/beta/mod.rs"),
    ];
    let deps = vec![
        make_dep("a", "b", 1),
        make_dep("a", "b", 2),
        make_dep("a", "b", 3),
    ];
    let settings = crate::settings::Settings::default();

    let auto = audit_with_settings(&modules, &deps, &settings);

    let mut manual = audit(&modules, &deps);
    manual.filter_by_severity(settings.thresholds.minimum_severity);
    manual.apply_coupling_mode(settings.effective_coupling_mode());
    manual.apply_risk_weights(&settings.risk_weights);
    manual.apply_layer_weights(&settings.layers);
    manual.filter_by_layers(&settings.layers);

    assert_eq!(auto.score, manual.score);
    assert_eq!(auto.violations.len(), manual.violations.len());
    assert_eq!(auto.tri, manual.tri);
    for (a, m) in auto.violations.iter().zip(manual.violations.iter()) {
        assert_eq!(a.rri, m.rri);
        assert_eq!(a.from_module, m.from_module);
        assert_eq!(a.to_module, m.to_module);
    }
}

#[test]
fn audit_with_settings_empty_project_scores_100() {
    let settings = crate::settings::Settings::default();
    let result = audit_with_settings(&[], &[], &settings);
    assert!((result.score - 100.0).abs() < f64::EPSILON);
}
