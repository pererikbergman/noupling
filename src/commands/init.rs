use std::path::Path;

pub fn run(path: &str) -> anyhow::Result<()> {
    let project_path = Path::new(path);
    crate::settings::Settings::write_defaults(project_path)?;
    let settings_path = project_path.join(".noupling").join("settings.json");
    println!("Created {}", settings_path.display());
    println!("Edit this file to customize thresholds, ignored directories, and source extensions.");
    Ok(())
}
