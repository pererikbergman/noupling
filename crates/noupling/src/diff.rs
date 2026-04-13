use anyhow::Result;
use std::path::Path;
use std::process::Command;

/// Get the list of files changed compared to a base branch.
/// Returns relative paths from the project root.
pub fn get_changed_files(project_root: &Path, base_branch: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", &format!("{}...HEAD", base_branch)])
        .current_dir(project_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "git diff failed (base: {}): {}. Make sure the branch exists and you are in a git repository.",
            base_branch,
            stderr.trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fails_on_invalid_branch() {
        let dir = tempfile::tempdir().unwrap();
        // Not a git repo, should fail
        let result = get_changed_files(dir.path(), "main");
        assert!(result.is_err());
    }
}
