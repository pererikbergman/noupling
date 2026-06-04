//! Git pre-commit hook management.

use anyhow::Result;
use std::path::Path;

const HOOK_MARKER: &str = "# noupling pre-commit hook";

const HOOK_SCRIPT: &str = r#"#!/bin/sh
# noupling pre-commit hook
# Bypass with: git commit --no-verify

noupling scan . --diff-base HEAD
noupling audit . --fail-below 90

if [ $? -ne 0 ]; then
    echo ""
    echo "noupling: commit blocked due to new coupling violations."
    echo "Fix the violations above, or bypass with: git commit --no-verify"
    exit 1
fi
"#;

/// Install the pre-commit hook to `.git/hooks/pre-commit`.
pub fn install(project_path: &Path) -> Result<()> {
    let hooks_dir = project_path.join(".git").join("hooks");
    if !hooks_dir.exists() {
        anyhow::bail!(
            "Not a git repository (no .git/hooks/ found at {})",
            project_path.display()
        );
    }

    let hook_path = hooks_dir.join("pre-commit");

    // Check if hook already exists
    if hook_path.exists() {
        let content = std::fs::read_to_string(&hook_path)?;
        if content.contains(HOOK_MARKER) {
            println!("noupling pre-commit hook is already installed.");
            return Ok(());
        }
        anyhow::bail!(
            "A pre-commit hook already exists at {}. Remove it first or add noupling manually.",
            hook_path.display()
        );
    }

    std::fs::write(&hook_path, HOOK_SCRIPT)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)?;
    }

    println!("Pre-commit hook installed at {}", hook_path.display());
    println!("Bypass with: git commit --no-verify");
    Ok(())
}

/// Remove the noupling pre-commit hook.
pub fn uninstall(project_path: &Path) -> Result<()> {
    let hook_path = project_path.join(".git").join("hooks").join("pre-commit");

    if !hook_path.exists() {
        println!("No pre-commit hook found.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&hook_path)?;
    if !content.contains(HOOK_MARKER) {
        anyhow::bail!(
            "The pre-commit hook at {} was not installed by noupling. Remove it manually.",
            hook_path.display()
        );
    }

    std::fs::remove_file(&hook_path)?;
    println!("Pre-commit hook removed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_uninstall() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_dir = dir.path().join(".git").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        // Install
        install(dir.path()).unwrap();
        let hook_path = hooks_dir.join("pre-commit");
        assert!(hook_path.exists());
        let content = std::fs::read_to_string(&hook_path).unwrap();
        assert!(content.contains("noupling"));

        // Install again = no error
        install(dir.path()).unwrap();

        // Uninstall
        uninstall(dir.path()).unwrap();
        assert!(!hook_path.exists());

        // Uninstall again = no error
        uninstall(dir.path()).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_foreign_hook() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_dir = dir.path().join(".git").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        std::fs::write(hooks_dir.join("pre-commit"), "#!/bin/sh\necho other").unwrap();

        let result = install(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn refuses_to_uninstall_foreign_hook() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_dir = dir.path().join(".git").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        std::fs::write(hooks_dir.join("pre-commit"), "#!/bin/sh\necho other").unwrap();

        let result = uninstall(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn fails_without_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = install(dir.path());
        assert!(result.is_err());
    }
}
