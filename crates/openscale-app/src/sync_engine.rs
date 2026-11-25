use std::collections::{HashMap, HashSet};

use chrono::Utc;
use openscale_domain::{BranchCatalogMode, Plu, PluKey};
use openscale_storage::Database;
use openscale_storage::repos::{BranchRepository, PluRepository, ScaleRepository};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::error::AppError;
use crate::scale_manager::ScaleManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    Delta,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncReport {
    pub scale_id: Uuid,
    pub mode: SyncMode,
    pub total_catalog_plus: usize,
    pub sent_plus: usize,
    pub skipped_unchanged: usize,
    pub failed_plus: usize,
}

/// Orchestrates catalog imports from physical scales and delta/full synchronization to scales.
#[derive(Clone)]
pub struct SyncEngine {
    db: Database,
    scales: ScaleManager,
}

impl SyncEngine {
    pub fn new(db: Database, scales: ScaleManager) -> Self {
        Self { db, scales }
    }

    /// Reads all PLUs from a physical scale and imports them into the local SQLite Master Catalog.
    pub async fn import_from_scale(&self, scale_id: Uuid) -> Result<usize, AppError> {
        let handle = self.scales.get(scale_id).await?;
        let remote_plus = match handle.list_plus(None, None).await {
            Ok(list) => list,
            Err(AppError::Driver(
                openscale_driver::DriverError::NotConnected
                | openscale_driver::DriverError::Io(_)
                | openscale_driver::DriverError::Timeout,
            )) => {
                handle.connect().await?;
                handle.list_plus(None, None).await?
            }
            Err(err) => return Err(err),
        };
        let plu_repo = PluRepository::new(self.db.pool().clone());

        let now = Utc::now().to_rfc3339();
        let mut imported = 0usize;

        for plu in &remote_plus {
            plu_repo.upsert(plu).await?;
            self.record_checksum(scale_id, plu.key, &plu.delta_hash, &now)
                .await?;
            imported += 1;
        }

        Ok(imported)
    }

    /// Resolves the effective PLU catalog for a scale, applying its branch's `catalog_mode`
    /// while keeping global Master Catalog prices identical across all branches.
    pub async fn effective_catalog_for_scale(
        &self,
        store_id: Uuid,
        scale_id: Uuid,
    ) -> Result<Vec<Plu>, AppError> {
        let plu_repo = PluRepository::new(self.db.pool().clone());
        let all_plus = plu_repo.list_active(store_id).await?;

        let scale_repo = ScaleRepository::new(self.db.pool().clone());
        let Some(scale) = scale_repo.get_by_id(store_id, scale_id).await? else {
            return Ok(all_plus);
        };
        let Some(branch_id) = scale.branch_id else {
            return Ok(all_plus);
        };

        let branch_repo = BranchRepository::new(self.db.pool().clone());
        let Some(branch) = branch_repo.get_by_id(store_id, branch_id).await? else {
            return Ok(all_plus);
        };

        if branch.catalog_mode == BranchCatalogMode::All {
            return Ok(all_plus);
        }

        let allowed: HashSet<PluKey> = branch_repo
            .list_plu_keys(branch_id)
            .await?
            .into_iter()
            .collect();
        Ok(all_plus
            .into_iter()
            .filter(|plu| allowed.contains(&plu.key))
            .collect())
    }

    /// Synchronizes the local SQLite Master Catalog to a target scale using Delta or Full mode.
    pub async fn sync_to_scale(
        &self,
        store_id: Uuid,
        scale_id: Uuid,
        mode: SyncMode,
    ) -> Result<SyncReport, AppError> {
        let handle = self.scales.get(scale_id).await?;
        let catalog = self.effective_catalog_for_scale(store_id, scale_id).await?;
        let existing_checksums = self.load_checksums(scale_id).await?;

        let mut sent_plus = 0usize;
        let mut skipped_unchanged = 0usize;
        let mut failed_plus = 0usize;
        let now = Utc::now().to_rfc3339();

        for plu in &catalog {
            let needs_send = match mode {
                SyncMode::Full => true,
                SyncMode::Delta => existing_checksums
                    .get(&plu.key)
                    .is_none_or(|hash| hash != &plu.delta_hash),
            };

            if !needs_send {
                skipped_unchanged += 1;
                continue;
            }

            match handle.upsert_plu(plu.clone()).await {
                Ok(()) => {
                    self.record_checksum(scale_id, plu.key, &plu.delta_hash, &now)
                        .await?;
                    sent_plus += 1;
                }
                Err(AppError::Driver(
                    openscale_driver::DriverError::NotConnected
                    | openscale_driver::DriverError::Io(_)
                    | openscale_driver::DriverError::Timeout,
                )) => {
                    if handle.connect().await.is_ok()
                        && handle.upsert_plu(plu.clone()).await.is_ok()
                    {
                        self.record_checksum(scale_id, plu.key, &plu.delta_hash, &now)
                            .await?;
                        sent_plus += 1;
                    } else {
                        failed_plus += 1;
                    }
                }
                Err(_) => {
                    failed_plus += 1;
                }
            }
        }

        Ok(SyncReport {
            scale_id,
            mode,
            total_catalog_plus: catalog.len(),
            sent_plus,
            skipped_unchanged,
            failed_plus,
        })
    }

    async fn load_checksums(&self, scale_id: Uuid) -> Result<HashMap<PluKey, String>, AppError> {
        let rows = sqlx::query(
            "SELECT did, plu_number, delta_hash FROM scale_plu_checksums WHERE scale_id = ?",
        )
        .bind(scale_id.to_string())
        .fetch_all(self.db.pool())
        .await?;

        let mut map = HashMap::with_capacity(rows.len());
        for row in rows {
            let did: i64 = row.try_get("did")?;
            let plu_number: i64 = row.try_get("plu_number")?;
            let delta_hash: String = row.try_get("delta_hash")?;
            if let Ok(key) = PluKey::new(did as u32, plu_number as u64) {
                map.insert(key, delta_hash);
            }
        }
        Ok(map)
    }

    async fn record_checksum(
        &self,
        scale_id: Uuid,
        key: PluKey,
        delta_hash: &str,
        synced_at: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO scale_plu_checksums (scale_id, did, plu_number, delta_hash, synced_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(scale_id, did, plu_number) DO UPDATE SET
                delta_hash = excluded.delta_hash,
                synced_at = excluded.synced_at
            "#,
        )
        .bind(scale_id.to_string())
        .bind(i64::from(key.did))
        .bind(key.plu as i64)
        .bind(delta_hash)
        .bind(synced_at)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }
}
