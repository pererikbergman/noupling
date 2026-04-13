use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let db = Self { conn };
        db.initialize_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.initialize_schema()?;
        Ok(db)
    }

    fn initialize_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id TEXT PRIMARY KEY,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                root_path TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                snapshot_id TEXT REFERENCES snapshots(id),
                parent_id TEXT REFERENCES nodes(id),
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                node_type TEXT CHECK(node_type IN ('FILE', 'DIR')),
                depth INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS dependencies (
                from_node_id TEXT REFERENCES nodes(id),
                to_node_id TEXT REFERENCES nodes(id),
                line_number INTEGER,
                PRIMARY KEY (from_node_id, to_node_id, line_number)
            );",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_all_tables() {
        let db = Database::open_in_memory().unwrap();
        let tables: Vec<String> = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(tables.contains(&"snapshots".to_string()));
        assert!(tables.contains(&"nodes".to_string()));
        assert!(tables.contains(&"dependencies".to_string()));
    }

    #[test]
    fn schema_is_idempotent() {
        let db = Database::open_in_memory().unwrap();
        // Call initialize again via re-open pattern
        db.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS snapshots (
                    id TEXT PRIMARY KEY,
                    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                    root_path TEXT NOT NULL
                );",
            )
            .unwrap();
    }

    #[test]
    fn creates_db_file_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join(".noupling").join("history.db");
        let _db = Database::open(&db_path).unwrap();
        assert!(db_path.exists());
    }

    #[test]
    fn snapshots_table_has_correct_columns() {
        let db = Database::open_in_memory().unwrap();
        let columns: Vec<String> = db
            .conn
            .prepare("PRAGMA table_info(snapshots)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(columns, vec!["id", "timestamp", "root_path"]);
    }

    #[test]
    fn nodes_table_has_correct_columns() {
        let db = Database::open_in_memory().unwrap();
        let columns: Vec<String> = db
            .conn
            .prepare("PRAGMA table_info(nodes)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(
            columns,
            vec!["id", "snapshot_id", "parent_id", "name", "path", "node_type", "depth"]
        );
    }

    #[test]
    fn dependencies_table_has_correct_columns() {
        let db = Database::open_in_memory().unwrap();
        let columns: Vec<String> = db
            .conn
            .prepare("PRAGMA table_info(dependencies)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(columns, vec!["from_node_id", "to_node_id", "line_number"]);
    }
}
