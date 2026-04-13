use std::path::Path;

/// Resolves an import path to a relative file path within the project.
/// Returns None if the import refers to an external dependency.
pub fn resolve_import(
    import_path: &str,
    source_file: &str,
    _project_root: &Path,
    known_paths: &[String],
) -> Option<String> {
    let ext = Path::new(source_file).extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "rs" => resolve_rust_import(import_path, source_file, known_paths),
        "kt" | "kts" => resolve_kotlin_import(import_path, known_paths),
        _ => None,
    }
}

fn resolve_rust_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    let segments: Vec<&str> = if import_path.starts_with("crate::") {
        import_path.strip_prefix("crate::")?.split("::").collect()
    } else if import_path.starts_with("super::") {
        return resolve_super_import(import_path, source_file, known_paths);
    } else if import_path.starts_with("self::") {
        return resolve_self_import(import_path, source_file, known_paths);
    } else {
        return None;
    };

    let src_root = find_src_root(source_file)?;
    try_resolve_segments(&segments, &src_root, known_paths)
}

/// Resolves a Kotlin import (dot-separated) to a project file path.
/// e.g., "com.example.MyClass" -> "src/main/kotlin/com/example/MyClass.kt"
fn resolve_kotlin_import(
    import_path: &str,
    known_paths: &[String],
) -> Option<String> {
    // Convert dot-separated path to file path segments
    let segments: Vec<&str> = import_path.split('.').collect();
    if segments.is_empty() {
        return None;
    }

    // Try the full path as a .kt file
    let file_path = segments.join("/");
    for ext in &["kt", "kts"] {
        let candidate = format!("{}.{}", file_path, ext);
        // Check if any known path ends with this candidate
        if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
            return Some(found.clone());
        }
    }

    // Try without last segment (it might be a class name, not a file)
    if segments.len() > 1 {
        let parent_path = segments[..segments.len() - 1].join("/");
        for ext in &["kt", "kts"] {
            let candidate = format!("{}.{}", parent_path, ext);
            if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
                return Some(found.clone());
            }
        }
    }

    None
}

fn find_src_root(source_file: &str) -> Option<String> {
    let source_path = Path::new(source_file);
    let mut current = source_path.parent()?;
    loop {
        if current.file_name()?.to_str()? == "src" {
            return Some(current.to_string_lossy().to_string());
        }
        current = current.parent()?;
    }
}

fn try_resolve_segments(
    segments: &[&str],
    src_root: &str,
    known_paths: &[String],
) -> Option<String> {
    if segments.is_empty() {
        return None;
    }

    let base = Path::new(src_root);

    // Try as a file: segments joined as directories, last segment as .rs file
    let mut file_path = base.to_path_buf();
    for (i, seg) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            file_path.push(format!("{}.rs", seg));
        } else {
            file_path.push(seg);
        }
    }
    let candidate = file_path.to_string_lossy().to_string();
    if known_paths.contains(&candidate) {
        return Some(candidate);
    }

    // Try as a module directory with mod.rs
    let mut mod_path = base.to_path_buf();
    for seg in segments {
        mod_path.push(seg);
    }
    mod_path.push("mod.rs");
    let candidate = mod_path.to_string_lossy().to_string();
    if known_paths.contains(&candidate) {
        return Some(candidate);
    }

    // Try without the last segment (it might be a type/function, not a file)
    if segments.len() > 1 {
        let parent_segments = &segments[..segments.len() - 1];
        return try_resolve_segments(parent_segments, src_root, known_paths);
    }

    None
}

fn resolve_super_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    let source_dir = Path::new(source_file).parent()?;
    let remaining = import_path.strip_prefix("super::")?;

    let is_mod_rs = Path::new(source_file)
        .file_name()
        .map(|f| f == "mod.rs" || f == "lib.rs")
        .unwrap_or(false);

    let base = if is_mod_rs {
        source_dir.parent()?
    } else {
        source_dir
    };

    let segments: Vec<&str> = remaining.split("::").collect();
    let src_root = base.to_string_lossy().to_string();

    try_resolve_segments(&segments, &src_root, known_paths)
}

