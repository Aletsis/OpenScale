use std::time::Duration;

use openscale_domain::{Plu, PluKey, Scale};
use openscale_driver::DiscoveredScale;
use openscale_storage::repos::{AuditRepository, PluRepository, ScaleRepository};
use openscale_storage::{DEFAULT_LOCAL_STORE_ID, Database};
use uuid::Uuid;

use crate::discovery::discover_all_scales;
use crate::error::AppError;
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
}
