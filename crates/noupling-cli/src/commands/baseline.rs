use std::path::Path;

pub fn run(action: &str, path: &str) -> anyhow::Result<()> {
    match action {
        "save" => {
            let session = crate::db_session::DatabaseSession::open(path)?;
            let project_settings = noupling_core::settings::Settings::load(Path::new(path))?;
            // Same pipeline as `audit` / `report`, so the saved Issues are
            // exactly the ones those commands will compare against.
            let pipeline = crate::audit_pipeline::AuditPipeline::new(
                Path::new(path),
                session.db(),
                &project_settings,
            );
            let outcome = pipeline.run(crate::audit_pipeline::PipelineOptions::default())?;
            let saved = noupling_core::baseline::save_baseline(Path::new(path), &outcome.result)?;
            println!(
                "Baseline saved with {} issue{} to {}",
                saved,
                if saved == 1 { "" } else { "s" },
                Path::new(path)
                    .join(".noupling")
                    .join("baseline.json")
                    .display()
            );
        }
        _ => {
            anyhow::bail!("Unknown baseline action: {}. Use 'save'.", action);
        }
    }
    Ok(())
}
