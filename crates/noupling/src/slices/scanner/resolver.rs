use std::path::Path;

/// Resolves a Rust import path to a relative file path within the project.
/// Returns None if the import refers to an external crate.
pub fn resolve_import(
    import_path: &str,
    source_file: &str,
    _project_root: &Path,
    known_paths: &[String],
) -> Option<String> {
    // Only resolve crate-internal imports
    let segments: Vec<&str> = if import_path.starts_with("crate::") {
        import_path.strip_prefix("crate::")?.split("::").collect()
    } else if import_path.starts_with("super::") {
        return resolve_super_import(import_path, source_file, known_paths);
    } else if import_path.starts_with("self::") {
        return resolve_self_import(import_path, source_file, known_paths);
    } else {
        // External crate import (std, serde, etc.)
        return None;
    };

    // Find the src/ root within the source file path
    let src_root = find_src_root(source_file)?;

    // Try to resolve the path segments to a file
    try_resolve_segments(&segments, &src_root, known_paths)
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
}
