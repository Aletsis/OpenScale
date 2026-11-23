pub mod discovery;
pub mod error;
pub mod import_engine;
pub mod scale_manager;
pub mod service;
pub mod sync_engine;

pub use discovery::discover_all_scales;
pub use error::AppError;
pub use import_engine::{
    FieldDiff, ImportCategory, ImportCategoryCounts, ImportConflict, ImportSelection,
    ScaleImportApplyRequest, ScaleImportPreview, ScaleImportResult,
};
pub use scale_manager::{ScaleHandle, ScaleManager};
pub use service::OpenScaleApp;
pub use sync_engine::{SyncEngine, SyncMode, SyncReport};
