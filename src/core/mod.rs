//! Core domain types shared across all modules.

use serde::{Deserialize, Serialize};

/// Whether a discovered module is a file or directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModuleType {
    #[serde(rename = "FILE")]
    File,
    #[serde(rename = "DIR")]
    Dir,
}

/// A source file discovered during scanning.
///
/// Represents a single source file with its relative path, name,
/// and depth within the project tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// The snapshot this module belongs to.
    pub snapshot_id: String,
    /// Parent module ID (unused in current flat structure).
    pub parent_id: Option<String>,
    /// File name (e.g., `main.rs`).
    pub name: String,
    /// Relative path from project root (e.g., `src/scanner/parser.rs`).
    pub path: String,
    /// Whether this is a file or directory.
    pub module_type: ModuleType,
    /// Depth from the project root (number of path components).
    pub depth: i32,
}

/// An import dependency between two modules.
///
/// Records that `from_module_id` imports something from `to_module_id`
/// at a specific line number in the source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// The module containing the import statement.
    pub from_module_id: String,
    /// The module being imported.
    pub to_module_id: String,
    /// Line number of the import statement in the source file.
    pub line_number: i32,
}

/// A point-in-time scan of a project.
///
/// Each scan creates a new snapshot with a unique ID, allowing
/// historical comparison of architectural health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// When the snapshot was created (SQLite CURRENT_TIMESTAMP).
    pub timestamp: String,
    /// The project root path that was scanned.
    pub root_path: String,
}

#[cfg(test)]
mod tests {
    use serde_json;

    #[test]
    fn module_type_serializes_to_string() {
        let file_type = super::ModuleType::File;
        let dir_type = super::ModuleType::Dir;
        assert_eq!(serde_json::to_string(&file_type).unwrap(), "\"FILE\"");
        assert_eq!(serde_json::to_string(&dir_type).unwrap(), "\"DIR\"");
    }

    #[test]
    fn module_type_roundtrip() {
        let original = super::ModuleType::File;
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: super::ModuleType = serde_json::from_str(&json).unwrap();
        assert_eq!(format!("{:?}", original), format!("{:?}", deserialized));
    }

    #[test]
    fn module_roundtrip() {
        let module = super::Module {
            id: "mod-001".to_string(),
            snapshot_id: "snap-001".to_string(),
            parent_id: None,
            name: "src".to_string(),
            path: "src".to_string(),
            module_type: super::ModuleType::Dir,
            depth: 0,
        };
        let json = serde_json::to_string(&module).unwrap();
        let deserialized: super::Module = serde_json::from_str(&json).unwrap();
        assert_eq!(module.id, deserialized.id);
        assert_eq!(module.snapshot_id, deserialized.snapshot_id);
        assert!(deserialized.parent_id.is_none());
        assert_eq!(module.name, deserialized.name);
        assert_eq!(module.path, deserialized.path);
        assert_eq!(module.depth, deserialized.depth);
    }

    #[test]
    fn module_with_parent_roundtrip() {
        let module = super::Module {
            id: "mod-002".to_string(),
            snapshot_id: "snap-001".to_string(),
            parent_id: Some("mod-001".to_string()),
            name: "main.rs".to_string(),
            path: "src/main.rs".to_string(),
            module_type: super::ModuleType::File,
            depth: 1,
        };
        let json = serde_json::to_string(&module).unwrap();
        let deserialized: super::Module = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.parent_id, Some("mod-001".to_string()));
    }

    #[test]
    fn dependency_roundtrip() {
        let dep = super::Dependency {
            from_module_id: "mod-002".to_string(),
            to_module_id: "mod-003".to_string(),
            line_number: 5,
        };
        let json = serde_json::to_string(&dep).unwrap();
        let deserialized: super::Dependency = serde_json::from_str(&json).unwrap();
        assert_eq!(dep.from_module_id, deserialized.from_module_id);
        assert_eq!(dep.to_module_id, deserialized.to_module_id);
        assert_eq!(dep.line_number, deserialized.line_number);
    }

    #[test]
    fn snapshot_roundtrip() {
        let snap = super::Snapshot {
            id: "snap-001".to_string(),
            timestamp: "2026-04-13T12:00:00".to_string(),
            root_path: "/project".to_string(),
        };
        let json = serde_json::to_string(&snap).unwrap();
        let deserialized: super::Snapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap.id, deserialized.id);
        assert_eq!(snap.timestamp, deserialized.timestamp);
        assert_eq!(snap.root_path, deserialized.root_path);
    }
}
