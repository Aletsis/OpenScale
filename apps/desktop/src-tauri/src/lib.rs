use std::time::Duration;

use openscale_app::{OpenScaleApp, SyncMode, SyncReport};
use openscale_domain::{Plu, PluKey, Scale};
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
                Ok::<OpenScaleApp, openscale_storage::StorageError>(OpenScaleApp::new(db))
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenScale desktop application");
}
