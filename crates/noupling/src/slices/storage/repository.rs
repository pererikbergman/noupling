use anyhow::Result;
use rusqlite::Connection;

use crate::core::{Dependency, Node, NodeType, Snapshot};

pub struct SnapshotRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SnapshotRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn create(&self, root_path: &str) -> Result<Snapshot> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO snapshots (id, root_path) VALUES (?1, ?2)",
            rusqlite::params![id, root_path],
        )?;
        let timestamp: String = self.conn.query_row(
            "SELECT timestamp FROM snapshots WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )?;
        Ok(Snapshot {
            id,
            timestamp,
            root_path: root_path.to_string(),
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Snapshot>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, timestamp, root_path FROM snapshots WHERE id = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| {
            Ok(Snapshot {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                root_path: row.get(2)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn get_latest(&self) -> Result<Option<Snapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, root_path FROM snapshots ORDER BY rowid DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map([], |row| {
            Ok(Snapshot {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                root_path: row.get(2)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }
}

pub struct NodeRepository<'a> {
    conn: &'a Connection,
}

impl<'a> NodeRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn bulk_insert(&self, nodes: &[Node]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO nodes (id, snapshot_id, parent_id, name, path, node_type, depth) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for node in nodes {
                let node_type_str = match node.node_type {
                    NodeType::File => "FILE",
                    NodeType::Dir => "DIR",
                };
                stmt.execute(rusqlite::params![
                    node.id,
                    node.snapshot_id,
                    node.parent_id,
                    node.name,
                    node.path,
                    node_type_str,
                    node.depth,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_by_snapshot(&self, snapshot_id: &str) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, snapshot_id, parent_id, name, path, node_type, depth FROM nodes WHERE snapshot_id = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![snapshot_id], |row| {
            let node_type_str: String = row.get(5)?;
            let node_type = match node_type_str.as_str() {
                "FILE" => NodeType::File,
                _ => NodeType::Dir,
            };
            Ok(Node {
                id: row.get(0)?,
                snapshot_id: row.get(1)?,
                parent_id: row.get(2)?,
                name: row.get(3)?,
                path: row.get(4)?,
                node_type,
                depth: row.get(6)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_children(&self, parent_id: &str) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, snapshot_id, parent_id, name, path, node_type, depth FROM nodes WHERE parent_id = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![parent_id], |row| {
            let node_type_str: String = row.get(5)?;
            let node_type = match node_type_str.as_str() {
                "FILE" => NodeType::File,
                _ => NodeType::Dir,
            };
            Ok(Node {
                id: row.get(0)?,
                snapshot_id: row.get(1)?,
                parent_id: row.get(2)?,
                name: row.get(3)?,
                path: row.get(4)?,
                node_type,
                depth: row.get(6)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

pub struct DependencyRepository<'a> {
    conn: &'a Connection,
}

impl<'a> DependencyRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn bulk_insert(&self, deps: &[Dependency]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO dependencies (from_node_id, to_node_id, line_number) VALUES (?1, ?2, ?3)",
            )?;
            for dep in deps {
                stmt.execute(rusqlite::params![
                    dep.from_node_id,
                    dep.to_node_id,
                    dep.line_number,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_by_snapshot(&self, snapshot_id: &str) -> Result<Vec<Dependency>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.from_node_id, d.to_node_id, d.line_number
             FROM dependencies d
             JOIN nodes n ON d.from_node_id = n.id
             WHERE n.snapshot_id = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![snapshot_id], |row| {
            Ok(Dependency {
                from_node_id: row.get(0)?,
                to_node_id: row.get(1)?,
                line_number: row.get(2)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slices::storage::Database;

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    // ── SnapshotRepository ──

    #[test]
    fn snapshot_create_returns_valid_snapshot() {
        let db = setup_db();
        let repo = SnapshotRepository::new(&db.conn);
        let snap = repo.create("/project").unwrap();
        assert!(!snap.id.is_empty());
        assert_eq!(snap.root_path, "/project");
        assert!(!snap.timestamp.is_empty());
    }

    #[test]
    fn snapshot_get_by_id_found() {
        let db = setup_db();
        let repo = SnapshotRepository::new(&db.conn);
        let created = repo.create("/project").unwrap();
        let found = repo.get_by_id(&created.id).unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.root_path, "/project");
    }

    #[test]
    fn snapshot_get_by_id_not_found() {
        let db = setup_db();
        let repo = SnapshotRepository::new(&db.conn);
        let found = repo.get_by_id("nonexistent").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn snapshot_get_latest_returns_most_recent() {
        let db = setup_db();
        let repo = SnapshotRepository::new(&db.conn);
        let _first = repo.create("/project1").unwrap();
        let second = repo.create("/project2").unwrap();
        let latest = repo.get_latest().unwrap().unwrap();
        assert_eq!(latest.root_path, second.root_path);
    }

    #[test]
    fn snapshot_get_latest_empty_db() {
        let db = setup_db();
        let repo = SnapshotRepository::new(&db.conn);
        let latest = repo.get_latest().unwrap();
        assert!(latest.is_none());
    }

    // ── NodeRepository ──

    #[test]
    fn node_bulk_insert_and_get_by_snapshot() {
        let db = setup_db();
        let snap_repo = SnapshotRepository::new(&db.conn);
        let snap = snap_repo.create("/project").unwrap();

        let nodes = vec![
            Node {
                id: "n1".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "src".to_string(),
                path: "/project/src".to_string(),
                node_type: NodeType::Dir,
                depth: 0,
            },
            Node {
                id: "n2".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: Some("n1".to_string()),
                name: "main.rs".to_string(),
                path: "/project/src/main.rs".to_string(),
                node_type: NodeType::File,
                depth: 1,
            },
        ];

        let node_repo = NodeRepository::new(&db.conn);
        node_repo.bulk_insert(&nodes).unwrap();

        let result = node_repo.get_by_snapshot(&snap.id).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn node_get_children() {
        let db = setup_db();
        let snap_repo = SnapshotRepository::new(&db.conn);
        let snap = snap_repo.create("/project").unwrap();

        let nodes = vec![
            Node {
                id: "n1".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "src".to_string(),
                path: "/project/src".to_string(),
                node_type: NodeType::Dir,
                depth: 0,
            },
            Node {
                id: "n2".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: Some("n1".to_string()),
                name: "main.rs".to_string(),
                path: "/project/src/main.rs".to_string(),
                node_type: NodeType::File,
                depth: 1,
            },
            Node {
                id: "n3".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: Some("n1".to_string()),
                name: "lib.rs".to_string(),
                path: "/project/src/lib.rs".to_string(),
                node_type: NodeType::File,
                depth: 1,
            },
        ];

        let node_repo = NodeRepository::new(&db.conn);
        node_repo.bulk_insert(&nodes).unwrap();

        let children = node_repo.get_children("n1").unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn node_get_children_empty() {
        let db = setup_db();
        let node_repo = NodeRepository::new(&db.conn);
        let children = node_repo.get_children("nonexistent").unwrap();
        assert!(children.is_empty());
    }

    // ── DependencyRepository ──

    #[test]
    fn dependency_bulk_insert_and_get_by_snapshot() {
        let db = setup_db();
        let snap_repo = SnapshotRepository::new(&db.conn);
        let snap = snap_repo.create("/project").unwrap();

        let nodes = vec![
            Node {
                id: "n1".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "a.rs".to_string(),
                path: "/project/a.rs".to_string(),
                node_type: NodeType::File,
                depth: 0,
            },
            Node {
                id: "n2".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "b.rs".to_string(),
                path: "/project/b.rs".to_string(),
                node_type: NodeType::File,
                depth: 0,
            },
        ];
        NodeRepository::new(&db.conn).bulk_insert(&nodes).unwrap();

        let deps = vec![Dependency {
            from_node_id: "n1".to_string(),
            to_node_id: "n2".to_string(),
            line_number: 3,
        }];

        let dep_repo = DependencyRepository::new(&db.conn);
        dep_repo.bulk_insert(&deps).unwrap();

        let result = dep_repo.get_by_snapshot(&snap.id).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].from_node_id, "n1");
        assert_eq!(result[0].to_node_id, "n2");
        assert_eq!(result[0].line_number, 3);
    }

    #[test]
    fn dependency_get_by_snapshot_empty() {
        let db = setup_db();
        let snap_repo = SnapshotRepository::new(&db.conn);
        let snap = snap_repo.create("/project").unwrap();

        let dep_repo = DependencyRepository::new(&db.conn);
        let result = dep_repo.get_by_snapshot(&snap.id).unwrap();
        assert!(result.is_empty());
    }
}
