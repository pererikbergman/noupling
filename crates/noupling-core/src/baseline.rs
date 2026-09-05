//! Baseline file management for incremental adoption (`CONTEXT.md`
//! § Baseline, #343).
//!
//! `baseline save` fingerprints every Issue (kind + subject) into
//! `.noupling/baseline.json`. A later audit or report run with
//! `--baseline` loads that set and marks matching Issues `baselined`:
//! they are still reported, never dropped, and never counted as new.
//!
//! Older files are not migrated: loading one yields an empty set flagged
//! `legacy_format`, so callers can say "re-run `noupling baseline save`".
//! Version 1 (pre-0.9.0) fingerprinted only coupling violations; version
//! 2 (unreleased) keyed Coupling Violations and Red Flags on a
//! representative import rather than the directory pair.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::analyzer::AuditResult;

/// Current on-disk format. Bumped when the fingerprint scheme changes.
pub const BASELINE_VERSION: u32 = 3;

#[derive(Serialize, Deserialize)]
struct BaselineFile {
    version: u32,
    timestamp: String,
    #[serde(alias = "violation_count")]
    issue_count: usize,
    fingerprints: Vec<String>,
}

/// A loaded baseline: the accepted Issue fingerprints plus whether the
/// file predates the every-kind format.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Baseline {
    /// Fingerprints of accepted Issues (`Issue::fingerprint`).
    pub fingerprints: HashSet<String>,
    /// True when the file was written by a pre-0.9.0 noupling and
    /// therefore covers only coupling violations; `fingerprints` is
    /// empty in that case so nothing is wrongly treated as accepted.
    pub legacy_format: bool,
}

fn baseline_path(project: &Path) -> std::path::PathBuf {
    project.join(".noupling").join("baseline.json")
}

/// Save every Issue in `result` as the accepted baseline. Returns the
/// number of Issues written.
pub fn save_baseline(project: &Path, result: &AuditResult) -> Result<usize> {
    let fingerprints: Vec<String> = result.issues().iter().map(|i| i.fingerprint()).collect();
    let file = BaselineFile {
        version: BASELINE_VERSION,
        timestamp: chrono_now(),
        issue_count: fingerprints.len(),
        fingerprints,
    };
    std::fs::create_dir_all(project.join(".noupling"))?;
    std::fs::write(baseline_path(project), serde_json::to_string_pretty(&file)?)?;
    Ok(file.issue_count)
}

/// Load the baseline for `project`. Errors when no baseline exists or the
/// file is unreadable; an old-format file loads as an empty, legacy set.
pub fn load_baseline(project: &Path) -> Result<Baseline> {
    let path = baseline_path(project);
    if !path.exists() {
        anyhow::bail!(
            "No baseline found at {}. Run `noupling baseline save` first.",
            path.display()
        );
    }
    let content = std::fs::read_to_string(&path)?;
    let file: BaselineFile = serde_json::from_str(&content)
        .with_context(|| format!("{} is not a valid baseline file", path.display()))?;
    if file.version < BASELINE_VERSION {
        return Ok(Baseline {
            fingerprints: HashSet::new(),
            legacy_format: true,
        });
    }
    Ok(Baseline {
        fingerprints: file.fingerprints.into_iter().collect(),
        legacy_format: false,
    })
}

/// How the current Issues compare with an applied baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BaselineComparison {
    /// Issues present now and not in the baseline.
    pub new_count: usize,
    /// Issues present now and accepted by the baseline.
    pub baselined_count: usize,
    /// Baseline fingerprints with no matching Issue any more.
    pub resolved_count: usize,
}

impl AuditResult {
    /// Attach a baseline: every Issue whose fingerprint it contains is
    /// reported `baselined` by [`AuditResult::issues`]. Nothing is
    /// dropped and the score is unchanged. Returns the comparison counts.
    pub fn apply_baseline(&mut self, baseline: &Baseline) -> BaselineComparison {
        self.baseline = Some(baseline.clone());
        let issues = self.issues();
        let current: HashSet<String> = issues.iter().map(|i| i.fingerprint()).collect();
        BaselineComparison {
            new_count: issues.iter().filter(|i| !i.baselined).count(),
            baselined_count: issues.iter().filter(|i| i.baselined).count(),
            resolved_count: baseline
                .fingerprints
                .iter()
                .filter(|f| !current.contains(*f))
                .count(),
        }
    }
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
    use crate::analyzer::{audit_with_settings, IssueKind};
    use crate::core::Dependency;
    use crate::settings::Settings;

