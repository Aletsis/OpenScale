use std::time::Duration;

use chrono::Utc;
use openscale_domain::types::new_id;
use openscale_domain::{
    AuditLog, BarcodeFormat, Branch, Department, LabelFormat, MoneyCents, Plu, PluKey,
    SalesSnapshot, Scale, ScaleSalesReport, StoredText, SyncMetadata, Tare, TaxRate,
};
use openscale_driver::DiscoveredScale;
use openscale_storage::repos::{
    AuditRepository, BranchRepository, MasterTablesRepository, PluRepository,
    SalesSnapshotRepository, ScaleConfigProfile, ScaleRepository, SettingsRepository,
};
use openscale_storage::{DEFAULT_LOCAL_STORE_ID, Database};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::discovery::discover_all_scales;
use crate::error::AppError;
use crate::health_monitor::{
    ScaleHealthCheckResult, check_all_scales_health as check_all_scales_health_impl,
    check_scale_health as check_scale_health_impl,
    start_health_monitor as start_health_monitor_impl,
};
use crate::identity::{DiscoveredScaleMatch, classify_discovered_scales};
use crate::import_engine::{
    ImportSelection, ScaleImportApplyRequest, ScaleImportPreview, ScaleImportResult,
    apply_scale_import as apply_scale_import_impl,
    preview_import_from_scale as preview_import_from_scale_impl,
};
use crate::scale_manager::ScaleManager;
use crate::sync_engine::{DeltaSyncPreview, SyncEngine, SyncMode, SyncReport};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceChangeItem {
    pub key: PluKey,
    pub price_1: MoneyCents,
    #[serde(default)]
    pub price_2: Option<MoneyCents>,
    #[serde(default)]
    pub price_3: Option<MoneyCents>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceChangeScaleResult {
    pub scale_id: Uuid,
    pub updated: usize,
    pub failed: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceChangeReport {
    pub updated_catalog: usize,
    pub scale_results: Vec<PriceChangeScaleResult>,
}

/// High-level application facade shared by Phase 1 Tauri IPC and Phase 2 Axum API.
#[derive(Clone)]
pub struct OpenScaleApp {
    pub store_id: Uuid,
    pub db: Database,
    pub scales: ScaleManager,
    pub sync: SyncEngine,
}

impl OpenScaleApp {
    pub fn new(db: Database) -> Self {
        let scales = ScaleManager::new();
        let sync = SyncEngine::new(db.clone(), scales.clone());
        Self {
            store_id: DEFAULT_LOCAL_STORE_ID,
            db,
            scales,
            sync,
        }
    }

    pub fn scale_repo(&self) -> ScaleRepository {
        ScaleRepository::new(self.db.pool().clone())
    }

    pub fn branch_repo(&self) -> BranchRepository {
        BranchRepository::new(self.db.pool().clone())
    }

    pub fn plu_repo(&self) -> PluRepository {
        PluRepository::new(self.db.pool().clone())
    }

    pub fn master_tables_repo(&self) -> MasterTablesRepository {
        MasterTablesRepository::new(self.db.pool().clone())
    }

    pub fn sales_repo(&self) -> SalesSnapshotRepository {
        SalesSnapshotRepository::new(self.db.pool().clone())
    }

    pub fn audit_repo(&self) -> AuditRepository {
        AuditRepository::new(self.db.pool().clone())
    }

    pub fn settings_repo(&self) -> SettingsRepository {
        SettingsRepository::new(self.db.pool().clone())
    }

    pub async fn get_app_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self.settings_repo().get_app_setting(key).await?)
    }

    pub async fn set_app_setting(&self, key: &str, value_json: &str) -> Result<(), AppError> {
        Ok(self
            .settings_repo()
            .set_app_setting(key, value_json)
            .await?)
    }

    pub async fn get_scale_config_profile(
        &self,
        scope: &str,
        target_id: &str,
        family: &str,
    ) -> Result<Option<ScaleConfigProfile>, AppError> {
        Ok(self
            .settings_repo()
            .get_profile(self.store_id, scope, target_id, family)
            .await?)
    }

    pub async fn save_scale_config_profile(
        &self,
        mut profile: ScaleConfigProfile,
    ) -> Result<ScaleConfigProfile, AppError> {
        profile.store_id = self.store_id;
        self.settings_repo().upsert_profile(&profile).await?;
        Ok(profile)
    }

    pub async fn discover_scales(
        &self,
        timeout: Duration,
    ) -> Result<Vec<DiscoveredScale>, AppError> {
        discover_all_scales(timeout, &[], &[]).await
    }

    pub async fn discover_scales_with_identity(
        &self,
        timeout: Duration,
    ) -> Result<Vec<DiscoveredScaleMatch>, AppError> {
        let found = discover_all_scales(timeout, &[], &[]).await?;
        let registered = self.list_scales().await?;
        Ok(classify_discovered_scales(&found, &registered))
    }

    /// Updates a scale's IP address (e.g. after DHCP change) while preserving its ID,
    /// history, and `scale_plu_checksums`, and restarts its TCP actor.
    pub async fn reassign_scale_ip(
        &self,
        scale_id: Uuid,
        new_ip: String,
    ) -> Result<Scale, AppError> {
        let mut scale = self
            .scale_repo()
            .get_by_id(self.store_id, scale_id)
            .await?
            .ok_or(AppError::ScaleNotRegistered(scale_id))?;
        scale.ip_address = new_ip.trim().to_string();
        scale.meta.updated_at = Utc::now();
        self.scale_repo().upsert(&scale).await?;
        self.scales.register_scale(&scale).await;
        Ok(scale)
    }

    /// Swaps IP addresses between two registered scales and restarts both TCP actors.
    pub async fn swap_scale_ips(
        &self,
        scale_a_id: Uuid,
        scale_b_id: Uuid,
    ) -> Result<(Scale, Scale), AppError> {
        let mut a = self
            .scale_repo()
            .get_by_id(self.store_id, scale_a_id)
            .await?
            .ok_or(AppError::ScaleNotRegistered(scale_a_id))?;
        let mut b = self
            .scale_repo()
            .get_by_id(self.store_id, scale_b_id)
            .await?
            .ok_or(AppError::ScaleNotRegistered(scale_b_id))?;

        std::mem::swap(&mut a.ip_address, &mut b.ip_address);
        let now = Utc::now();
        a.meta.updated_at = now;
        b.meta.updated_at = now;

        self.scale_repo().upsert(&a).await?;
        self.scale_repo().upsert(&b).await?;
        self.scales.register_scale(&a).await;
        self.scales.register_scale(&b).await;
        Ok((a, b))
    }

    pub async fn list_branches(&self) -> Result<Vec<Branch>, AppError> {
        Ok(self.branch_repo().list_active(self.store_id).await?)
    }

    pub async fn upsert_branch(&self, branch: Branch) -> Result<Branch, AppError> {
        self.branch_repo().upsert(&branch).await?;
        Ok(branch)
    }

    pub async fn delete_branch(&self, branch_id: Uuid) -> Result<bool, AppError> {
        Ok(self
            .branch_repo()
            .soft_delete(self.store_id, branch_id)
            .await?)
    }

    pub async fn list_branch_plu_keys(&self, branch_id: Uuid) -> Result<Vec<PluKey>, AppError> {
        Ok(self.branch_repo().list_plu_keys(branch_id).await?)
    }

    pub async fn set_branch_plu_keys(
        &self,
        branch_id: Uuid,
        keys: Vec<PluKey>,
    ) -> Result<usize, AppError> {
        Ok(self.branch_repo().set_plu_keys(branch_id, &keys).await?)
    }

    pub async fn list_scales(&self) -> Result<Vec<Scale>, AppError> {
        Ok(self.scale_repo().list_active(self.store_id).await?)
    }

    pub async fn upsert_scale(&self, scale: Scale) -> Result<Scale, AppError> {
        self.scale_repo().upsert(&scale).await?;
        self.scales.register_scale(&scale).await;
        Ok(scale)
    }

    pub async fn delete_scale(&self, scale_id: Uuid) -> Result<bool, AppError> {
        let deleted = self
            .scale_repo()
            .soft_delete(self.store_id, scale_id)
            .await?;
        if deleted {
            self.scales.unregister(scale_id).await;
        }
        Ok(deleted)
    }

    pub async fn check_scale_health(
        &self,
        scale_id: Uuid,
    ) -> Result<ScaleHealthCheckResult, AppError> {
        check_scale_health_impl(self, scale_id).await
    }

    pub async fn check_all_scales_health(&self) -> Result<Vec<ScaleHealthCheckResult>, AppError> {
        check_all_scales_health_impl(self).await
    }

    pub fn start_health_monitor(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        start_health_monitor_impl(self.clone(), interval)
    }

    pub async fn list_plus(&self) -> Result<Vec<Plu>, AppError> {
        Ok(self.plu_repo().list_active(self.store_id).await?)
    }

    pub async fn get_plu(&self, key: PluKey) -> Result<Option<Plu>, AppError> {
        Ok(self.plu_repo().get_by_key(self.store_id, key).await?)
    }

    pub async fn upsert_plu(&self, mut plu: Plu) -> Result<Plu, AppError> {
        plu.delta_hash = plu.compute_delta_hash();
        self.plu_repo().upsert(&plu).await?;
        Ok(plu)
    }

    pub async fn delete_plu(&self, key: PluKey) -> Result<bool, AppError> {
        Ok(self.plu_repo().soft_delete(self.store_id, key).await?)
    }

    pub async fn sync_scale(&self, scale_id: Uuid, mode: SyncMode) -> Result<SyncReport, AppError> {
        self.sync.sync_to_scale(self.store_id, scale_id, mode).await
    }

    pub async fn preview_delta_sync(&self, scale_id: Uuid) -> Result<DeltaSyncPreview, AppError> {
        self.sync.preview_delta_sync(self.store_id, scale_id).await
    }

    pub async fn sync_scale_with_options(
        &self,
        scale_id: Uuid,
        mode: SyncMode,
        include_deletes: bool,
    ) -> Result<SyncReport, AppError> {
        self.sync
            .sync_to_scale_with_options(self.store_id, scale_id, mode, include_deletes)
            .await
    }

    /// Updates global Master Catalog prices (P1/P2/P3) for the given PLUs, records an audit entry,
    /// and optionally pushes fast `CHANGE_PRICE` (`CMD 5` on XS / `READONE+WRITE` on XTi) directly
    /// to each selected scale whose branch includes that PLU.
    pub async fn apply_price_changes(
        &self,
        items: Vec<PriceChangeItem>,
        target_scale_ids: Vec<Uuid>,
        send_immediately: bool,
    ) -> Result<PriceChangeReport, AppError> {
        let plu_repo = self.plu_repo();
        let now = Utc::now();
        let mut updated_records: Vec<(Plu, String)> = Vec::new();

        for item in &items {
            let Some(mut plu) = plu_repo.get_by_key(self.store_id, item.key).await? else {
                continue;
            };
            let prev_hash = plu.delta_hash.clone();
            let prev_p1 = plu.price_1.0;

            plu.price_1 = item.price_1;
            if let Some(p2) = item.price_2 {
                plu.price_2 = p2;
            }
            if let Some(p3) = item.price_3 {
                plu.price_3 = p3;
            }
            plu.delta_hash = plu.compute_delta_hash();
            plu.meta.sync_version += 1;
            plu.meta.updated_at = now;
            plu_repo.upsert(&plu).await?;

            let _ = self
                .audit_repo()
                .record(&AuditLog {
                    id: new_id(),
                    store_id: self.store_id,
                    actor_id: None,
                    actor_name: "local".to_string(),
                    action: "price_change".to_string(),
                    target_type: "plu".to_string(),
                    target_id: Some(format!("{}:{}", plu.key.did, plu.key.plu)),
                    details_json: Some(
                        serde_json::json!({
                            "previous_price_1": prev_p1,
                            "new_price_1": plu.price_1.0,
                            "new_price_2": plu.price_2.0,
                            "new_price_3": plu.price_3.0,
                        })
                        .to_string(),
                    ),
                    created_at: now,
                })
                .await;

            updated_records.push((plu, prev_hash));
        }

        let mut scale_results = Vec::new();
        if send_immediately && !updated_records.is_empty() {
            for scale_id in target_scale_ids {
                let handle = match self.scales.get(scale_id).await {
                    Ok(h) => h,
                    Err(err) => {
                        scale_results.push(PriceChangeScaleResult {
                            scale_id,
                            updated: 0,
                            failed: updated_records.len(),
                            error: Some(err.to_string()),
                        });
                        continue;
                    }
                };

                let allowed_keys: std::collections::HashSet<PluKey> = self
                    .sync
                    .effective_catalog_for_scale(self.store_id, scale_id)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| p.key)
                    .collect();

                let mut updated = 0usize;
                let mut failed = 0usize;
                let mut last_err = None;

                for (plu, prev_hash) in &updated_records {
                    if !allowed_keys.contains(&plu.key) {
                        continue;
                    }
                    let res = match handle
                        .change_price(plu.key, plu.price_1, Some(plu.price_2), Some(plu.price_3))
                        .await
                    {
                        Ok(()) => Ok(()),
                        Err(AppError::Driver(
                            openscale_driver::DriverError::NotConnected
                            | openscale_driver::DriverError::Io(_)
                            | openscale_driver::DriverError::Timeout,
                        )) => {
                            if let Err(e) = handle.connect().await {
                                Err(e)
                            } else {
                                handle
                                    .change_price(
                                        plu.key,
                                        plu.price_1,
                                        Some(plu.price_2),
                                        Some(plu.price_3),
                                    )
                                    .await
                            }
                        }
                        Err(e) => Err(e),
                    };

                    match res {
                        Ok(()) => {
                            let _ = self
                                .sync
                                .record_plu_checksum_if_synced(
                                    scale_id,
                                    plu.key,
                                    prev_hash,
                                    &plu.delta_hash,
                                )
                                .await;
                            updated += 1;
                        }
                        Err(e) => {
                            last_err = Some(e.to_string());
                            failed += 1;
                        }
                    }
                }

                scale_results.push(PriceChangeScaleResult {
                    scale_id,
                    updated,
                    failed,
                    error: last_err,
                });
            }
        }

        Ok(PriceChangeReport {
            updated_catalog: updated_records.len(),
            scale_results,
        })
    }

    // --- Master Tables (v0.2) ---

    pub async fn list_departments(&self) -> Result<Vec<Department>, AppError> {
        Ok(self
            .master_tables_repo()
            .list_departments(self.store_id)
            .await?)
    }

    pub async fn upsert_department(&self, dept: Department) -> Result<Department, AppError> {
        self.master_tables_repo().upsert_department(&dept).await?;
        Ok(dept)
    }

    pub async fn list_tares(&self) -> Result<Vec<Tare>, AppError> {
        Ok(self.master_tables_repo().list_tares(self.store_id).await?)
    }

    pub async fn upsert_tare(&self, tare: Tare) -> Result<Tare, AppError> {
        self.master_tables_repo().upsert_tare(&tare).await?;
        Ok(tare)
    }

    pub async fn list_tax_rates(&self) -> Result<Vec<TaxRate>, AppError> {
        Ok(self
            .master_tables_repo()
            .list_tax_rates(self.store_id)
            .await?)
    }

    pub async fn upsert_tax_rate(&self, tax: TaxRate) -> Result<TaxRate, AppError> {
        self.master_tables_repo().upsert_tax_rate(&tax).await?;
        Ok(tax)
    }

    pub async fn list_stored_texts(&self) -> Result<Vec<StoredText>, AppError> {
        Ok(self
            .master_tables_repo()
            .list_stored_texts(self.store_id)
            .await?)
    }

    pub async fn upsert_stored_text(&self, text: StoredText) -> Result<StoredText, AppError> {
        self.master_tables_repo().upsert_stored_text(&text).await?;
        Ok(text)
    }

    pub async fn list_barcode_formats(&self) -> Result<Vec<BarcodeFormat>, AppError> {
        Ok(self
            .master_tables_repo()
            .list_barcode_formats(self.store_id)
            .await?)
    }

    pub async fn upsert_barcode_format(
        &self,
        fmt: BarcodeFormat,
    ) -> Result<BarcodeFormat, AppError> {
        self.master_tables_repo()
            .upsert_barcode_format(&fmt)
            .await?;
        Ok(fmt)
    }

    pub async fn list_scale_barcode_formats(
        &self,
        scale_id: Uuid,
    ) -> Result<Vec<BarcodeFormat>, AppError> {
        let handle = self.scales.get(scale_id).await?;
        if handle.ping().await.is_err() {
            handle.connect().await?;
        }
        handle.list_barcode_formats().await
    }

    pub async fn write_scale_barcode_format(
        &self,
        scale_id: Uuid,
        fmt: BarcodeFormat,
    ) -> Result<BarcodeFormat, AppError> {
        let handle = self.scales.get(scale_id).await?;
        if handle.ping().await.is_err() {
            handle.connect().await?;
        }
        handle.upsert_barcode_format(fmt.clone()).await?;
        self.master_tables_repo()
            .upsert_barcode_format(&fmt)
            .await?;
        Ok(fmt)
    }

    pub async fn list_label_formats(&self) -> Result<Vec<LabelFormat>, AppError> {
        Ok(self
            .master_tables_repo()
            .list_label_formats(self.store_id)
            .await?)
    }

    pub async fn upsert_label_format(&self, fmt: LabelFormat) -> Result<LabelFormat, AppError> {
        self.master_tables_repo().upsert_label_format(&fmt).await?;
        Ok(fmt)
    }

    /// Reads all active/populated label formats directly from a registered scale's TCP actor.
    pub async fn list_scale_label_formats(
        &self,
        scale_id: Uuid,
    ) -> Result<Vec<LabelFormat>, AppError> {
        let handle = self.scales.get(scale_id).await?;
        if handle.ping().await.is_err() {
            handle.connect().await?;
        }
        handle.list_label_formats().await
    }

    /// Writes a label format to a registered scale and persists it to the local SQLite catalog.
    pub async fn write_scale_label_format(
        &self,
        scale_id: Uuid,
        fmt: LabelFormat,
    ) -> Result<LabelFormat, AppError> {
        let handle = self.scales.get(scale_id).await?;
        if handle.ping().await.is_err() {
            handle.connect().await?;
        }
        handle.upsert_label_format(fmt.clone()).await?;
        self.master_tables_repo().upsert_label_format(&fmt).await?;
        Ok(fmt)
    }

    pub async fn preview_import_from_scale(
        &self,
        scale_id: Uuid,
        selection: ImportSelection,
    ) -> Result<ScaleImportPreview, AppError> {
        preview_import_from_scale_impl(&self.db, &self.scales, self.store_id, scale_id, &selection)
            .await
    }

    pub async fn apply_scale_import(
        &self,
        request: ScaleImportApplyRequest,
    ) -> Result<ScaleImportResult, AppError> {
        apply_scale_import_impl(&self.db, request).await
    }

    /// Pushes all active local master tables to a target scale.
    pub async fn sync_master_tables(&self, scale_id: Uuid) -> Result<usize, AppError> {
        let handle = self.scales.get(scale_id).await?;
        let _ = handle.connect().await;
        let mut sent = 0usize;

        for dept in self.list_departments().await? {
            if handle.upsert_department(dept).await.is_ok() {
                sent += 1;
            }
        }
        for tare in self.list_tares().await? {
            if handle.upsert_tare(tare).await.is_ok() {
                sent += 1;
            }
        }
        for tax in self.list_tax_rates().await? {
            if handle.upsert_tax_rate(tax).await.is_ok() {
                sent += 1;
            }
        }
        for text in self.list_stored_texts().await? {
            if handle.upsert_stored_text(text, 0).await.is_ok() {
                sent += 1;
            }
        }
        for fmt in self.list_barcode_formats().await? {
            if handle.upsert_barcode_format(fmt).await.is_ok() {
                sent += 1;
            }
        }

        Ok(sent)
    }

    // --- Sales & Snapshots (v0.2) ---

    pub async fn capture_sales_snapshot(
        &self,
        scale_id: Uuid,
        period: u8,
    ) -> Result<ScaleSalesReport, AppError> {
        let handle = self.scales.get(scale_id).await?;
        let report = handle.capture_sales_report(period).await?;
        let payload_json =
            serde_json::to_string(&report).map_err(openscale_storage::StorageError::Serde)?;

        let snapshot = SalesSnapshot {
            id: new_id(),
            scale_id,
            period,
            captured_at: Utc::now(),
            payload_json,
            meta: SyncMetadata::new(self.store_id),
        };
        self.sales_repo().insert(&snapshot).await?;
        Ok(report)
    }

    pub async fn list_sales_snapshots(
        &self,
        scale_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<SalesSnapshot>, AppError> {
        Ok(self
            .sales_repo()
            .list_recent(self.store_id, scale_id, limit)
            .await?)
    }
}
