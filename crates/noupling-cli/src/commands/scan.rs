use std::path::Path;

pub fn run(path: &str, diff_base: Option<&str>) -> anyhow::Result<()> {
    let project_path = Path::new(path);
    if !project_path.exists() {
        anyhow::bail!("Path does not exist: {}", path);
    }

    // Get changed files if diff mode
    let changed_files = if let Some(base) = diff_base {
        let files = noupling_core::diff::get_changed_files(project_path, base)?;
        println!(
            "Diff mode: {} files changed compared to {}",
            files.len(),
            base
        );
        Some(files)
    } else {
        None
    };

    println!("Scanning: {}", path);

    let db_path = project_path.join(".noupling").join("history.db");
    let db = noupling_core::storage::Database::open(&db_path)?;
    // scan owns its own Database for the initial create-or-open path
    // (DatabaseSession::open expects an existing db). All subsequent
    // commands flow through DatabaseSession.
    let snap_repo = noupling_core::storage::repository::SnapshotRepository::new(&db.conn);
    let snapshot = snap_repo.create(path)?;
    println!("Created snapshot: {}", snapshot.id);

    // Always scan the full project (needed for dependency resolution)
    let scan_settings = noupling_core::settings::Settings::load(project_path)?;
    let result = noupling_core::scanner::scan_project(
        project_path,
        &snapshot.id,
        scan_settings.allow_inline_suppression,
    )?;
    println!("Discovered {} modules", result.modules.len());
    if result.suppressed_count > 0 {
        println!(
            "{} import{} suppressed by noupling:ignore comments",
            result.suppressed_count,
            if result.suppressed_count == 1 {
                ""
            } else {
                "s"
            }
        );
    }

    let module_repo = noupling_core::storage::repository::ModuleRepository::new(&db.conn);
    module_repo.bulk_insert(&result.modules)?;

    let mut unique_deps = result.dependencies;
    unique_deps.sort_by(|a, b| {
        (&a.from_module_id, &a.to_module_id, &a.line_number).cmp(&(
            &b.from_module_id,
            &b.to_module_id,
            &b.line_number,
        ))
    });
    unique_deps.dedup_by(|a, b| {
        a.from_module_id == b.from_module_id
            && a.to_module_id == b.to_module_id
            && a.line_number == b.line_number
    });

    let dep_repo = noupling_core::storage::repository::DependencyRepository::new(&db.conn);
    dep_repo.bulk_insert(&unique_deps)?;
    println!("Found {} dependencies", unique_deps.len());

    // Log external imports summary
    if !result.external_imports.is_empty() {
        let total: usize = result.external_imports.iter().map(|e| e.count).sum();
        println!(
            "{} external (third-party) imports detected across {} modules",
            total,
            result.external_imports.len()
        );
    }

    // Persist scan metadata in SQLite alongside the snapshot
    let scan_meta = noupling_core::storage::SnapshotMeta {
        suppressed_count: result.suppressed_count,
        diff_base: diff_base.map(|s| s.to_string()),
        diff_changed_files: changed_files.clone(),
        external_deps: result
            .external_imports
            .iter()
            .map(|e| noupling_core::storage::ExternalDepRow {
                module_path: e.module_path.clone(),
                count: e.count,
            })
            .collect(),
    };
    snap_repo.save_meta(&snapshot.id, &scan_meta)?;

    // Audit the fresh snapshot once so its score and per-kind Issue counts
    // are on record for trends (#349), whether or not the user ever runs
    // `audit` or `report`. Best-effort: a failure here is not a scan failure.
    if let Ok(outcome) =
        crate::audit_pipeline::AuditPipeline::new(project_path, &db, &scan_settings).run(
            crate::audit_pipeline::PipelineOptions {
                snapshot_id: Some(&snapshot.id),
                ..Default::default()
            },
        )
    {
        crate::audit_pipeline::record_snapshot_trends(&snap_repo, &outcome);
    }

    println!("Scan complete. Database: {}", db_path.display());
    Ok(())
}
