use std::path::Path;

pub fn run(action: &str, path: &str) -> anyhow::Result<()> {
    match action {
        "save" => {
            let db = super::find_db(path)?;
            let snap_repo = crate::storage::repository::SnapshotRepository::new(&db.conn);
            let snapshot = snap_repo
                .get_latest()?
                .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?;

            let module_repo = crate::storage::repository::ModuleRepository::new(&db.conn);
            let dep_repo = crate::storage::repository::DependencyRepository::new(&db.conn);
            let modules = module_repo.get_by_snapshot(&snapshot.id)?;
            let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

            let project_settings = crate::settings::Settings::load(Path::new(path))?;
            let result =
                crate::analyzer::audit_with_settings(&modules, &dependencies, &project_settings);

            crate::baseline::save_baseline(Path::new(path), &result)?;
        }
        _ => {
            anyhow::bail!("Unknown baseline action: {}. Use 'save'.", action);
        }
    }
    Ok(())
}
