use chrono::{DateTime, Utc};
use openscale_domain::{
    BarcodeFormat, Department, LabelFormat, StoredText, SyncMetadata, Tare, TaxRate,
};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::StorageError;

#[derive(Debug, Clone)]
pub struct MasterTablesRepository {
    pool: SqlitePool,
}

impl MasterTablesRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_department(&self, dept: &Department) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO departments (id, store_id, did, name, sync_version, updated_at, deleted_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(store_id, did) DO UPDATE SET
                name = excluded.name,
                sync_version = departments.sync_version + 1,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at
            "#,
        )
        .bind(dept.id.to_string())
        .bind(dept.meta.store_id.to_string())
        .bind(i64::from(dept.did))
        .bind(&dept.name)
        .bind(dept.meta.sync_version)
        .bind(dept.meta.updated_at.to_rfc3339())
        .bind(dept.meta.deleted_at.map(|d| d.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_departments(&self, store_id: Uuid) -> Result<Vec<Department>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, store_id, did, name, sync_version, updated_at, deleted_at FROM departments WHERE store_id = ? AND deleted_at IS NULL ORDER BY did ASC",
        )
        .bind(store_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(Department {
                    id: Uuid::parse_str(&row.try_get::<String, _>("id")?)?,
                    did: row.try_get::<i64, _>("did")? as u32,
                    name: row.try_get("name")?,
                    meta: parse_meta(&row)?,
                })
            })
            .collect()
    }

    pub async fn upsert_tare(&self, tare: &Tare) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO tares (id, store_id, cid, name, weight_grams, sync_version, updated_at, deleted_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(store_id, cid) DO UPDATE SET
                name = excluded.name,
                weight_grams = excluded.weight_grams,
                sync_version = tares.sync_version + 1,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at
            "#,
        )
        .bind(tare.id.to_string())
        .bind(tare.meta.store_id.to_string())
        .bind(i64::from(tare.cid))
        .bind(&tare.name)
        .bind(i64::from(tare.weight_grams))
        .bind(tare.meta.sync_version)
        .bind(tare.meta.updated_at.to_rfc3339())
        .bind(tare.meta.deleted_at.map(|d| d.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_tares(&self, store_id: Uuid) -> Result<Vec<Tare>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, store_id, cid, name, weight_grams, sync_version, updated_at, deleted_at FROM tares WHERE store_id = ? AND deleted_at IS NULL ORDER BY cid ASC",
        )
        .bind(store_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(Tare {
                    id: Uuid::parse_str(&row.try_get::<String, _>("id")?)?,
                    cid: row.try_get::<i64, _>("cid")? as u16,
                    name: row.try_get("name")?,
                    weight_grams: row.try_get::<i64, _>("weight_grams")? as u32,
                    meta: parse_meta(&row)?,
                })
            })
            .collect()
    }

    pub async fn upsert_tax_rate(&self, tax: &TaxRate) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO tax_rates (id, store_id, tax_id, name, rate_basis_points, sync_version, updated_at, deleted_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(store_id, tax_id) DO UPDATE SET
                name = excluded.name,
                rate_basis_points = excluded.rate_basis_points,
                sync_version = tax_rates.sync_version + 1,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at
            "#,
        )
        .bind(tax.id.to_string())
        .bind(tax.meta.store_id.to_string())
        .bind(i64::from(tax.tax_id))
        .bind(&tax.name)
        .bind(i64::from(tax.rate_basis_points))
        .bind(tax.meta.sync_version)
        .bind(tax.meta.updated_at.to_rfc3339())
        .bind(tax.meta.deleted_at.map(|d| d.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_tax_rates(&self, store_id: Uuid) -> Result<Vec<TaxRate>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, store_id, tax_id, name, rate_basis_points, sync_version, updated_at, deleted_at FROM tax_rates WHERE store_id = ? AND deleted_at IS NULL ORDER BY tax_id ASC",
        )
        .bind(store_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(TaxRate {
                    id: Uuid::parse_str(&row.try_get::<String, _>("id")?)?,
                    tax_id: row.try_get::<i64, _>("tax_id")? as u8,
                    name: row.try_get("name")?,
                    rate_basis_points: row.try_get::<i64, _>("rate_basis_points")? as u32,
                    meta: parse_meta(&row)?,
                })
            })
            .collect()
    }

    pub async fn upsert_stored_text(&self, text: &StoredText) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO stored_texts (id, store_id, text_id, title, content, sync_version, updated_at, deleted_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(store_id, text_id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                sync_version = stored_texts.sync_version + 1,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at
            "#,
        )
        .bind(text.id.to_string())
        .bind(text.meta.store_id.to_string())
        .bind(i64::from(text.text_id))
        .bind(&text.title)
        .bind(&text.content)
        .bind(text.meta.sync_version)
        .bind(text.meta.updated_at.to_rfc3339())
        .bind(text.meta.deleted_at.map(|d| d.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_stored_texts(&self, store_id: Uuid) -> Result<Vec<StoredText>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, store_id, text_id, title, content, sync_version, updated_at, deleted_at FROM stored_texts WHERE store_id = ? AND deleted_at IS NULL ORDER BY text_id ASC",
        )
        .bind(store_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(StoredText {
                    id: Uuid::parse_str(&row.try_get::<String, _>("id")?)?,
                    text_id: row.try_get::<i64, _>("text_id")? as u32,
                    title: row.try_get("title")?,
                    content: row.try_get("content")?,
                    meta: parse_meta(&row)?,
                })
            })
            .collect()
    }

    pub async fn upsert_barcode_format(&self, fmt: &BarcodeFormat) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO barcode_formats (id, store_id, bid, name, definition_json, sync_version, updated_at, deleted_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(store_id, bid) DO UPDATE SET
                name = excluded.name,
                definition_json = excluded.definition_json,
                sync_version = barcode_formats.sync_version + 1,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at
            "#,
        )
        .bind(fmt.id.to_string())
        .bind(fmt.meta.store_id.to_string())
        .bind(i64::from(fmt.bid))
        .bind(&fmt.name)
        .bind(&fmt.definition_json)
        .bind(fmt.meta.sync_version)
        .bind(fmt.meta.updated_at.to_rfc3339())
        .bind(fmt.meta.deleted_at.map(|d| d.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_barcode_formats(
        &self,
        store_id: Uuid,
    ) -> Result<Vec<BarcodeFormat>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, store_id, bid, name, definition_json, sync_version, updated_at, deleted_at FROM barcode_formats WHERE store_id = ? AND deleted_at IS NULL ORDER BY bid ASC",
        )
        .bind(store_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(BarcodeFormat {
                    id: Uuid::parse_str(&row.try_get::<String, _>("id")?)?,
                    bid: row.try_get::<i64, _>("bid")? as u16,
                    name: row.try_get("name")?,
                    definition_json: row.try_get("definition_json")?,
                    meta: parse_meta(&row)?,
                })
            })
            .collect()
    }

    pub async fn upsert_label_format(&self, fmt: &LabelFormat) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO label_formats (id, store_id, pfid, name, width_tenths_mm, height_tenths_mm, definition_json, raw_binary, sync_version, updated_at, deleted_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(store_id, pfid) DO UPDATE SET
                name = excluded.name,
                width_tenths_mm = excluded.width_tenths_mm,
                height_tenths_mm = excluded.height_tenths_mm,
                definition_json = excluded.definition_json,
                raw_binary = excluded.raw_binary,
                sync_version = label_formats.sync_version + 1,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at
            "#,
        )
        .bind(fmt.id.to_string())
        .bind(fmt.meta.store_id.to_string())
        .bind(i64::from(fmt.pfid))
        .bind(&fmt.name)
        .bind(i64::from(fmt.width_tenths_mm))
        .bind(i64::from(fmt.height_tenths_mm))
        .bind(&fmt.definition_json)
        .bind(fmt.raw_binary.as_deref())
        .bind(fmt.meta.sync_version)
        .bind(fmt.meta.updated_at.to_rfc3339())
        .bind(fmt.meta.deleted_at.map(|d| d.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_label_formats(
        &self,
        store_id: Uuid,
    ) -> Result<Vec<LabelFormat>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, store_id, pfid, name, width_tenths_mm, height_tenths_mm, definition_json, raw_binary, sync_version, updated_at, deleted_at FROM label_formats WHERE store_id = ? AND deleted_at IS NULL ORDER BY pfid ASC",
        )
        .bind(store_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(LabelFormat {
                    id: Uuid::parse_str(&row.try_get::<String, _>("id")?)?,
                    pfid: row.try_get::<i64, _>("pfid")? as u16,
                    name: row.try_get("name")?,
                    width_tenths_mm: row.try_get::<i64, _>("width_tenths_mm")? as u16,
                    height_tenths_mm: row.try_get::<i64, _>("height_tenths_mm")? as u16,
                    definition_json: row.try_get("definition_json")?,
                    raw_binary: row.try_get("raw_binary")?,
                    meta: parse_meta(&row)?,
                })
            })
            .collect()
    }
}

