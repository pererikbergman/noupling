//! Baseline file management for incremental adoption.
//!
//! Saves current violations as an accepted baseline. Future audits with
//! `--baseline` only report violations not in the baseline.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::analyzer::{AuditResult, CouplingViolation};

/// A fingerprint that uniquely identifies a violation.
fn fingerprint(v: &CouplingViolation) -> String {
    if v.is_circular {
        let dirs: Vec<String> = v
            .cycle_path
            .iter()
            .map(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(p)
                    .to_string()
            })
            .collect();
        format!("circular:{}:{}", v.cycle_order, dirs.join("->"))
    } else {
        format!("coupling:{}:{}", v.from_module, v.to_module)
    }
}

#[derive(Serialize, Deserialize)]
struct BaselineFile {
    version: u32,
    timestamp: String,
    violation_count: usize,
    fingerprints: Vec<String>,
}

/// Save current violations as the baseline.
pub fn save_baseline(path: &Path, result: &AuditResult) -> Result<()> {
    let fingerprints: Vec<String> = result.violations.iter().map(fingerprint).collect();

    let baseline = BaselineFile {
        version: 1,
        timestamp: chrono_now(),
        violation_count: fingerprints.len(),
        fingerprints,
    };

    let content = serde_json::to_string_pretty(&baseline)?;
    let baseline_path = path.join(".noupling").join("baseline.json");
    std::fs::create_dir_all(path.join(".noupling"))?;
    std::fs::write(&baseline_path, content)?;
    println!(
        "Baseline saved with {} violations to {}",
        baseline.violation_count,
        baseline_path.display()
    );
    Ok(())
}

/// Compare current violations against the baseline.
/// Returns (new_violations, resolved_count).
pub fn compare_baseline(path: &Path, result: &mut AuditResult) -> Result<(usize, usize)> {
    let baseline_path = path.join(".noupling").join("baseline.json");
    if !baseline_path.exists() {
        anyhow::bail!(
            "No baseline found at {}. Run `noupling baseline save` first.",
            baseline_path.display()
        );
    }

    let content = std::fs::read_to_string(&baseline_path)?;
    let baseline: BaselineFile = serde_json::from_str(&content)?;
    let baseline_set: HashSet<String> = baseline.fingerprints.into_iter().collect();

    // Partition violations into new (not in baseline) and existing (in baseline)
    let mut new_count = 0;
    let current_fingerprints: Vec<String> = result.violations.iter().map(fingerprint).collect();

    // Count resolved (in baseline but not in current)
    let current_set: HashSet<String> = current_fingerprints.iter().cloned().collect();
    let resolved_count = baseline_set
        .iter()
        .filter(|f| !current_set.contains(*f))
        .count();

    // Keep only new violations
    result.violations.retain(|v| {
        let fp = fingerprint(v);
        let is_new = !baseline_set.contains(&fp);
        if is_new {
            new_count += 1;
        }
        is_new
    });
    result.recalculate_score();

    Ok((new_count, resolved_count))
}

fn chrono_now() -> String {
    // Simple timestamp without adding chrono dependency
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_coupling(from: &str, to: &str) -> CouplingViolation {
        CouplingViolation {
            dir_a: "a".to_string(),
            dir_b: "b".to_string(),
            from_module: from.to_string(),
            to_module: to.to_string(),
            line_number: 1,
            weight: 1,
            depth: 0,
            severity: 0.5,
            is_circular: false,
            cycle_path: Vec::new(),
            cycle_hop_files: Vec::new(),
            cycle_order: 0,
            cycle_hop_counts: Vec::new(),
            weakest_link: None,
            break_cost: 0,
        }
    }

    #[test]
    fn save_and_compare_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let noupling_dir = dir.path().join(".noupling");
        std::fs::create_dir_all(&noupling_dir).unwrap();
        // Create a fake history.db so find_db doesn't fail
        std::fs::write(noupling_dir.join("history.db"), "").unwrap();

        let result = AuditResult {
            violations: vec![make_coupling("a.rs", "b.rs"), make_coupling("c.rs", "d.rs")],
            score: 50.0,
            total_modules: 4,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
        };

        // Save baseline
        save_baseline(dir.path(), &result).unwrap();
        assert!(dir.path().join(".noupling/baseline.json").exists());

        // Same violations = all existing, none new
        let mut same_result = AuditResult {
            violations: vec![make_coupling("a.rs", "b.rs"), make_coupling("c.rs", "d.rs")],
            score: 50.0,
            total_modules: 4,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
        };
        let (new, resolved) = compare_baseline(dir.path(), &mut same_result).unwrap();
        assert_eq!(new, 0);
        assert_eq!(resolved, 0);
        assert!(same_result.violations.is_empty());
    }

    #[test]
    fn detects_new_violations() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".noupling")).unwrap();

        let baseline_result = AuditResult {
            violations: vec![make_coupling("a.rs", "b.rs")],
            score: 75.0,
            total_modules: 4,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
        };
        save_baseline(dir.path(), &baseline_result).unwrap();

        // New violation added
        let mut current = AuditResult {
            violations: vec![
                make_coupling("a.rs", "b.rs"), // existing
                make_coupling("x.rs", "y.rs"), // new
            ],
            score: 50.0,
            total_modules: 4,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
        };
        let (new, resolved) = compare_baseline(dir.path(), &mut current).unwrap();
        assert_eq!(new, 1);
        assert_eq!(resolved, 0);
        assert_eq!(current.violations.len(), 1);
        assert_eq!(current.violations[0].from_module, "x.rs");
    }

    #[test]
    fn detects_resolved_violations() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".noupling")).unwrap();

        let baseline_result = AuditResult {
            violations: vec![make_coupling("a.rs", "b.rs"), make_coupling("c.rs", "d.rs")],
            score: 50.0,
            total_modules: 4,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
        };
        save_baseline(dir.path(), &baseline_result).unwrap();

        // One violation resolved
        let mut current = AuditResult {
            violations: vec![make_coupling("a.rs", "b.rs")],
            score: 75.0,
            total_modules: 4,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
        };
        let (new, resolved) = compare_baseline(dir.path(), &mut current).unwrap();
        assert_eq!(new, 0);
        assert_eq!(resolved, 1);
    }
}
