//! Deep module that owns the SQLite database and lends repositories
//! to command handlers (#308). Eighteen callsites used to open a
//! database and construct three `Repository::new(&db.conn)` lines by
//! hand; with the session, each callsite is one `open()` plus typed
//! accessors that lend the repo with the right lifetime.
//!
//! The session also owns the "find or fail" semantics of `find_db` —
//! a missing `.noupling/history.db` is the same error everywhere
//! ("Run `noupling scan` first"), so locality stays in one place.

use anyhow::Result;
use noupling_core::storage::repository::{
    DependencyRepository, ModuleRepository, SnapshotRepository,
};
use noupling_core::storage::Database;
use std::path::Path;

/// A connected SQLite session against a project's `.noupling/history.db`.
/// Construct with `open(path)`; access the typed repos via the
/// accessors. Each accessor returns a repo borrowing the session for
/// its lifetime — cheap to construct, never owns state of its own.
pub struct DatabaseSession {
    db: Database,
}

impl DatabaseSession {
    /// Open the project's history database. Bails with the standard
    /// "run `noupling scan` first" guidance when the file is absent.
    pub fn open(project_path: &str) -> Result<Self> {
        let db_path = Path::new(project_path).join(".noupling").join("history.db");
        if !db_path.exists() {
            anyhow::bail!(
                "No database found at {}. Run `noupling scan <PATH>` first.",
                db_path.display()
            );
        }
        let db = Database::open(&db_path)?;
        Ok(Self { db })
    }

    /// Direct access to the underlying `Database` — only the few
    /// callers that still take `&Database` (notably AuditPipeline)
    /// need this. Everyone else should use the typed accessors.
    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn snapshots(&self) -> SnapshotRepository<'_> {
        SnapshotRepository::new(&self.db.conn)
    }

    pub fn modules(&self) -> ModuleRepository<'_> {
        ModuleRepository::new(&self.db.conn)
    }

    pub fn dependencies(&self) -> DependencyRepository<'_> {
        DependencyRepository::new(&self.db.conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noupling_core::core::{Module, ModuleType};
    use tempfile::TempDir;

    /// Build a project tree with an initialised history.db so the
    /// session can open it. Returns the tempdir + the project path
    /// string so test bodies can call open(path) directly.
    fn project_with_db() -> (TempDir, String) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().to_str().unwrap().to_string();
        std::fs::create_dir_all(tmp.path().join(".noupling")).unwrap();
        // Force schema initialisation by opening once. The session will
        // open again — repeated initialisation is idempotent per the
        // CREATE TABLE IF NOT EXISTS pattern in storage::db.
        let db_path = tmp.path().join(".noupling").join("history.db");
        let _ = Database::open(&db_path).expect("init schema");
        (tmp, project)
    }

    #[test]
    fn open_returns_session_for_existing_database() {
        let (_tmp, project) = project_with_db();
        let session = DatabaseSession::open(&project).expect("open session");
        // Smoke: a fresh db has no snapshots.
        let snaps = session.snapshots().get_all().expect("list snaps");
        assert!(snaps.is_empty());
    }

    #[test]
    fn open_bails_when_history_db_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().to_str().unwrap().to_string();
        let result = DatabaseSession::open(&project);
        match result {
            Ok(_) => panic!("expected error when history.db is missing"),
            Err(err) => assert!(
                err.to_string().contains("Run `noupling scan <PATH>` first"),
                "got: {}",
                err
            ),
        }
    }

    #[test]
    fn accessors_return_independent_repo_views_borrowing_the_session() {
        // Two repos from one session must both work without conflict —
        // the session is the seam, not the individual repos.
        let (_tmp, project) = project_with_db();
        let session = DatabaseSession::open(&project).expect("open session");

        let snap = session.snapshots().create("/tmp/example").expect("snap");
        session
            .modules()
            .bulk_insert(&[Module {
                id: "m-1".into(),
                snapshot_id: snap.id.clone(),
                parent_id: None,
                name: "main.rs".into(),
                path: "src/main.rs".into(),
                module_type: ModuleType::File,
                depth: 1,
            }])
            .expect("insert");

        let loaded = session
            .modules()
            .get_by_snapshot(&snap.id)
            .expect("load modules");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].path, "src/main.rs");
    }
}
