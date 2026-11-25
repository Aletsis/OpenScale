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
    #[serde(default)]
    pub deleted_plus: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeltaSyncPreview {
    pub scale_id: Uuid,
    pub new_plus: Vec<Plu>,
    pub modified_plus: Vec<Plu>,
    pub deleted_keys: Vec<PluKey>,
    pub unchanged_count: usize,
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

    /// Previews which PLUs are new, modified, or removed for a specific scale before sending.
    pub async fn preview_delta_sync(
        &self,
        store_id: Uuid,
        scale_id: Uuid,
    ) -> Result<DeltaSyncPreview, AppError> {
        let catalog = self.effective_catalog_for_scale(store_id, scale_id).await?;
        let existing_checksums = self.load_checksums(scale_id).await?;

        let mut new_plus = Vec::new();
        let mut modified_plus = Vec::new();
        let mut unchanged_count = 0usize;
        let mut active_keys = HashSet::with_capacity(catalog.len());

        for plu in catalog {
            active_keys.insert(plu.key);
            match existing_checksums.get(&plu.key) {
                None => new_plus.push(plu),
                Some(hash) if hash != &plu.delta_hash => modified_plus.push(plu),
                Some(_) => unchanged_count += 1,
            }
        }

        let mut deleted_keys: Vec<PluKey> = existing_checksums
            .keys()
            .copied()
            .filter(|k| !active_keys.contains(k))
            .collect();
        deleted_keys.sort();

        Ok(DeltaSyncPreview {
            scale_id,
            new_plus,
            modified_plus,
            deleted_keys,
            unchanged_count,
        })
    }

    /// Synchronizes the local SQLite Master Catalog to a target scale using Delta or Full mode.
    pub async fn sync_to_scale(
        &self,
        store_id: Uuid,
        scale_id: Uuid,
        mode: SyncMode,
    ) -> Result<SyncReport, AppError> {
        self.sync_to_scale_with_options(store_id, scale_id, mode, false)
            .await
    }

    /// Synchronizes only new/modified PLUs (Delta) or all PLUs (Full), optionally propagating
    /// removals (`include_deletes`) for PLUs deleted from the catalog or removed from the branch.
    pub async fn sync_to_scale_with_options(
        &self,
        store_id: Uuid,
        scale_id: Uuid,
        mode: SyncMode,
        include_deletes: bool,
    ) -> Result<SyncReport, AppError> {
        let handle = self.scales.get(scale_id).await?;
        let catalog = self.effective_catalog_for_scale(store_id, scale_id).await?;
        let existing_checksums = self.load_checksums(scale_id).await?;

        let mut sent_plus = 0usize;
        let mut skipped_unchanged = 0usize;
        let mut failed_plus = 0usize;
        let mut deleted_plus = 0usize;
        let now = Utc::now().to_rfc3339();
        let active_keys: HashSet<PluKey> = catalog.iter().map(|p| p.key).collect();

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
                Err(
                    first_err @ AppError::Driver(
                        openscale_driver::DriverError::NotConnected
                        | openscale_driver::DriverError::Io(_)
                        | openscale_driver::DriverError::Timeout,
                    ),
                ) => {
                    tracing::warn!(
                        scale_id = %scale_id,
                        did = plu.key.did,
                        plu = plu.key.plu,
                        error = %first_err,
                        "Transient driver error during PLU sync; reconnecting and retrying"
                    );
                    match handle.connect().await {
                        Ok(_) => match handle.upsert_plu(plu.clone()).await {
                            Ok(()) => {
                                self.record_checksum(scale_id, plu.key, &plu.delta_hash, &now)
                                    .await?;
                                sent_plus += 1;
                            }
                            Err(retry_err) => {
                                tracing::error!(
                                    scale_id = %scale_id,
                                    did = plu.key.did,
                                    plu = plu.key.plu,
                                    error = %retry_err,
                                    error_debug = ?retry_err,
                                    "PLU sync failed after reconnect retry"
                                );
                                failed_plus += 1;
                            }
                        },
                        Err(connect_err) => {
                            tracing::error!(
                                scale_id = %scale_id,
                                did = plu.key.did,
                                plu = plu.key.plu,
                                error = %connect_err,
                                error_debug = ?connect_err,
                                "Scale reconnect failed during PLU sync"
                            );
                            failed_plus += 1;
                        }
                    }
                }
                Err(err) => {
                    tracing::error!(
                        scale_id = %scale_id,
                        did = plu.key.did,
                        plu = plu.key.plu,
                        error = %err,
                        error_debug = ?err,
                        "PLU sync failed"
                    );
                    failed_plus += 1;
                }
            }
        }

        if include_deletes {
            for key in existing_checksums
                .keys()
                .copied()
                .filter(|k| !active_keys.contains(k))
            {
                let ok = match handle.delete_plu(key).await {
                    Ok(()) => true,
                    Err(
                        first_err @ AppError::Driver(
                            openscale_driver::DriverError::NotConnected
                            | openscale_driver::DriverError::Io(_)
                            | openscale_driver::DriverError::Timeout,
                        ),
                    ) => {
                        tracing::warn!(
                            scale_id = %scale_id,
                            did = key.did,
                            plu = key.plu,
                            error = %first_err,
                            "Transient driver error during PLU delete; reconnecting and retrying"
                        );
                        match handle.connect().await {
                            Ok(_) => match handle.delete_plu(key).await {
                                Ok(()) => true,
                                Err(retry_err) => {
                                    tracing::error!(
                                        scale_id = %scale_id,
                                        did = key.did,
                                        plu = key.plu,
                                        error = %retry_err,
                                        error_debug = ?retry_err,
                                        "PLU delete failed after reconnect retry"
                                    );
                                    false
                                }
                            },
                            Err(connect_err) => {
                                tracing::error!(
                                    scale_id = %scale_id,
                                    did = key.did,
                                    plu = key.plu,
                                    error = %connect_err,
                                    error_debug = ?connect_err,
                                    "Scale reconnect failed during PLU delete"
                                );
                                false
                            }
                        }
                    }
                    Err(err) => {
                        tracing::error!(
                            scale_id = %scale_id,
                            did = key.did,
                            plu = key.plu,
                            error = %err,
                            error_debug = ?err,
                            "PLU delete failed"
                        );
                        false
                    }
                };
                if ok {
                    let _ = self.delete_checksum(scale_id, key).await;
                    deleted_plus += 1;
                } else {
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
            deleted_plus,
        })
    }

    pub async fn record_plu_checksum_if_synced(
        &self,
        scale_id: Uuid,
        key: PluKey,
        previous_hash: &str,
        new_hash: &str,
    ) -> Result<(), AppError> {
        let existing = self.load_checksums(scale_id).await?;
        if existing.get(&key).is_some_and(|h| h == previous_hash) {
            let now = Utc::now().to_rfc3339();
            self.record_checksum(scale_id, key, new_hash, &now).await?;
        }
        Ok(())
    }

    async fn delete_checksum(&self, scale_id: Uuid, key: PluKey) -> Result<(), AppError> {
        sqlx::query(
            "DELETE FROM scale_plu_checksums WHERE scale_id = ? AND did = ? AND plu_number = ?",
        )
        .bind(scale_id.to_string())
        .bind(i64::from(key.did))
        .bind(key.plu as i64)
        .execute(self.db.pool())
        .await?;
        Ok(())
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
