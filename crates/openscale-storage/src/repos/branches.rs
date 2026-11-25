use chrono::{DateTime, Utc};
use openscale_domain::{Branch, BranchCatalogMode, PluKey, SyncMetadata};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::StorageError;

#[derive(Debug, Clone)]
pub struct BranchRepository {
    pool: SqlitePool,
}

impl BranchRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, branch: &Branch) -> Result<(), StorageError> {
        let mode = catalog_mode_to_str(branch.catalog_mode);
        let updated_at = branch.meta.updated_at.to_rfc3339();
        let deleted_at = branch.meta.deleted_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO branches (
                id, store_id, code, name, address, notes, catalog_mode,
                sync_version, updated_at, deleted_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                code = excluded.code,
                name = excluded.name,
                address = excluded.address,
                notes = excluded.notes,
                catalog_mode = excluded.catalog_mode,
                sync_version = branches.sync_version + 1,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at
            "#,
        )
        .bind(branch.id.to_string())
        .bind(branch.meta.store_id.to_string())
        .bind(&branch.code)
        .bind(&branch.name)
        .bind(&branch.address)
        .bind(&branch.notes)
        .bind(mode)
        .bind(branch.meta.sync_version)
        .bind(updated_at)
        .bind(deleted_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_active(&self, store_id: Uuid) -> Result<Vec<Branch>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, store_id, code, name, address, notes, catalog_mode,
                sync_version, updated_at, deleted_at
            FROM branches
            WHERE store_id = ? AND deleted_at IS NULL
            ORDER BY name ASC
            "#,
        )
        .bind(store_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_branch).collect()
    }

    pub async fn get_by_id(
        &self,
        store_id: Uuid,
        branch_id: Uuid,
    ) -> Result<Option<Branch>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, store_id, code, name, address, notes, catalog_mode,
                sync_version, updated_at, deleted_at
            FROM branches
            WHERE store_id = ? AND id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(store_id.to_string())
        .bind(branch_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_branch).transpose()
    }

    pub async fn soft_delete(&self, store_id: Uuid, branch_id: Uuid) -> Result<bool, StorageError> {
        let now = Utc::now().to_rfc3339();
        let res = sqlx::query(
            r#"
            UPDATE branches
            SET deleted_at = ?, updated_at = ?, sync_version = sync_version + 1
            WHERE store_id = ? AND id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(store_id.to_string())
        .bind(branch_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    pub async fn list_plu_keys(&self, branch_id: Uuid) -> Result<Vec<PluKey>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT did, plu_number
            FROM branch_plus
            WHERE branch_id = ?
            ORDER BY did ASC, plu_number ASC
            "#,
        )
        .bind(branch_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut keys = Vec::with_capacity(rows.len());
        for row in rows {
            let did: i64 = row.try_get("did")?;
            let plu: i64 = row.try_get("plu_number")?;
            if let Ok(key) = PluKey::new(did as u32, plu as u64) {
                keys.push(key);
            }
        }
        Ok(keys)
    }

    /// Replaces the entire set of PLUs assigned to a branch atomically in a transaction.
    pub async fn set_plu_keys(
        &self,
        branch_id: Uuid,
        keys: &[PluKey],
    ) -> Result<usize, StorageError> {
        let mut tx = self.pool.begin().await?;
        let branch_str = branch_id.to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query("DELETE FROM branch_plus WHERE branch_id = ?")
            .bind(&branch_str)
            .execute(&mut *tx)
            .await?;

        for key in keys {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO branch_plus (branch_id, did, plu_number, created_at)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(&branch_str)
            .bind(i64::from(key.did))
            .bind(key.plu as i64)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(keys.len())
    }
}

fn catalog_mode_to_str(mode: BranchCatalogMode) -> &'static str {
    match mode {
        BranchCatalogMode::All => "all",
        BranchCatalogMode::Selected => "selected",
    }
}

fn str_to_catalog_mode(value: &str) -> Result<BranchCatalogMode, StorageError> {
    match value {
        "all" => Ok(BranchCatalogMode::All),
        "selected" => Ok(BranchCatalogMode::Selected),
        other => Err(StorageError::InvalidEnumValue {
            column: "catalog_mode",
            value: other.to_string(),
        }),
    }
}

fn row_to_branch(row: sqlx::sqlite::SqliteRow) -> Result<Branch, StorageError> {
    let id_str: String = row.try_get("id")?;
    let store_id_str: String = row.try_get("store_id")?;
    let mode_str: String = row.try_get("catalog_mode")?;
    let updated_at_str: String = row.try_get("updated_at")?;
    let deleted_at_str: Option<String> = row.try_get("deleted_at")?;

    Ok(Branch {
        id: Uuid::parse_str(&id_str)?,
        code: row.try_get("code")?,
        name: row.try_get("name")?,
        address: row.try_get("address")?,
        notes: row.try_get("notes")?,
        catalog_mode: str_to_catalog_mode(&mode_str)?,
        meta: SyncMetadata {
            store_id: Uuid::parse_str(&store_id_str)?,
            sync_version: row.try_get("sync_version")?,
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)?.with_timezone(&Utc),
            deleted_at: deleted_at_str
                .as_deref()
                .map(|s| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
                .transpose()?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DEFAULT_LOCAL_BRANCH_ID, DEFAULT_LOCAL_STORE_ID, Database};
    use openscale_domain::types::new_id;

    #[tokio::test]
    async fn branch_crud_and_plu_assignment_roundtrip() {
        let db = Database::connect_in_memory().await.unwrap();
        let repo = BranchRepository::new(db.pool().clone());

        let seeded = repo.list_active(DEFAULT_LOCAL_STORE_ID).await.unwrap();
        assert_eq!(seeded.len(), 1);
        assert_eq!(seeded[0].id, DEFAULT_LOCAL_BRANCH_ID);
        assert_eq!(seeded[0].catalog_mode, BranchCatalogMode::All);

        let branch = Branch {
            id: new_id(),
            code: "SUC02".to_string(),
            name: "Sucursal Norte".to_string(),
            address: Some("Av. Reforma 123".to_string()),
            notes: None,
            catalog_mode: BranchCatalogMode::Selected,
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        };
        repo.upsert(&branch).await.unwrap();

        let keys = vec![PluKey::new(1, 100).unwrap(), PluKey::new(2, 200).unwrap()];
        assert_eq!(repo.set_plu_keys(branch.id, &keys).await.unwrap(), 2);
        assert_eq!(repo.list_plu_keys(branch.id).await.unwrap(), keys);

        assert!(
            repo.soft_delete(DEFAULT_LOCAL_STORE_ID, branch.id)
                .await
                .unwrap()
        );
        assert_eq!(
            repo.list_active(DEFAULT_LOCAL_STORE_ID)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
