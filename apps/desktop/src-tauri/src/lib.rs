use std::time::Duration;

use openscale_app::{OpenScaleApp, SyncMode, SyncReport};
use openscale_domain::{
    BarcodeFormat, Department, Plu, PluKey, SalesSnapshot, Scale, ScaleSalesReport, StoredText,
    Tare, TaxRate,
};
use openscale_driver::DiscoveredScale;
use openscale_storage::{Database, StorageConfig};
use tauri::{Emitter, Manager, State};
use uuid::Uuid;

struct DesktopState {
    app: OpenScaleApp,
}

#[tauri::command]
async fn discover_scales(state: State<'_, DesktopState>) -> Result<Vec<DiscoveredScale>, String> {
    state
        .app
        .discover_scales(Duration::from_secs(2))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_scales(state: State<'_, DesktopState>) -> Result<Vec<Scale>, String> {
    state.app.list_scales().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn upsert_scale(state: State<'_, DesktopState>, scale: Scale) -> Result<Scale, String> {
    state
        .app
        .upsert_scale(scale)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_plus(state: State<'_, DesktopState>) -> Result<Vec<Plu>, String> {
    state.app.list_plus().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_plu(state: State<'_, DesktopState>, key: PluKey) -> Result<Option<Plu>, String> {
    state.app.get_plu(key).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn upsert_plu(state: State<'_, DesktopState>, plu: Plu) -> Result<Plu, String> {
    state.app.upsert_plu(plu).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_plu(state: State<'_, DesktopState>, key: PluKey) -> Result<bool, String> {
    state.app.delete_plu(key).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_from_scale(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
) -> Result<usize, String> {
    state
        .app
        .sync
        .import_from_scale(scale_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn sync_scale(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    mode: SyncMode,
) -> Result<SyncReport, String> {
    state
        .app
        .sync_scale(scale_id, mode)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_departments(state: State<'_, DesktopState>) -> Result<Vec<Department>, String> {
    state
        .app
        .list_departments()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn upsert_department(
    state: State<'_, DesktopState>,
    dept: Department,
) -> Result<Department, String> {
    state
        .app
        .upsert_department(dept)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_tares(state: State<'_, DesktopState>) -> Result<Vec<Tare>, String> {
    state.app.list_tares().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn upsert_tare(state: State<'_, DesktopState>, tare: Tare) -> Result<Tare, String> {
    state.app.upsert_tare(tare).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_tax_rates(state: State<'_, DesktopState>) -> Result<Vec<TaxRate>, String> {
    state.app.list_tax_rates().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn upsert_tax_rate(state: State<'_, DesktopState>, tax: TaxRate) -> Result<TaxRate, String> {
    state
        .app
        .upsert_tax_rate(tax)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_stored_texts(state: State<'_, DesktopState>) -> Result<Vec<StoredText>, String> {
    state
        .app
        .list_stored_texts()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn upsert_stored_text(
    state: State<'_, DesktopState>,
    text: StoredText,
) -> Result<StoredText, String> {
    state
        .app
        .upsert_stored_text(text)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_barcode_formats(
    state: State<'_, DesktopState>,
) -> Result<Vec<BarcodeFormat>, String> {
    state
        .app
        .list_barcode_formats()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn upsert_barcode_format(
    state: State<'_, DesktopState>,
    fmt: BarcodeFormat,
) -> Result<BarcodeFormat, String> {
    state
        .app
        .upsert_barcode_format(fmt)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn sync_master_tables(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
) -> Result<usize, String> {
    state
        .app
        .sync_master_tables(scale_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn capture_sales_snapshot(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    period: u8,
) -> Result<ScaleSalesReport, String> {
    state
        .app
        .capture_sales_snapshot(scale_id, period)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_sales_snapshots(
    state: State<'_, DesktopState>,
    scale_id: Option<Uuid>,
) -> Result<Vec<SalesSnapshot>, String> {
    state
        .app
        .list_sales_snapshots(scale_id, 50)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .expect("resolve app local data dir");
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("openscale.db");

            let core = tauri::async_runtime::block_on(async move {
                let db = Database::connect(&StorageConfig::new(db_path)).await?;
                let app_core = OpenScaleApp::new(db);
                if let Ok(saved_scales) = app_core.list_scales().await {
                    for scale in &saved_scales {
                        app_core.scales.register_scale(scale).await;
                    }
                }
                Ok::<OpenScaleApp, openscale_storage::StorageError>(app_core)
            })?;

            // Forward real-time ScaleManager events to Tauri frontend
            let mut rx = core.scales.subscribe();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    let _ = handle.emit("openscale://event", &event);
                }
            });

            app.manage(DesktopState { app: core });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover_scales,
            list_scales,
            upsert_scale,
            list_plus,
            get_plu,
            upsert_plu,
            delete_plu,
            import_from_scale,
            sync_scale,
            list_departments,
            upsert_department,
            list_tares,
            upsert_tare,
            list_tax_rates,
            upsert_tax_rate,
            list_stored_texts,
            upsert_stored_text,
            list_barcode_formats,
            upsert_barcode_format,
            sync_master_tables,
            capture_sales_snapshot,
            list_sales_snapshots,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenScale desktop application");
}
