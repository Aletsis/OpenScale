use chrono::{DateTime, Utc};
use openscale_domain::{Scale, ScaleFamily, ScaleStatus, SyncMetadata};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::StorageError;

#[derive(Debug, Clone)]
pub struct ScaleRepository {
    pool: SqlitePool,
}

impl ScaleRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, scale: &Scale) -> Result<(), StorageError> {
        let family = family_to_str(scale.family);
        let status = status_to_str(scale.status);
        let last_seen = scale.last_seen_at.map(|dt| dt.to_rfc3339());
        let updated_at = scale.meta.updated_at.to_rfc3339();
        let deleted_at = scale.meta.deleted_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO scales (
                id, store_id, group_id, name, family, ip_address, data_port, service_port,
                serial_number, model, firmware_version, mac_address, status, last_seen_at,
                sync_version, updated_at, deleted_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                group_id = excluded.group_id,
                name = excluded.name,
                family = excluded.family,
                ip_address = excluded.ip_address,
                data_port = excluded.data_port,
                service_port = excluded.service_port,
                serial_number = excluded.serial_number,
                model = excluded.model,
                firmware_version = excluded.firmware_version,
                mac_address = excluded.mac_address,
                status = excluded.status,
                last_seen_at = excluded.last_seen_at,
                sync_version = scales.sync_version + 1,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at
            "#,
        )
        .bind(scale.id.to_string())
        .bind(scale.meta.store_id.to_string())
        .bind(scale.group_id.map(|id| id.to_string()))
        .bind(&scale.name)
        .bind(family)
        .bind(&scale.ip_address)
        .bind(i64::from(scale.data_port))
        .bind(scale.service_port.map(i64::from))
        .bind(&scale.serial_number)
        .bind(&scale.model)
        .bind(&scale.firmware_version)
        .bind(&scale.mac_address)
        .bind(status)
        .bind(last_seen)
        .bind(scale.meta.sync_version)
        .bind(updated_at)
        .bind(deleted_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_active(&self, store_id: Uuid) -> Result<Vec<Scale>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, store_id, group_id, name, family, ip_address, data_port, service_port,
                serial_number, model, firmware_version, mac_address, status, last_seen_at,
                sync_version, updated_at, deleted_at
            FROM scales
            WHERE store_id = ? AND deleted_at IS NULL
            ORDER BY name ASC
            "#,
        )
        .bind(store_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_scale).collect()
    }
}

fn family_to_str(family: ScaleFamily) -> &'static str {
    match family {
        ScaleFamily::AveryBerkelXs => "avery_berkel_xs",
        ScaleFamily::AveryBerkelXti => "avery_berkel_xti",
    }
}

fn str_to_family(value: &str) -> Result<ScaleFamily, StorageError> {
    match value {
        "avery_berkel_xs" => Ok(ScaleFamily::AveryBerkelXs),
        "avery_berkel_xti" => Ok(ScaleFamily::AveryBerkelXti),
        other => Err(StorageError::InvalidEnumValue {
            column: "family",
            value: other.to_string(),
        }),
    }
}

fn status_to_str(status: ScaleStatus) -> &'static str {
    match status {
        ScaleStatus::Unknown => "unknown",
        ScaleStatus::Online => "online",
        ScaleStatus::Offline => "offline",
        ScaleStatus::Syncing => "syncing",
        ScaleStatus::Error => "error",
    }
}

fn str_to_status(value: &str) -> Result<ScaleStatus, StorageError> {
    match value {
        "unknown" => Ok(ScaleStatus::Unknown),
        "online" => Ok(ScaleStatus::Online),
        "offline" => Ok(ScaleStatus::Offline),
        "syncing" => Ok(ScaleStatus::Syncing),
        "error" => Ok(ScaleStatus::Error),
        other => Err(StorageError::InvalidEnumValue {
            column: "status",
            value: other.to_string(),
        }),
    }
}

fn row_to_scale(row: sqlx::sqlite::SqliteRow) -> Result<Scale, StorageError> {
    let id_str: String = row.try_get("id")?;
    let store_id_str: String = row.try_get("store_id")?;
    let group_id_str: Option<String> = row.try_get("group_id")?;
    let family_str: String = row.try_get("family")?;
    let status_str: String = row.try_get("status")?;
    let last_seen_str: Option<String> = row.try_get("last_seen_at")?;
    let updated_at_str: String = row.try_get("updated_at")?;
    let deleted_at_str: Option<String> = row.try_get("deleted_at")?;
    let data_port: i64 = row.try_get("data_port")?;
    let service_port: Option<i64> = row.try_get("service_port")?;

    Ok(Scale {
        id: Uuid::parse_str(&id_str)?,
        group_id: group_id_str.as_deref().map(Uuid::parse_str).transpose()?,
        name: row.try_get("name")?,
        family: str_to_family(&family_str)?,
        ip_address: row.try_get("ip_address")?,
        data_port: data_port as u16,
        service_port: service_port.map(|p| p as u16),
        serial_number: row.try_get("serial_number")?,
        model: row.try_get("model")?,
        firmware_version: row.try_get("firmware_version")?,
        mac_address: row.try_get("mac_address")?,
        status: str_to_status(&status_str)?,
        last_seen_at: last_seen_str
            .as_deref()
            .map(|s| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
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
    async fn scale_upsert_and_list_roundtrip() {
        let db = Database::connect_in_memory().await.unwrap();
        let repo = ScaleRepository::new(db.pool().clone());

        let scale = Scale {
            id: new_id(),
            group_id: None,
            name: "Carnicería 01".to_string(),
            family: ScaleFamily::AveryBerkelXs,
            ip_address: "192.168.9.113".to_string(),
            data_port: 3001,
            service_port: Some(28822),
            serial_number: Some("22320310".to_string()),
            model: Some("XS200".to_string()),
            firmware_version: Some("APP. v4.1.7.0".to_string()),
            mac_address: None,
            status: ScaleStatus::Online,
            last_seen_at: Some(Utc::now()),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        };

        repo.upsert(&scale).await.unwrap();
        let list = repo.list_active(DEFAULT_LOCAL_STORE_ID).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].ip_address, "192.168.9.113");
        assert_eq!(list[0].family, ScaleFamily::AveryBerkelXs);
    }
}
