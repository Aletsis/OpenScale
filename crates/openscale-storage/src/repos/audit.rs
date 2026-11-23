use openscale_domain::AuditLog;
use sqlx::SqlitePool;

use crate::error::StorageError;

#[derive(Debug, Clone)]
pub struct AuditRepository {
    pool: SqlitePool,
}

impl AuditRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn record(&self, entry: &AuditLog) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                id, store_id, actor_id, actor_name, action, target_type, target_id, details_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(entry.id.to_string())
        .bind(entry.store_id.to_string())
        .bind(entry.actor_id.map(|id| id.to_string()))
        .bind(&entry.actor_name)
        .bind(&entry.action)
        .bind(&entry.target_type)
        .bind(&entry.target_id)
        .bind(&entry.details_json)
        .bind(entry.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
