use std::path::Path;

pub fn run(path: &str, diff_base: Option<&str>) -> anyhow::Result<()> {
    let project_path = Path::new(path);
    if !project_path.exists() {
        anyhow::bail!("Path does not exist: {}", path);
    }

    // Get changed files if diff mode
    let changed_files = if let Some(base) = diff_base {
        let files = crate::diff::get_changed_files(project_path, base)?;
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
    let db = crate::storage::Database::open(&db_path)?;

    let snap_repo = crate::storage::repository::SnapshotRepository::new(&db.conn);
    let snapshot = snap_repo.create(path)?;
    println!("Created snapshot: {}", snapshot.id);

    // Always scan the full project (needed for dependency resolution)
    let scan_settings = crate::settings::Settings::load(project_path)?;
    let result = crate::scanner::scan_project(
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

    let module_repo = crate::storage::repository::ModuleRepository::new(&db.conn);
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

    let dep_repo = crate::storage::repository::DependencyRepository::new(&db.conn);
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
    let scan_meta = crate::storage::SnapshotMeta {
        suppressed_count: result.suppressed_count,
        diff_base: diff_base.map(|s| s.to_string()),
        diff_changed_files: changed_files.clone(),
        external_deps: result
            .external_imports
            .iter()
            .map(|e| crate::storage::ExternalDepRow {
                module_path: e.module_path.clone(),
                count: e.count,
            })
            .collect(),
    };
    snap_repo.save_meta(&snapshot.id, &scan_meta)?;

    println!("Scan complete. Database: {}", db_path.display());
    Ok(())
}
