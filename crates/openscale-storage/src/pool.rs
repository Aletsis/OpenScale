use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use uuid::Uuid;

use crate::error::StorageError;

/// Default local store ID seeded by `0001_init.sql`.
pub const DEFAULT_LOCAL_STORE_ID: Uuid = Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0001);

/// Default local branch ID seeded by `0002_branches_and_settings.sql`.
pub const DEFAULT_LOCAL_BRANCH_ID: Uuid =
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0010);

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub path: PathBuf,
    pub max_connections: u32,
    pub busy_timeout: Duration,
}

impl StorageConfig {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            max_connections: 8,
            busy_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Opens a persistent SQLite database configured with WAL mode and foreign keys.
    pub async fn connect(config: &StorageConfig) -> Result<Self, StorageError> {
        let options = SqliteConnectOptions::new()
            .filename(&config.path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(config.busy_timeout);

        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Opens an ephemeral in-memory SQLite database (ideal for unit & integration tests).
    pub async fn connect_in_memory() -> Result<Self, StorageError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    pub async fn run_migrations(&self) -> Result<(), StorageError> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_create_default_local_store() {
        let db = Database::connect_in_memory().await.unwrap();
        let row: (String, String) = sqlx::query_as("SELECT id, code FROM stores WHERE id = ?")
            .bind(DEFAULT_LOCAL_STORE_ID.to_string())
            .fetch_one(db.pool())
            .await
            .unwrap();

        assert_eq!(row.0, DEFAULT_LOCAL_STORE_ID.to_string());
        assert_eq!(row.1, "LOCAL");
    }
}