fn resolve_self_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    let source_dir = Path::new(source_file).parent()?;
    let remaining = import_path.strip_prefix("self::")?;

    let segments: Vec<&str> = remaining.split("::").collect();
    let src_root = source_dir.to_string_lossy().to_string();

    try_resolve_segments(&segments, &src_root, known_paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_paths() -> Vec<String> {
        vec![
            "src/main.rs".to_string(),
            "src/core/mod.rs".to_string(),
            "src/core/error.rs".to_string(),
            "src/utils.rs".to_string(),
            "src/slices/scanner/mod.rs".to_string(),
            "src/slices/scanner/parser.rs".to_string(),
        ]
    }

    #[test]
    fn resolves_crate_module_path() {
        let paths = project_paths();
        let root = Path::new("");
        let result = resolve_import("crate::core", "src/main.rs", root, &paths);
        assert_eq!(result, Some("src/core/mod.rs".to_string()));
    }

    #[test]
    fn resolves_crate_file_path() {
        let paths = project_paths();
        let root = Path::new("");
        let result = resolve_import("crate::utils", "src/main.rs", root, &paths);
        assert_eq!(result, Some("src/utils.rs".to_string()));
    }

    #[test]
    fn resolves_crate_nested_path() {
        let paths = project_paths();
        let root = Path::new("");
        let result = resolve_import("crate::core::error", "src/main.rs", root, &paths);
        assert_eq!(result, Some("src/core/error.rs".to_string()));
    }

    #[test]
    fn resolves_crate_type_to_parent_file() {
        let paths = project_paths();
        let root = Path::new("");
        let result = resolve_import("crate::core::error::CoreError", "src/main.rs", root, &paths);
        assert_eq!(result, Some("src/core/error.rs".to_string()));
    }

    #[test]
    fn returns_none_for_external_crate() {
        let paths = project_paths();
        let root = Path::new("");
        let result = resolve_import("std::collections::HashMap", "src/main.rs", root, &paths);
        assert!(result.is_none());
    }

    #[test]
    fn returns_none_for_serde() {
        let paths = project_paths();
        let root = Path::new("");
        let result = resolve_import("serde::Deserialize", "src/main.rs", root, &paths);
        assert!(result.is_none());
    }

    #[test]
    fn resolves_super_import_from_file() {
        let paths = project_paths();
        let root = Path::new("");
        let result = resolve_import("super::parser", "src/slices/scanner/mod.rs", root, &paths);
        // super from mod.rs goes up to slices/, no parser there
        assert!(result.is_none());
    }

    #[test]
    fn resolves_self_import() {
        let paths = project_paths();
        let root = Path::new("");
        let result = resolve_import("self::parser", "src/slices/scanner/mod.rs", root, &paths);
        assert_eq!(result, Some("src/slices/scanner/parser.rs".to_string()));
    }

    // ── Kotlin resolver ──

    fn kotlin_paths() -> Vec<String> {
        vec![
            "app/src/main/kotlin/com/example/MainActivity.kt".to_string(),
            "app/src/main/kotlin/com/example/data/Repository.kt".to_string(),
            "app/src/main/kotlin/com/example/data/Model.kt".to_string(),
            "app/src/main/kotlin/com/example/ui/HomeScreen.kt".to_string(),
        ]
    }

    #[test]
    fn kotlin_resolves_direct_import() {
        let paths = kotlin_paths();
        let root = Path::new("");
        let result = resolve_import(
            "com.example.data.Repository",
            "app/src/main/kotlin/com/example/MainActivity.kt",
            root,
            &paths,
        );
        assert_eq!(
            result,
            Some("app/src/main/kotlin/com/example/data/Repository.kt".to_string())
        );
    }

    #[test]
    fn kotlin_resolves_class_to_file() {
        let paths = kotlin_paths();
        let root = Path::new("");
        // Import a class name that matches the file
        let result = resolve_import(
            "com.example.ui.HomeScreen",
            "app/src/main/kotlin/com/example/MainActivity.kt",
            root,
            &paths,
        );
        assert_eq!(
            result,
            Some("app/src/main/kotlin/com/example/ui/HomeScreen.kt".to_string())
        );
    }

    #[test]
    fn kotlin_returns_none_for_external_dep() {
        let paths = kotlin_paths();
        let root = Path::new("");
        let result = resolve_import(
            "androidx.compose.runtime.Composable",
            "app/src/main/kotlin/com/example/MainActivity.kt",
            root,
            &paths,
        );
        assert!(result.is_none());
    }
}