fn parse_meta(row: &sqlx::sqlite::SqliteRow) -> Result<SyncMetadata, StorageError> {
    let store_id: String = row.try_get("store_id")?;
    let updated_at: String = row.try_get("updated_at")?;
    let deleted_at: Option<String> = row.try_get("deleted_at")?;
    Ok(SyncMetadata {
        store_id: Uuid::parse_str(&store_id)?,
        sync_version: row.try_get("sync_version")?,
        updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
        deleted_at: deleted_at
            .as_deref()
            .map(|s| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DEFAULT_LOCAL_STORE_ID, Database};
    use openscale_domain::types::new_id;

    #[tokio::test]
    async fn master_tables_crud_roundtrip() {
        let db = Database::connect_in_memory().await.unwrap();
        let repo = MasterTablesRepository::new(db.pool().clone());

        repo.upsert_department(&Department {
            id: new_id(),
            did: 1,
            name: "CARNICERÍA".to_string(),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        })
        .await
        .unwrap();
        assert_eq!(
            repo.list_departments(DEFAULT_LOCAL_STORE_ID)
                .await
                .unwrap()
                .len(),
            1
        );

        repo.upsert_tare(&Tare {
            id: new_id(),
            cid: 1,
            name: "Charola chica".to_string(),
            weight_grams: 15,
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        })
        .await
        .unwrap();
        assert_eq!(
            repo.list_tares(DEFAULT_LOCAL_STORE_ID).await.unwrap().len(),
            1
        );

        repo.upsert_tax_rate(&TaxRate {
            id: new_id(),
            tax_id: 1,
            name: "IVA 16%".to_string(),
            rate_basis_points: 1600,
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        })
        .await
        .unwrap();
        assert_eq!(
            repo.list_tax_rates(DEFAULT_LOCAL_STORE_ID)
                .await
                .unwrap()
                .len(),
            1
        );

        repo.upsert_stored_text(&StoredText {
            id: new_id(),
            text_id: 1,
            title: "Encabezado".to_string(),
            content: "CARNICERIA LA BLANQUITA".to_string(),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        })
        .await
        .unwrap();
        assert_eq!(
            repo.list_stored_texts(DEFAULT_LOCAL_STORE_ID)
                .await
                .unwrap()
                .len(),
            1
        );

        repo.upsert_barcode_format(&BarcodeFormat {
            id: new_id(),
            bid: 1,
            name: "EAN-13".to_string(),
            definition_json: r#"{"bmc":0,"bt":3,"bpd":1,"subtype":3,"template":"2NNAAAAWWWWWC"}"#
                .to_string(),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        })
        .await
        .unwrap();
        assert_eq!(
            repo.list_barcode_formats(DEFAULT_LOCAL_STORE_ID)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
