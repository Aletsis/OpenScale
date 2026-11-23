use chrono::{DateTime, Utc};
use openscale_domain::{SalesSnapshot, SyncMetadata};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::StorageError;

#[derive(Debug, Clone)]
pub struct SalesSnapshotRepository {
    pool: SqlitePool,
}

impl SalesSnapshotRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, snapshot: &SalesSnapshot) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO sales_snapshots (
                id, store_id, scale_id, period, captured_at, payload_json, sync_version, updated_at, deleted_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(snapshot.id.to_string())
        .bind(snapshot.meta.store_id.to_string())
        .bind(snapshot.scale_id.to_string())
        .bind(i64::from(snapshot.period))
        .bind(snapshot.captured_at.to_rfc3339())
        .bind(&snapshot.payload_json)
        .bind(snapshot.meta.sync_version)
        .bind(snapshot.meta.updated_at.to_rfc3339())
        .bind(snapshot.meta.deleted_at.map(|d| d.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_recent(
        &self,
        store_id: Uuid,
        scale_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<SalesSnapshot>, StorageError> {
        let rows = if let Some(sid) = scale_id {
            sqlx::query(
                r#"
                SELECT id, store_id, scale_id, period, captured_at, payload_json, sync_version, updated_at, deleted_at
                FROM sales_snapshots
                WHERE store_id = ? AND scale_id = ? AND deleted_at IS NULL
                ORDER BY captured_at DESC
                LIMIT ?
                "#,
            )
            .bind(store_id.to_string())
            .bind(sid.to_string())
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, store_id, scale_id, period, captured_at, payload_json, sync_version, updated_at, deleted_at
                FROM sales_snapshots
                WHERE store_id = ? AND deleted_at IS NULL
                ORDER BY captured_at DESC
                LIMIT ?
                "#,
            )
            .bind(store_id.to_string())
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter()
            .map(|row| {
                let captured_at: String = row.try_get("captured_at")?;
                let updated_at: String = row.try_get("updated_at")?;
                let deleted_at: Option<String> = row.try_get("deleted_at")?;
                Ok(SalesSnapshot {
                    id: Uuid::parse_str(&row.try_get::<String, _>("id")?)?,
                    scale_id: Uuid::parse_str(&row.try_get::<String, _>("scale_id")?)?,
                    period: row.try_get::<i64, _>("period")? as u8,
                    captured_at: DateTime::parse_from_rfc3339(&captured_at)?.with_timezone(&Utc),
                    payload_json: row.try_get("payload_json")?,
                    meta: SyncMetadata {
                        store_id: Uuid::parse_str(&row.try_get::<String, _>("store_id")?)?,
                        sync_version: row.try_get("sync_version")?,
                        updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
                        deleted_at: deleted_at
                            .as_deref()
                            .map(|s| {
                                DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc))
                            })
                            .transpose()?,
                    },
                })
            })
            .collect()
    }
}
