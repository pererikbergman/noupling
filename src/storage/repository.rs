use anyhow::Result;
use rusqlite::Connection;

use crate::core::{Dependency, Module, ModuleType, Snapshot};

/// Scan metadata stored alongside a snapshot in SQLite.
#[derive(Debug, Default)]
pub struct SnapshotMeta {
    /// Number of imports suppressed by `noupling:ignore` comments.
    pub suppressed_count: usize,
    /// The base branch/commit used for diff mode, if any.
    pub diff_base: Option<String>,
    /// Files changed compared to the diff base, if diff mode was used.
    pub diff_changed_files: Option<Vec<String>>,
    /// Per-module count of external (third-party) imports.
    pub external_deps: Vec<ExternalDepRow>,
}

/// One row from snapshot_external_deps.
#[derive(Debug, Clone)]
pub struct ExternalDepRow {
    pub module_path: String,
    pub count: usize,
}

/// Repository for creating and querying scan snapshots.
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

    pub fn get_all(&self) -> Result<Vec<Snapshot>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, timestamp, root_path FROM snapshots ORDER BY rowid")?;
        let rows = stmt.query_map([], |row| {
            Ok(Snapshot {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                root_path: row.get(2)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Persist scan-time metadata for an existing snapshot.
    pub fn save_meta(&self, snapshot_id: &str, meta: &SnapshotMeta) -> Result<()> {
        let diff_changed_files_json = meta
            .diff_changed_files
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.conn.execute(
            "UPDATE snapshots SET suppressed_count = ?1, diff_base = ?2, diff_changed_files = ?3 WHERE id = ?4",
            rusqlite::params![
                meta.suppressed_count as i64,
                meta.diff_base,
                diff_changed_files_json,
                snapshot_id,
            ],
        )?;

        // Insert external dep rows
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO snapshot_external_deps (snapshot_id, module_path, count) VALUES (?1, ?2, ?3)",
            )?;
            for row in &meta.external_deps {
                stmt.execute(rusqlite::params![
                    snapshot_id,
                    row.module_path,
                    row.count as i64
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Load scan-time metadata for a snapshot.
    pub fn get_meta(&self, snapshot_id: &str) -> Result<SnapshotMeta> {
        let (suppressed_count, diff_base, diff_changed_files_json): (i64, Option<String>, Option<String>) = self
            .conn
            .query_row(
                "SELECT suppressed_count, diff_base, diff_changed_files FROM snapshots WHERE id = ?1",
                rusqlite::params![snapshot_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or((0, None, None));

        let diff_changed_files: Option<Vec<String>> = diff_changed_files_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        let mut ext_stmt = self.conn.prepare(
            "SELECT module_path, count FROM snapshot_external_deps WHERE snapshot_id = ?1",
        )?;
        let external_deps: Vec<ExternalDepRow> = ext_stmt
            .query_map(rusqlite::params![snapshot_id], |row| {
                Ok(ExternalDepRow {
                    module_path: row.get(0)?,
                    count: row.get::<_, i64>(1)? as usize,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(SnapshotMeta {
            suppressed_count: suppressed_count as usize,
            diff_base,
            diff_changed_files,
            external_deps,
        })
    }
}

/// Repository for bulk inserting and querying source modules.
pub struct ModuleRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ModuleRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn bulk_insert(&self, modules: &[Module]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO modules (id, snapshot_id, parent_id, name, path, module_type, depth) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for module in modules {
                let module_type_str = match module.module_type {
                    ModuleType::File => "FILE",
                    ModuleType::Dir => "DIR",
                };
                stmt.execute(rusqlite::params![
                    module.id,
                    module.snapshot_id,
                    module.parent_id,
                    module.name,
                    module.path,
                    module_type_str,
                    module.depth,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_by_snapshot(&self, snapshot_id: &str) -> Result<Vec<Module>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, snapshot_id, parent_id, name, path, module_type, depth FROM modules WHERE snapshot_id = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![snapshot_id], |row| {
            let module_type_str: String = row.get(5)?;
            let module_type = match module_type_str.as_str() {
                "FILE" => ModuleType::File,
                _ => ModuleType::Dir,
            };
            Ok(Module {
                id: row.get(0)?,
                snapshot_id: row.get(1)?,
                parent_id: row.get(2)?,
                name: row.get(3)?,
                path: row.get(4)?,
                module_type,
                depth: row.get(6)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    #[allow(dead_code)]
    pub fn get_children(&self, parent_id: &str) -> Result<Vec<Module>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, snapshot_id, parent_id, name, path, module_type, depth FROM modules WHERE parent_id = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![parent_id], |row| {
            let module_type_str: String = row.get(5)?;
            let module_type = match module_type_str.as_str() {
                "FILE" => ModuleType::File,
                _ => ModuleType::Dir,
            };
            Ok(Module {
                id: row.get(0)?,
                snapshot_id: row.get(1)?,
                parent_id: row.get(2)?,
                name: row.get(3)?,
                path: row.get(4)?,
                module_type,
                depth: row.get(6)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

/// Repository for bulk inserting and querying import dependencies.
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
                "INSERT INTO dependencies (from_module_id, to_module_id, line_number) VALUES (?1, ?2, ?3)",
            )?;
            for dep in deps {
                stmt.execute(rusqlite::params![
                    dep.from_module_id,
                    dep.to_module_id,
                    dep.line_number,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_by_snapshot(&self, snapshot_id: &str) -> Result<Vec<Dependency>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.from_module_id, d.to_module_id, d.line_number
             FROM dependencies d
             JOIN modules m ON d.from_module_id = m.id
             WHERE m.snapshot_id = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![snapshot_id], |row| {
            Ok(Dependency {
                from_module_id: row.get(0)?,
                to_module_id: row.get(1)?,
                line_number: row.get(2)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

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

    // ── ModuleRepository ──

    #[test]
    fn module_bulk_insert_and_get_by_snapshot() {
        let db = setup_db();
        let snap_repo = SnapshotRepository::new(&db.conn);
        let snap = snap_repo.create("/project").unwrap();

        let modules = vec![
            Module {
                id: "m1".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "src".to_string(),
                path: "src".to_string(),
                module_type: ModuleType::Dir,
                depth: 0,
            },
            Module {
                id: "m2".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: Some("m1".to_string()),
                name: "main.rs".to_string(),
                path: "src/main.rs".to_string(),
                module_type: ModuleType::File,
                depth: 1,
            },
        ];

        let module_repo = ModuleRepository::new(&db.conn);
        module_repo.bulk_insert(&modules).unwrap();

        let result = module_repo.get_by_snapshot(&snap.id).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn module_get_children() {
        let db = setup_db();
        let snap_repo = SnapshotRepository::new(&db.conn);
        let snap = snap_repo.create("/project").unwrap();

        let modules = vec![
            Module {
                id: "m1".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "src".to_string(),
                path: "src".to_string(),
                module_type: ModuleType::Dir,
                depth: 0,
            },
            Module {
                id: "m2".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: Some("m1".to_string()),
                name: "main.rs".to_string(),
                path: "src/main.rs".to_string(),
                module_type: ModuleType::File,
                depth: 1,
            },
            Module {
                id: "m3".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: Some("m1".to_string()),
                name: "lib.rs".to_string(),
                path: "src/lib.rs".to_string(),
                module_type: ModuleType::File,
                depth: 1,
            },
        ];

        let module_repo = ModuleRepository::new(&db.conn);
        module_repo.bulk_insert(&modules).unwrap();

        let children = module_repo.get_children("m1").unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn module_get_children_empty() {
        let db = setup_db();
        let module_repo = ModuleRepository::new(&db.conn);
        let children = module_repo.get_children("nonexistent").unwrap();
        assert!(children.is_empty());
    }

    // ── DependencyRepository ──

    #[test]
    fn dependency_bulk_insert_and_get_by_snapshot() {
        let db = setup_db();
        let snap_repo = SnapshotRepository::new(&db.conn);
        let snap = snap_repo.create("/project").unwrap();

        let modules = vec![
            Module {
                id: "m1".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "a.rs".to_string(),
                path: "a.rs".to_string(),
                module_type: ModuleType::File,
                depth: 0,
            },
            Module {
                id: "m2".to_string(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "b.rs".to_string(),
                path: "b.rs".to_string(),
                module_type: ModuleType::File,
                depth: 0,
            },
        ];
        ModuleRepository::new(&db.conn)
            .bulk_insert(&modules)
            .unwrap();

        let deps = vec![Dependency {
            from_module_id: "m1".to_string(),
            to_module_id: "m2".to_string(),
            line_number: 3,
        }];

        let dep_repo = DependencyRepository::new(&db.conn);
        dep_repo.bulk_insert(&deps).unwrap();

        let result = dep_repo.get_by_snapshot(&snap.id).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].from_module_id, "m1");
        assert_eq!(result[0].to_module_id, "m2");
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

    // ── SnapshotMeta (scan-time metadata) ──

    #[test]
    fn snapshot_meta_save_and_load_roundtrip() {
        let db = setup_db();
        let repo = SnapshotRepository::new(&db.conn);
        let snap = repo.create("/project").unwrap();

        let meta = SnapshotMeta {
            suppressed_count: 7,
            diff_base: Some("origin/main".to_string()),
            diff_changed_files: Some(vec!["src/foo.rs".to_string(), "src/bar.rs".to_string()]),
            external_deps: vec![
                ExternalDepRow {
                    module_path: "src/main.rs".to_string(),
                    count: 3,
                },
                ExternalDepRow {
                    module_path: "src/lib.rs".to_string(),
                    count: 5,
                },
            ],
        };

        repo.save_meta(&snap.id, &meta).unwrap();
        let loaded = repo.get_meta(&snap.id).unwrap();

        assert_eq!(loaded.suppressed_count, 7);
        assert_eq!(loaded.diff_base.as_deref(), Some("origin/main"));
        let files = loaded.diff_changed_files.unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"src/foo.rs".to_string()));
        assert_eq!(loaded.external_deps.len(), 2);
        let total: usize = loaded.external_deps.iter().map(|e| e.count).sum();
        assert_eq!(total, 8);
    }

    #[test]
    fn snapshot_meta_defaults_when_not_set() {
        let db = setup_db();
        let repo = SnapshotRepository::new(&db.conn);
        let snap = repo.create("/project").unwrap();

        // Don't call save_meta — get_meta should return safe defaults
        let loaded = repo.get_meta(&snap.id).unwrap();
        assert_eq!(loaded.suppressed_count, 0);
        assert!(loaded.diff_base.is_none());
        assert!(loaded.diff_changed_files.is_none());
        assert!(loaded.external_deps.is_empty());
    }
}
