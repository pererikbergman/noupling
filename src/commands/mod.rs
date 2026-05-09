pub mod audit;
pub mod baseline;
pub mod hook;
pub mod init;
pub mod report;
pub mod scan;
pub mod trend;

use std::path::Path;

pub(crate) fn find_db(project_path: &str) -> anyhow::Result<crate::storage::Database> {
    let db_path = Path::new(project_path).join(".noupling").join("history.db");
    if !db_path.exists() {
        anyhow::bail!(
            "No database found at {}. Run `noupling scan <PATH>` first.",
            db_path.display()
        );
    }
    crate::storage::Database::open(&db_path)
}
