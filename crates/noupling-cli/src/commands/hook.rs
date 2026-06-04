use std::path::Path;

pub fn run(action: &str, path: &str) -> anyhow::Result<()> {
    match action {
        "install" => crate::hook::install(Path::new(path)),
        "uninstall" => crate::hook::uninstall(Path::new(path)),
        _ => anyhow::bail!(
            "Unknown hook action: {}. Use 'install' or 'uninstall'.",
            action
        ),
    }
}
