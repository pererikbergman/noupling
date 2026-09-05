//! Deep module pulling the load → filter → audit → enrich ladder out
//! of the three callers (`audit`, `report`, `trend`) that previously
//! inlined it. Issue #304.
//!
//! The interface is one method (`run`) plus an options struct. Behind
//! it sit five stages — snapshot resolution, monorepo filtering, audit
//! invocation, scan-meta enrichment, diff-filter application — each
//! of which used to live as ~30 lines of copy-pasted boilerplate in
//! every caller. Locality: when an invariant changes (e.g. how the
//! diff filter interacts with external deps), it changes once.

use anyhow::{Context, Result};
use noupling_core::analyzer::{self, AuditResult, ExternalDepMetric};
use noupling_core::baseline::{load_baseline, BaselineComparison};
use noupling_core::core::{Dependency, Module, Snapshot};
use noupling_core::scanner;
use noupling_core::settings::Settings;
use noupling_core::storage::repository::{
    DependencyRepository, ModuleRepository, SnapshotRepository,
};
use noupling_core::storage::Database;
use std::collections::HashSet;
use std::path::Path;

/// Knobs that vary per call site. Defaults run against the latest
/// snapshot with no monorepo split — what `report` does. `audit`
/// supplies a specific snapshot id; both can supply a module filter.
#[derive(Debug, Default)]
pub struct PipelineOptions<'a> {
    /// Snapshot to audit. `None` picks the most recent one (the
    /// "latest" semantics that `report` and the no-arg form of
    /// `audit` rely on).
    pub snapshot_id: Option<&'a str>,
    /// Monorepo split — restrict the audit to one configured module
    /// from `settings.modules`. None = audit the whole codebase.
    pub module_filter: Option<&'a str>,
    /// Apply `.noupling/baseline.json`: matching Issues are marked
    /// `baselined` (never dropped) and the outcome carries the
    /// comparison counts. Errors when no baseline file exists.
    pub baseline: bool,
}

/// Everything a caller needs after the pipeline runs. The fields
/// audit/report previously assembled by hand are now returned in one
/// shape, so post-pipeline steps (violation-age, baseline compare,
/// format dispatch) stay clearly distinct from the shared ladder.
#[derive(Debug)]
pub struct PipelineOutcome {
    pub snapshot: Snapshot,
    pub modules: Vec<Module>,
    pub dependencies: Vec<Dependency>,
    pub result: AuditResult,
    /// Not-in-baseline / baselined / resolved counts when `baseline` was requested.
    pub baseline: Option<BaselineComparison>,
    /// The snapshot's whole-project trend point — score and per-kind Issue
    /// counts captured *before* any diff filter — or `None` when a
    /// `--module` filter made the run a subset of the snapshot. This is
    /// what `record_snapshot_trends` persists, so a CI project that only
    /// runs `scan --diff-base` still keeps its history.
    pub trend: Option<SnapshotTrend>,
}

/// One snapshot's trend point (#349).
#[derive(Debug, Clone)]
pub struct SnapshotTrend {
    pub score: f64,
    pub kind_counts: std::collections::BTreeMap<String, usize>,
    /// The layers this audit inferred (empty when configured or none), kept
    /// on the snapshot so the next audit can reuse them (#355).
    pub inferred_layers: Vec<noupling_core::settings::Layer>,
}

impl SnapshotTrend {
    fn of(result: &AuditResult) -> Self {
        let issues = result.issues();
        SnapshotTrend {
            inferred_layers: if result.layers_auto_detected {
                result.layers.clone()
            } else {
                Vec::new()
            },
            score: result.score,
            kind_counts: noupling_core::analyzer::IssueKind::ALL
                .iter()
                .map(|k| {
                    (
                        k.id().to_string(),
                        issues.iter().filter(|i| i.kind() == *k).count(),
                    )
                })
                .collect(),
        }
    }
}

/// The seam. Owns the project path + db + settings for the duration
/// of one or more `run()` calls.
pub struct AuditPipeline<'a> {
    project_path: &'a Path,
    db: &'a Database,
    settings: &'a Settings,
}

impl<'a> AuditPipeline<'a> {
    pub fn new(project_path: &'a Path, db: &'a Database, settings: &'a Settings) -> Self {
        Self {
            project_path,
            db,
            settings,
        }
    }

