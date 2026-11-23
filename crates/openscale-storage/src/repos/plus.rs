use chrono::{DateTime, Utc};
use openscale_domain::{MoneyCents, Plu, PluKey, ProductType, SyncMetadata};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::StorageError;

#[derive(Debug, Clone)]
pub struct PluRepository {
    pool: SqlitePool,
}

impl PluRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, plu: &Plu) -> Result<(), StorageError> {
        let delta_hash = if plu.delta_hash.is_empty() {
            plu.compute_delta_hash()
        } else {
            plu.delta_hash.clone()
        };
        let text_areas_json = serde_json::to_string(&plu.text_areas)?;
        let updated_at = plu.meta.updated_at.to_rfc3339();
        let deleted_at = plu.meta.deleted_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO plus (
                id, store_id, did, plu_number, description, button_text,
                price_1_cents, price_2_cents, price_3_cents, product_type,
                ean, sku, group_id, barcode_format_id, label_format_id,
                tare_id, tare_grams, date_offset_1, date_print_control_1,
                date_offset_2, date_print_control_2, tax_rate_id_1, tax_rate_id_2,
                text_areas_json, delta_hash, sync_version, updated_at, deleted_at
            ) VALUES (
                ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?, ?
            )
            ON CONFLICT(store_id, did, plu_number) DO UPDATE SET
                description = excluded.description,
                button_text = excluded.button_text,
                price_1_cents = excluded.price_1_cents,
                price_2_cents = excluded.price_2_cents,
                price_3_cents = excluded.price_3_cents,
                product_type = excluded.product_type,
                ean = excluded.ean,
                sku = excluded.sku,
                group_id = excluded.group_id,
                barcode_format_id = excluded.barcode_format_id,
                label_format_id = excluded.label_format_id,
                tare_id = excluded.tare_id,
                tare_grams = excluded.tare_grams,
                date_offset_1 = excluded.date_offset_1,
                date_print_control_1 = excluded.date_print_control_1,
                date_offset_2 = excluded.date_offset_2,
                date_print_control_2 = excluded.date_print_control_2,
                tax_rate_id_1 = excluded.tax_rate_id_1,
                tax_rate_id_2 = excluded.tax_rate_id_2,
                text_areas_json = excluded.text_areas_json,
                delta_hash = excluded.delta_hash,
                sync_version = plus.sync_version + 1,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at
            "#,
        )
        .bind(plu.id.to_string())
        .bind(plu.meta.store_id.to_string())
        .bind(i64::from(plu.key.did))
        .bind(plu.key.plu as i64)
        .bind(&plu.description)
        .bind(&plu.button_text)
        .bind(plu.price_1.0)
        .bind(plu.price_2.0)
        .bind(plu.price_3.0)
        .bind(product_type_to_str(plu.product_type))
        .bind(&plu.ean)
        .bind(&plu.sku)
        .bind(i64::from(plu.group_id))
        .bind(i64::from(plu.barcode_format_id))
        .bind(i64::from(plu.label_format_id))
        .bind(plu.tare_id.map(i64::from))
        .bind(plu.tare_grams.map(i64::from))
        .bind(i64::from(plu.date_offset_1))
        .bind(i64::from(plu.date_print_control_1))
        .bind(i64::from(plu.date_offset_2))
        .bind(i64::from(plu.date_print_control_2))
        .bind(i64::from(plu.tax_rate_id_1))
        .bind(i64::from(plu.tax_rate_id_2))
        .bind(text_areas_json)
        .bind(delta_hash)
        .bind(plu.meta.sync_version)
        .bind(updated_at)
        .bind(deleted_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_by_key(
        &self,
        store_id: Uuid,
        key: PluKey,
    ) -> Result<Option<Plu>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, store_id, did, plu_number, description, button_text,
                price_1_cents, price_2_cents, price_3_cents, product_type,
                ean, sku, group_id, barcode_format_id, label_format_id,
                tare_id, tare_grams, date_offset_1, date_print_control_1,
                date_offset_2, date_print_control_2, tax_rate_id_1, tax_rate_id_2,
                text_areas_json, delta_hash, sync_version, updated_at, deleted_at
            FROM plus
            WHERE store_id = ? AND did = ? AND plu_number = ? AND deleted_at IS NULL
            "#,
        )
        .bind(store_id.to_string())
        .bind(i64::from(key.did))
        .bind(key.plu as i64)
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_plu).transpose()
    }

    pub async fn list_active(&self, store_id: Uuid) -> Result<Vec<Plu>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, store_id, did, plu_number, description, button_text,
                price_1_cents, price_2_cents, price_3_cents, product_type,
                ean, sku, group_id, barcode_format_id, label_format_id,
                tare_id, tare_grams, date_offset_1, date_print_control_1,
                date_offset_2, date_print_control_2, tax_rate_id_1, tax_rate_id_2,
                text_areas_json, delta_hash, sync_version, updated_at, deleted_at
            FROM plus
            WHERE store_id = ? AND deleted_at IS NULL
            ORDER BY did ASC, plu_number ASC
            "#,
        )
        .bind(store_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_plu).collect()
    }

    pub async fn soft_delete(&self, store_id: Uuid, key: PluKey) -> Result<bool, StorageError> {
        let now = Utc::now().to_rfc3339();
        let res = sqlx::query(
            r#"
            UPDATE plus
            SET deleted_at = ?, updated_at = ?, sync_version = sync_version + 1
            WHERE store_id = ? AND did = ? AND plu_number = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(store_id.to_string())
        .bind(i64::from(key.did))
        .bind(key.plu as i64)
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected() > 0)
    }
}

