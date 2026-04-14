use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModuleType {
    #[serde(rename = "FILE")]
    File,
    #[serde(rename = "DIR")]
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub snapshot_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub path: String,
    pub module_type: ModuleType,
    pub depth: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub from_module_id: String,
    pub to_module_id: String,
    pub line_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub timestamp: String,
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