    pub fn run(&self, options: PipelineOptions<'_>) -> Result<PipelineOutcome> {
        let snap_repo = SnapshotRepository::new(&self.db.conn);
        let module_repo = ModuleRepository::new(&self.db.conn);
        let dep_repo = DependencyRepository::new(&self.db.conn);

        // (1) Load — resolve snapshot, then its modules + dependencies.
        let snapshot = match options.snapshot_id {
            Some(id) => snap_repo
                .get_by_id(id)?
                .with_context(|| format!("Snapshot not found: {}", id))?,
            None => snap_repo
                .get_latest()?
                .context("No snapshots found. Run `noupling scan` first.")?,
        };
        let modules = module_repo.get_by_snapshot(&snapshot.id)?;
        let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

        // (2) Filter — apply the monorepo module split if requested.
        let (filtered_modules, filtered_deps) = apply_module_filter(
            &modules,
            &dependencies,
            self.settings,
            options.module_filter,
        )?;

        // (3) Audit — single-project pipeline through the analyzer. Layer
        // resolution (configured or inferred, ADR 0001) and the rule /
        // layer violation checks live inside `audit_with_settings`, so
        // every caller sees the same Issues.
        let type_counts = scanner::recompute_type_counts(self.project_path, &filtered_modules);
        // Inference hysteresis (#355): the layers the previous snapshot
        // inferred stay in force while they still fit, so one commit near
        // the coverage threshold cannot flip the audit's mode. Whole-
        // snapshot runs only; a --module run is not the snapshot.
        let prior_inferred = if options.module_filter.is_none() {
            snap_repo
                .get_previous_inferred_layers(&snapshot.id)
                .unwrap_or(None)
        } else {
            None
        };
        let mut result = analyzer::audit_with_settings_and_prior_layers(
            &filtered_modules,
            &filtered_deps,
            &type_counts,
            self.settings,
            prior_inferred.as_deref(),
        );

        // (4) Enrich with scan-time metadata (suppressed count + external
        // deps) recorded at scan time.
        let scan_meta = snap_repo.get_meta(&snapshot.id)?;
        result.suppressed_count = scan_meta.suppressed_count;
        result.external_deps = scan_meta
            .external_deps
            .iter()
            .map(|e| ExternalDepMetric {
                module_path: e.module_path.clone(),
                count: e.count,
            })
            .collect();
        result.total_external_imports = result.external_deps.iter().map(|e| e.count).sum();

        // Whole-snapshot trend point, taken before the diff filter narrows
        // the result. A module filter means this run is not the snapshot.
        let trend = if options.module_filter.is_none() {
            Some(SnapshotTrend::of(&result))
        } else {
            None
        };

        // (5) Diff filter — if the scan ran against a diff base, narrow
        // the result to the files that changed.
        if let Some(ref changed_files) = scan_meta.diff_changed_files {
            result.filter_by_changed_files(changed_files, &self.settings.risk_weights);
        }

        // (6) Baseline — mark accepted Issues; never drop them.
        let baseline = if options.baseline {
            let loaded = load_baseline(self.project_path)?;
            if loaded.legacy_format {
                eprintln!(
                    "warning: .noupling/baseline.json is in the pre-0.9.0 format (coupling only); \
                     nothing is treated as baselined — re-run `noupling baseline save`"
                );
            }
            Some(result.apply_baseline(&loaded))
        } else {
            None
        };

        Ok(PipelineOutcome {
            snapshot,
            modules: filtered_modules,
            dependencies: filtered_deps,
            result,
            baseline,
            trend,
        })
    }
}

/// Record what the strategy report and the Explorer's history scrubber
/// read later: the snapshot's whole-project score and per-kind Issue
/// counts (#349). Best-effort — a failed write must not fail the command.
/// Nothing is written for a `--module` run (`outcome.trend` is `None`).
pub fn record_snapshot_trends(snap_repo: &SnapshotRepository<'_>, outcome: &PipelineOutcome) {
    let Some(trend) = &outcome.trend else {
        return;
    };
    let snapshot_id = outcome.snapshot.id.as_str();
    let _ = snap_repo.save_health_score(snapshot_id, trend.score);
    let _ = snap_repo.save_issue_kind_counts(snapshot_id, &trend.kind_counts);
    let _ = snap_repo.save_inferred_layers(snapshot_id, &trend.inferred_layers);
}

