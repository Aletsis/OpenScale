use std::time::Duration;

use chrono::Utc;
use openscale_domain::types::new_id;
use openscale_domain::{
    BarcodeFormat, Department, LabelFormat, Plu, PluKey, SalesSnapshot, Scale, ScaleSalesReport,
    StoredText, SyncMetadata, Tare, TaxRate,
};
use openscale_driver::DiscoveredScale;
use openscale_storage::repos::{
    AuditRepository, MasterTablesRepository, PluRepository, SalesSnapshotRepository,
    ScaleRepository,
};
use openscale_storage::{DEFAULT_LOCAL_STORE_ID, Database};
use uuid::Uuid;

use crate::discovery::discover_all_scales;
use crate::error::AppError;
use crate::import_engine::{
    ImportSelection, ScaleImportApplyRequest, ScaleImportPreview, ScaleImportResult,
    apply_scale_import as apply_scale_import_impl,
    preview_import_from_scale as preview_import_from_scale_impl,
};
use crate::scale_manager::ScaleManager;
use crate::sync_engine::{SyncEngine, SyncMode, SyncReport};

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

    pub async fn discover_scales(
        &self,
        timeout: Duration,
    ) -> Result<Vec<DiscoveredScale>, AppError> {
        discover_all_scales(timeout, &[], &[]).await
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
