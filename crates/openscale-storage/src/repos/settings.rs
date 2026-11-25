use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::StorageError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleConfigProfile {
    pub id: Uuid,
    pub store_id: Uuid,
    /// `"global"` | `"branch"` | `"scale"`
    pub scope: String,
    /// `"global"` for global profile, or UUID string for branch/scale
    pub target_id: String,
    /// `"avery_berkel_xs"` | `"avery_berkel_xti"`
    pub family: String,
    pub machine_params_json: Option<String>,
    pub banks_748_json: Option<String>,
    /// JSON array of field keys explicitly overridden at this scope (e.g. `["print_mode", "banks_748"]`)
    pub overrides_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct SettingsRepository {
    pool: SqlitePool,
}

impl SettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_app_setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        let row = sqlx::query("SELECT value_json FROM app_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<String, _>("value_json")))
    }

    pub async fn set_app_setting(&self, key: &str, value_json: &str) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value_json, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(key) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(key)
        .bind(value_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_profile(
        &self,
        store_id: Uuid,
        scope: &str,
        target_id: &str,
        family: &str,
    ) -> Result<Option<ScaleConfigProfile>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, store_id, scope, target_id, family,
                machine_params_json, banks_748_json, overrides_json, updated_at
            FROM scale_config_profiles
            WHERE store_id = ? AND scope = ? AND target_id = ? AND family = ?
            "#,
        )
        .bind(store_id.to_string())
        .bind(scope)
        .bind(target_id)
        .bind(family)
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_profile).transpose()
    }

    pub async fn upsert_profile(&self, profile: &ScaleConfigProfile) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO scale_config_profiles (
                id, store_id, scope, target_id, family,
                machine_params_json, banks_748_json, overrides_json, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(store_id, scope, target_id, family) DO UPDATE SET
                machine_params_json = excluded.machine_params_json,
                banks_748_json = excluded.banks_748_json,
                overrides_json = excluded.overrides_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(profile.id.to_string())
        .bind(profile.store_id.to_string())
        .bind(&profile.scope)
        .bind(&profile.target_id)
        .bind(&profile.family)
        .bind(&profile.machine_params_json)
        .bind(&profile.banks_748_json)
        .bind(&profile.overrides_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn row_to_profile(row: sqlx::sqlite::SqliteRow) -> Result<ScaleConfigProfile, StorageError> {
    let id_str: String = row.try_get("id")?;
    let store_id_str: String = row.try_get("store_id")?;
    Ok(ScaleConfigProfile {
        id: Uuid::parse_str(&id_str)?,
        store_id: Uuid::parse_str(&store_id_str)?,
        scope: row.try_get("scope")?,
        target_id: row.try_get("target_id")?,
        family: row.try_get("family")?,
        machine_params_json: row.try_get("machine_params_json")?,
        banks_748_json: row.try_get("banks_748_json")?,
        overrides_json: row.try_get("overrides_json")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DEFAULT_LOCAL_STORE_ID, Database};
    use openscale_domain::types::new_id;

    #[tokio::test]
    async fn settings_and_scale_profiles_roundtrip() {
        let db = Database::connect_in_memory().await.unwrap();
        let repo = SettingsRepository::new(db.pool().clone());

        repo.set_app_setting("general", r#"{"health_interval_secs":15}"#)
            .await
            .unwrap();
        assert_eq!(
            repo.get_app_setting("general").await.unwrap().as_deref(),
            Some(r#"{"health_interval_secs":15}"#)
        );

        let profile = ScaleConfigProfile {
            id: new_id(),
            store_id: DEFAULT_LOCAL_STORE_ID,
            scope: "global".to_string(),
            target_id: "global".to_string(),
            family: "avery_berkel_xs".to_string(),
            machine_params_json: Some(r#"{"print_mode":2}"#.to_string()),
            banks_748_json: None,
            overrides_json: r#"["print_mode"]"#.to_string(),
            updated_at: Utc::now().to_rfc3339(),
        };
        repo.upsert_profile(&profile).await.unwrap();

        let loaded = repo
            .get_profile(
                DEFAULT_LOCAL_STORE_ID,
                "global",
                "global",
                "avery_berkel_xs",
            )
            .await
            .unwrap()
            .expect("profile should exist");
        assert_eq!(
            loaded.machine_params_json.as_deref(),
            Some(r#"{"print_mode":2}"#)
        );
    }
}