fn apply_module_filter(
    modules: &[Module],
    dependencies: &[Dependency],
    settings: &Settings,
    module_filter: Option<&str>,
) -> Result<(Vec<Module>, Vec<Dependency>)> {
    let Some(name) = module_filter else {
        return Ok((modules.to_vec(), dependencies.to_vec()));
    };
    let cfg = settings
        .modules
        .iter()
        .find(|m| m.name == name)
        .with_context(|| format!("Module '{}' not found in settings", name))?;
    let prefix = format!("{}/", cfg.path);
    let filtered_modules: Vec<_> = modules
        .iter()
        .filter(|m| m.path.starts_with(&prefix) || m.path == cfg.path)
        .cloned()
        .collect();
    let ids: HashSet<&str> = filtered_modules.iter().map(|m| m.id.as_str()).collect();
    let filtered_deps: Vec<_> = dependencies
        .iter()
        .filter(|d| {
            ids.contains(d.from_module_id.as_str()) && ids.contains(d.to_module_id.as_str())
        })
        .cloned()
        .collect();
    Ok((filtered_modules, filtered_deps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use noupling_core::core::ModuleType;
    use noupling_core::storage::{ExternalDepRow, SnapshotMeta};
    use tempfile::TempDir;

    /// Spin up a real on-disk SQLite (in a tempdir) — Database's
    /// `open_in_memory` is `#[cfg(test)]` inside its own crate, so
    /// downstream crates have to use the on-disk path. Snapshot, two
    /// modules, one dependency.
    struct Fixture {
        _tmp: TempDir,
        db: Database,
        snapshot: Snapshot,
    }

    fn small_project() -> Fixture {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".noupling")).unwrap();
        let db_path = tmp.path().join(".noupling").join("history.db");
        let db = Database::open(&db_path).expect("open db");
        let snap_repo = SnapshotRepository::new(&db.conn);
        let snapshot = snap_repo
            .create(tmp.path().to_str().unwrap())
            .expect("snap");
        let modules = vec![
            Module {
                id: "m-main".into(),
                snapshot_id: snapshot.id.clone(),
                parent_id: None,
                name: "main.rs".into(),
                path: "src/main.rs".into(),
                module_type: ModuleType::File,
                depth: 1,
            },
            Module {
                id: "m-helper".into(),
                snapshot_id: snapshot.id.clone(),
                parent_id: None,
                name: "helper.rs".into(),
                path: "src/helper.rs".into(),
                module_type: ModuleType::File,
                depth: 1,
            },
        ];
        ModuleRepository::new(&db.conn)
            .bulk_insert(&modules)
            .unwrap();
        DependencyRepository::new(&db.conn)
            .bulk_insert(&[Dependency {
                from_module_id: "m-main".into(),
                to_module_id: "m-helper".into(),
                line_number: 1,
            }])
            .unwrap();
        Fixture {
            _tmp: tmp,
            db,
            snapshot,
        }
    }

    fn empty_settings() -> Settings {
        serde_json::from_str("{}").expect("default settings")
    }

    #[test]
    fn pipeline_resolves_latest_snapshot_and_loads_its_modules_and_deps() {
        let f = small_project();
        let settings = empty_settings();
        let pipeline = AuditPipeline::new(Path::new("/tmp"), &f.db, &settings);

        let outcome = pipeline.run(PipelineOptions::default()).expect("run");

        assert_eq!(outcome.snapshot.id, f.snapshot.id);
        assert_eq!(outcome.modules.len(), 2);
        assert_eq!(outcome.dependencies.len(), 1);
        assert!(outcome.result.score > 0.0);
    }

    #[test]
    fn pipeline_enriches_result_with_scan_meta() {
        let f = small_project();
        SnapshotRepository::new(&f.db.conn)
            .save_meta(
                &f.snapshot.id,
                &SnapshotMeta {
                    suppressed_count: 7,
                    diff_base: None,
                    diff_changed_files: None,
                    external_deps: vec![ExternalDepRow {
                        module_path: "src/main.rs".into(),
                        count: 3,
                    }],
                },
            )
            .expect("save meta");
        let settings = empty_settings();
        let pipeline = AuditPipeline::new(Path::new("/tmp"), &f.db, &settings);

        let outcome = pipeline.run(PipelineOptions::default()).expect("run");

        assert_eq!(outcome.result.suppressed_count, 7);
        assert_eq!(outcome.result.external_deps.len(), 1);
        assert_eq!(outcome.result.total_external_imports, 3);
    }

    #[test]
    fn pipeline_applies_diff_filter_when_scan_meta_records_changed_files() {
        let f = small_project();
        SnapshotRepository::new(&f.db.conn)
            .save_meta(
                &f.snapshot.id,
                &SnapshotMeta {
                    suppressed_count: 0,
                    diff_base: Some("HEAD~1".into()),
                    diff_changed_files: Some(vec!["src/main.rs".into()]),
                    external_deps: vec![],
                },
            )
            .expect("save meta");
        let settings = empty_settings();
        let pipeline = AuditPipeline::new(Path::new("/tmp"), &f.db, &settings);

        let outcome = pipeline.run(PipelineOptions::default()).expect("run");

        // Clean fixture has no violations either way — the assertion is
        // that the call succeeded with the diff filter applied without
        // panicking. Behaviour of filter_by_changed_files itself is
        // covered by noupling-core's own tests.
        assert!(outcome.result.violations.is_empty());
    }

    /// A diff-scoped run still records the snapshot's *whole* trend point:
    /// the score and per-kind counts are captured before the diff filter
    /// narrows the result, so a CI project that only ever runs
    /// `scan --diff-base` keeps its history. Module-filtered runs record
    /// nothing (a subset of the snapshot is not the snapshot).
    #[test]
    fn diff_scoped_runs_record_the_whole_snapshot_trend_point() {
        let f = small_project();
        let settings = empty_settings();
        let pipeline = AuditPipeline::new(Path::new("/tmp"), &f.db, &settings);
        let snap_repo = SnapshotRepository::new(&f.db.conn);
        let whole = pipeline.run(PipelineOptions::default()).expect("run");

        snap_repo
            .save_meta(
                &f.snapshot.id,
                &SnapshotMeta {
                    suppressed_count: 0,
                    diff_base: Some("HEAD~1".into()),
                    diff_changed_files: Some(vec!["src/nothing.rs".into()]),
                    external_deps: vec![],
                },
            )
            .unwrap();
        let diff = pipeline.run(PipelineOptions::default()).expect("run");
        let trend = diff
            .trend
            .as_ref()
            .expect("whole-snapshot trend point captured");
        assert!((trend.score - whole.result.score).abs() < f64::EPSILON);
        record_snapshot_trends(&snap_repo, &diff);
        let rows = snap_repo.get_all_with_scores().unwrap();
        assert_eq!(rows.len(), 1, "diff snapshot has a recorded score");
        assert!(snap_repo
            .get_issue_kind_counts(&f.snapshot.id)
            .unwrap()
            .is_some());
    }

    /// A `--module` run is a subset of the snapshot: it carries no trend
    /// point and records nothing.
    #[test]
    fn module_filtered_runs_carry_no_trend_point() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".noupling")).unwrap();
        let db = Database::open(&tmp.path().join(".noupling").join("history.db")).unwrap();
        let snap = SnapshotRepository::new(&db.conn)
            .create(tmp.path().to_str().unwrap())
            .unwrap();
        ModuleRepository::new(&db.conn)
            .bulk_insert(&[Module {
                id: "m-orders".into(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "a.rs".into(),
                path: "services/orders/a.rs".into(),
                module_type: ModuleType::File,
                depth: 2,
            }])
            .unwrap();
        let settings: Settings =
            serde_json::from_str(r#"{"modules":[{"name":"orders","path":"services/orders"}]}"#)
                .unwrap();
        let pipeline = AuditPipeline::new(Path::new("/tmp"), &db, &settings);

        let outcome = pipeline
            .run(PipelineOptions {
                snapshot_id: None,
                module_filter: Some("orders"),
                baseline: false,
            })
            .expect("run");

        assert!(outcome.trend.is_none());
        let snap_repo = SnapshotRepository::new(&db.conn);
        record_snapshot_trends(&snap_repo, &outcome);
        assert!(snap_repo.get_all_with_scores().unwrap().is_empty());
        assert!(snap_repo.get_issue_kind_counts(&snap.id).unwrap().is_none());
    }

    #[test]
    fn pipeline_splits_monorepo_modules_when_filter_supplied() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".noupling")).unwrap();
        let db = Database::open(&tmp.path().join(".noupling").join("history.db")).unwrap();
        let snap = SnapshotRepository::new(&db.conn)
            .create(tmp.path().to_str().unwrap())
            .unwrap();
        ModuleRepository::new(&db.conn)
            .bulk_insert(&[
                Module {
                    id: "m-orders".into(),
                    snapshot_id: snap.id.clone(),
                    parent_id: None,
                    name: "a.rs".into(),
                    path: "services/orders/a.rs".into(),
                    module_type: ModuleType::File,
                    depth: 2,
                },
                Module {
                    id: "m-billing".into(),
                    snapshot_id: snap.id.clone(),
                    parent_id: None,
                    name: "b.rs".into(),
                    path: "services/billing/b.rs".into(),
                    module_type: ModuleType::File,
                    depth: 2,
                },
            ])
            .unwrap();
        let settings: Settings =
            serde_json::from_str(r#"{"modules":[{"name":"orders","path":"services/orders"}]}"#)
                .unwrap();

        let pipeline = AuditPipeline::new(Path::new("/tmp"), &db, &settings);
        let outcome = pipeline
            .run(PipelineOptions {
                snapshot_id: None,
                module_filter: Some("orders"),
                baseline: false,
            })
            .expect("run");

        assert_eq!(outcome.modules.len(), 1);
        assert_eq!(outcome.modules[0].path, "services/orders/a.rs");
    }

    #[test]
    fn pipeline_errors_when_unknown_module_filter_supplied() {
        let f = small_project();
        let settings = empty_settings();
        let pipeline = AuditPipeline::new(Path::new("/tmp"), &f.db, &settings);

        let err = pipeline
            .run(PipelineOptions {
                snapshot_id: None,
                module_filter: Some("ghost"),
                baseline: false,
            })
            .unwrap_err();
        assert!(err.to_string().contains("ghost"));
    }
}