fn product_type_to_str(pt: ProductType) -> &'static str {
    match pt {
        ProductType::ByWeight => "by_weight",
        ProductType::ByCount => "by_count",
        ProductType::NegativeByCount => "negative_by_count",
        ProductType::FixedWeight => "fixed_weight",
        ProductType::FixedPrice => "fixed_price",
        ProductType::ManualWeight => "manual_weight",
        ProductType::ByVolume => "by_volume",
    }
}

fn str_to_product_type(value: &str) -> Result<ProductType, StorageError> {
    match value {
        "by_weight" => Ok(ProductType::ByWeight),
        "by_count" => Ok(ProductType::ByCount),
        "negative_by_count" => Ok(ProductType::NegativeByCount),
        "fixed_weight" => Ok(ProductType::FixedWeight),
        "fixed_price" => Ok(ProductType::FixedPrice),
        "manual_weight" => Ok(ProductType::ManualWeight),
        "by_volume" => Ok(ProductType::ByVolume),
        other => Err(StorageError::InvalidEnumValue {
            column: "product_type",
            value: other.to_string(),
        }),
    }
}

fn row_to_plu(row: sqlx::sqlite::SqliteRow) -> Result<Plu, StorageError> {
    let id_str: String = row.try_get("id")?;
    let store_id_str: String = row.try_get("store_id")?;
    let did: i64 = row.try_get("did")?;
    let plu_number: i64 = row.try_get("plu_number")?;
    let pt_str: String = row.try_get("product_type")?;
    let text_areas_json: String = row.try_get("text_areas_json")?;
    let updated_at_str: String = row.try_get("updated_at")?;
    let deleted_at_str: Option<String> = row.try_get("deleted_at")?;

    Ok(Plu {
        id: Uuid::parse_str(&id_str)?,
        key: PluKey::new(did as u32, plu_number as u64)?,
        description: row.try_get("description")?,
        button_text: row.try_get("button_text")?,
        price_1: MoneyCents(row.try_get("price_1_cents")?),
        price_2: MoneyCents(row.try_get("price_2_cents")?),
        price_3: MoneyCents(row.try_get("price_3_cents")?),
        product_type: str_to_product_type(&pt_str)?,
        ean: row.try_get("ean")?,
        sku: row.try_get("sku")?,
        group_id: row.try_get::<i64, _>("group_id")? as u32,
        barcode_format_id: row.try_get::<i64, _>("barcode_format_id")? as u16,
        label_format_id: row.try_get::<i64, _>("label_format_id")? as u16,
        tare_id: row.try_get::<Option<i64>, _>("tare_id")?.map(|v| v as u16),
        tare_grams: row
            .try_get::<Option<i64>, _>("tare_grams")?
            .map(|v| v as u32),
        date_offset_1: row.try_get::<i64, _>("date_offset_1")? as u16,
        date_print_control_1: row.try_get::<i64, _>("date_print_control_1")? as u8,
        date_offset_2: row.try_get::<i64, _>("date_offset_2")? as u16,
        date_print_control_2: row.try_get::<i64, _>("date_print_control_2")? as u8,
        tax_rate_id_1: row.try_get::<i64, _>("tax_rate_id_1")? as u8,
        tax_rate_id_2: row.try_get::<i64, _>("tax_rate_id_2")? as u8,
        text_areas: serde_json::from_str(&text_areas_json)?,
        delta_hash: row.try_get("delta_hash")?,
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
    use crate::{DEFAULT_LOCAL_STORE_ID, Database};
    use openscale_domain::types::new_id;

    #[tokio::test]
    async fn plu_crud_and_soft_delete() {
        let db = Database::connect_in_memory().await.unwrap();
        let repo = PluRepository::new(db.pool().clone());
        let key = PluKey::new(1, 500).unwrap();

        let plu = Plu {
            id: new_id(),
            key,
            description: "BISTEC DE RES".to_string(),
            button_text: Some("Bistec".to_string()),
            price_1: MoneyCents(18900),
            price_2: MoneyCents(0),
            price_3: MoneyCents(0),
            product_type: ProductType::ByWeight,
            ean: Some("2000500000000".to_string()),
            sku: None,
            group_id: 1,
            barcode_format_id: 1,
            label_format_id: 2,
            tare_id: Some(1),
            tare_grams: Some(15),
            date_offset_1: 5,
            date_print_control_1: 1,
            date_offset_2: 0,
            date_print_control_2: 0,
            tax_rate_id_1: 0,
            tax_rate_id_2: 0,
            text_areas: vec!["Corte fresco".to_string()],
            delta_hash: String::new(),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        };

        repo.upsert(&plu).await.unwrap();
        let fetched = repo
            .get_by_key(DEFAULT_LOCAL_STORE_ID, key)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.description, "BISTEC DE RES");
        assert_eq!(fetched.price_1, MoneyCents(18900));
        assert_eq!(fetched.delta_hash.len(), 64);

        let deleted = repo.soft_delete(DEFAULT_LOCAL_STORE_ID, key).await.unwrap();
        assert!(deleted);
        assert!(
            repo.get_by_key(DEFAULT_LOCAL_STORE_ID, key)
                .await
                .unwrap()
                .is_none()
        );
    }
}