    fn fixture_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../noupling-cli/tests/fixtures/every_issue_kind")
    }

    /// Scan + audit the every_issue_kind fixture, optionally with extra
    /// dependencies spliced in.
    fn fixture_audit(
        extra_deps: impl Fn(&[crate::core::Module]) -> Vec<Dependency>,
    ) -> AuditResult {
        let root = fixture_root();
        let settings = Settings::load(&root).unwrap();
        let scan =
            crate::scanner::scan_project(&root, "snap", settings.allow_inline_suppression).unwrap();
        let mut deps = scan.dependencies.clone();
        deps.extend(extra_deps(&scan.modules));
        let type_counts = crate::scanner::recompute_type_counts(&root, &scan.modules);
        audit_with_settings(&scan.modules, &deps, &type_counts, &settings)
    }

    fn project_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".noupling")).unwrap();
        dir
    }

    #[test]
    fn save_then_compare_on_the_fixture_marks_every_kind_baselined_and_nothing_new() {
        let dir = project_dir();
        let result = fixture_audit(|_| vec![]);
        let saved = save_baseline(dir.path(), &result).unwrap();
        assert_eq!(saved, result.issues().len());

        let mut again = fixture_audit(|_| vec![]);
        let cmp = again.apply_baseline(&load_baseline(dir.path()).unwrap());

        assert_eq!(cmp.new_count, 0, "{cmp:?}");
        assert_eq!(cmp.resolved_count, 0, "{cmp:?}");
        assert_eq!(cmp.baselined_count, saved);
        let issues = again.issues();
        for kind in IssueKind::ALL {
            assert!(
                issues
                    .iter()
                    .filter(|i| i.kind() == kind)
                    .all(|i| i.baselined),
                "{kind} must be baselined"
            );
        }
        assert_eq!(
            issues.len(),
            saved,
            "baselined Issues are kept, not dropped"
        );
    }

    #[test]
    fn one_new_sibling_edge_yields_exactly_one_new_issue() {
        let dir = project_dir();
        save_baseline(dir.path(), &fixture_audit(|_| vec![])).unwrap();

        // plugins/exporter.rs → bag/a.rs: a sibling pair the fixture does not have.
        let mut current = fixture_audit(|modules| {
            let id = |path: &str| modules.iter().find(|m| m.path == path).unwrap().id.clone();
            vec![Dependency {
                from_module_id: id("src/plugins/exporter.rs"),
                to_module_id: id("src/bag/a.rs"),
                line_number: 99,
            }]
        });
        let cmp = current.apply_baseline(&load_baseline(dir.path()).unwrap());

        let new: Vec<_> = current
            .issues()
            .into_iter()
            .filter(|i| !i.baselined)
            .collect();
        assert_eq!(
            cmp.new_count,
            1,
            "{:?}",
            new.iter().map(|i| i.fingerprint()).collect::<Vec<_>>()
        );
        assert_eq!(new[0].kind(), IssueKind::CouplingViolation);
        assert_eq!(
            new[0].fingerprint(),
            "coupling_violation:src/plugins -> src/bag"
        );
    }

    /// A Coupling Violation is the directory pair, not whichever import
    /// happens to represent it: dropping the representative import must not
    /// make the pair look new (and its old fingerprint resolved).
    #[test]
    fn removing_the_representative_import_keeps_the_coupling_violation_baselined() {
        let dir = project_dir();
        // fused/left → fused/right has several imports; the representative
        // is the lexicographically smallest (l1.rs → r1.rs).
        save_baseline(dir.path(), &fixture_audit(|_| vec![])).unwrap();

        let mut current = {
            let root = fixture_root();
            let settings = Settings::load(&root).unwrap();
            let scan =
                crate::scanner::scan_project(&root, "snap", settings.allow_inline_suppression)
                    .unwrap();
            let id = |path: &str| {
                scan.modules
                    .iter()
                    .find(|m| m.path == path)
                    .unwrap()
                    .id
                    .clone()
            };
            let (l1, r1) = (id("src/fused/left/l1.rs"), id("src/fused/right/r1.rs"));
            let deps: Vec<Dependency> = scan
                .dependencies
                .iter()
                .filter(|d| !(d.from_module_id == l1 && d.to_module_id == r1))
                .cloned()
                .collect();
            let type_counts = crate::scanner::recompute_type_counts(&root, &scan.modules);
            audit_with_settings(&scan.modules, &deps, &type_counts, &settings)
        };
        let cmp = current.apply_baseline(&load_baseline(dir.path()).unwrap());

        let coupling_new: Vec<String> = current
            .issues()
            .iter()
            .filter(|i| i.kind() == IssueKind::CouplingViolation && !i.baselined)
            .map(|i| i.fingerprint())
            .collect();
        assert!(
            coupling_new.is_empty(),
            "pair must stay baselined: {coupling_new:?}"
        );
        assert_eq!(
            cmp.new_count, 0,
            "the Red Flag on the same pair stays too: {cmp:?}"
        );
    }

    #[test]
    fn issues_carry_no_baselined_flag_until_a_baseline_is_applied() {
        let result = fixture_audit(|_| vec![]);
        assert!(result.baseline.is_none());
        assert!(result.issues().iter().all(|i| !i.baselined));
    }

    #[test]
    fn old_format_file_loads_as_empty_legacy_set_instead_of_failing() {
        let dir = project_dir();
        std::fs::write(
            dir.path().join(".noupling/baseline.json"),
            r#"{"version":1,"timestamp":"0","violation_count":1,"fingerprints":["coupling:a.rs:b.rs"]}"#,
        )
        .unwrap();

        let baseline = load_baseline(dir.path()).unwrap();

        assert!(baseline.legacy_format);
        assert!(baseline.fingerprints.is_empty());
        let mut result = fixture_audit(|_| vec![]);
        let cmp = result.apply_baseline(&baseline);
        assert_eq!(cmp.baselined_count, 0);
        assert_eq!(cmp.new_count, result.issues().len());
    }

    #[test]
    fn missing_file_is_a_clear_error() {
        let dir = project_dir();
        let err = load_baseline(dir.path()).unwrap_err().to_string();
        assert!(err.contains("baseline save"), "{err}");
    }
}
