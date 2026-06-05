use std::path::Path;

pub fn run(action: &str, path: &str) -> anyhow::Result<()> {
    match action {
        "save" => {
            let session = crate::db_session::DatabaseSession::open(path)?;
            let snapshot = session
                .snapshots()
                .get_latest()?
                .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?;
            let modules = session.modules().get_by_snapshot(&snapshot.id)?;
            let dependencies = session.dependencies().get_by_snapshot(&snapshot.id)?;

            let project_settings = noupling_core::settings::Settings::load(Path::new(path))?;
            let type_counts =
                noupling_core::scanner::recompute_type_counts(Path::new(path), &modules);
            let result = noupling_core::analyzer::audit_with_settings(
                &modules,
                &dependencies,
                &type_counts,
                &project_settings,
            );

            noupling_core::baseline::save_baseline(Path::new(path), &result)?;
        }
        _ => {
            anyhow::bail!("Unknown baseline action: {}. Use 'save'.", action);
        }
    }
    Ok(())
}
