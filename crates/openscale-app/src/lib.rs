pub mod discovery;
pub mod error;
pub mod health_monitor;
pub mod identity;
pub mod import_engine;
pub mod scale_manager;
pub mod service;
pub mod sync_engine;

pub use discovery::discover_all_scales;
pub use error::AppError;
pub use health_monitor::{DEFAULT_HEALTH_CHECK_INTERVAL, ScaleHealthCheckResult};
pub use identity::{DiscoveredScaleMatch, ScaleIdentityStatus, classify_discovered_scales};
pub use import_engine::{
    FieldDiff, ImportCategory, ImportCategoryCounts, ImportConflict, ImportSelection,
    ScaleImportApplyRequest, ScaleImportPreview, ScaleImportResult,
};
pub use scale_manager::{ScaleHandle, ScaleManager};
pub use service::{OpenScaleApp, PriceChangeItem, PriceChangeReport, PriceChangeScaleResult};
pub use sync_engine::{DeltaSyncPreview, SyncEngine, SyncMode, SyncReport};
