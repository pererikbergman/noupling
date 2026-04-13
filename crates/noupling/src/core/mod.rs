use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    #[serde(rename = "FILE")]
    File,
    #[serde(rename = "DIR")]
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub snapshot_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub path: String,
    pub node_type: NodeType,
    pub depth: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub from_node_id: String,
    pub to_node_id: String,
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
    fn node_type_serializes_to_string() {
        let file_type = super::NodeType::File;
        let dir_type = super::NodeType::Dir;
        assert_eq!(serde_json::to_string(&file_type).unwrap(), "\"FILE\"");
        assert_eq!(serde_json::to_string(&dir_type).unwrap(), "\"DIR\"");
    }

    #[test]
    fn node_type_roundtrip() {
        let original = super::NodeType::File;
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: super::NodeType = serde_json::from_str(&json).unwrap();
        assert_eq!(format!("{:?}", original), format!("{:?}", deserialized));
    }

    #[test]
    fn node_roundtrip() {
        let node = super::Node {
            id: "node-001".to_string(),
            snapshot_id: "snap-001".to_string(),
            parent_id: None,
            name: "src".to_string(),
            path: "/project/src".to_string(),
            node_type: super::NodeType::Dir,
            depth: 0,
        };
        let json = serde_json::to_string(&node).unwrap();
        let deserialized: super::Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node.id, deserialized.id);
        assert_eq!(node.snapshot_id, deserialized.snapshot_id);
        assert!(deserialized.parent_id.is_none());
        assert_eq!(node.name, deserialized.name);
        assert_eq!(node.path, deserialized.path);
        assert_eq!(node.depth, deserialized.depth);
    }

    #[test]
    fn node_with_parent_roundtrip() {
        let node = super::Node {
            id: "node-002".to_string(),
            snapshot_id: "snap-001".to_string(),
            parent_id: Some("node-001".to_string()),
            name: "main.rs".to_string(),
            path: "/project/src/main.rs".to_string(),
            node_type: super::NodeType::File,
            depth: 1,
        };
        let json = serde_json::to_string(&node).unwrap();
        let deserialized: super::Node = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.parent_id, Some("node-001".to_string()));
    }

    #[test]
    fn dependency_roundtrip() {
        let dep = super::Dependency {
            from_node_id: "node-002".to_string(),
            to_node_id: "node-003".to_string(),
            line_number: 5,
        };
        let json = serde_json::to_string(&dep).unwrap();
        let deserialized: super::Dependency = serde_json::from_str(&json).unwrap();
        assert_eq!(dep.from_node_id, deserialized.from_node_id);
        assert_eq!(dep.to_node_id, deserialized.to_node_id);
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
