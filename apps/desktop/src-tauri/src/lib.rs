use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use openscale_app::{
    DEFAULT_HEALTH_CHECK_INTERVAL, ImportSelection, OpenScaleApp, ScaleHealthCheckResult,
    ScaleImportApplyRequest, ScaleImportPreview, ScaleImportResult, SyncMode, SyncReport,
};
use openscale_domain::{
    BarcodeFormat, Branch, Department, LabelFormat, Plu, PluKey, SalesSnapshot, Scale,
    ScaleSalesReport, StoredText, Tare, TaxRate,
};
use openscale_driver::{DiscoveredScale, DriverError, LcdFrame, ScaleDriver};
use openscale_driver_xs::{
    LabelFormatRecord, MachineParameters, ScaleSettings748, ServiceWeightReading, XS_SERVICE_PORT,
    XsDriver, XsDriverConfig, XsServiceClient,
};
use openscale_storage::{DEFAULT_LOCAL_STORE_ID, Database, StorageConfig};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;
use uuid::Uuid;

struct DesktopState {
    app: OpenScaleApp,
    service_sessions: Arc<Mutex<HashMap<String, XsServiceClient>>>,
}

macro_rules! run_service_op {
    ($state:expr, $host:expr, $port:expr, |$client:ident| $op:expr) => {{
        let host_trimmed = $host.trim().to_string();
        let key = format!("{}:{}", host_trimmed, $port.unwrap_or(XS_SERVICE_PORT));
        let mut sessions = $state.service_sessions.lock().await;
        let mut result: Result<_, String> = Err("XsServicePort session failed".to_string());

        for attempt in 0..2 {
            let was_cached = sessions.contains_key(&key);
            if !was_cached {
                match XsServiceClient::connect(&host_trimmed, $port, Duration::from_secs(5)).await {
                    Ok(client) => {
                        sessions.insert(key.clone(), client);
                    }
                    Err(e) => {
                        result = Err(e.to_string());
                        break;
                    }
                }
            }

            let $client = sessions.get_mut(&key).expect("session just inserted");
            match ($op).await {
                Ok(val) => {
                    result = Ok(val);
                    break;
                }
                Err(err) => {
                    let is_protocol_err = matches!(err, DriverError::Protocol { .. });
                    let should_retry = was_cached
                        && attempt == 0
                        && matches!(err, DriverError::Io(_) | DriverError::ConnectionFailed(_));

                    if !is_protocol_err {
                        sessions.remove(&key);
                    }
                    if !should_retry {
                        result = Err(err.to_string());
                        break;
                    }
                }
            }
        }
        result
    }};
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
async fn list_branches(state: State<'_, DesktopState>) -> Result<Vec<Branch>, String> {
    state.app.list_branches().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn upsert_branch(state: State<'_, DesktopState>, branch: Branch) -> Result<Branch, String> {
    state
        .app
        .upsert_branch(branch)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_branch(state: State<'_, DesktopState>, branch_id: Uuid) -> Result<bool, String> {
    state
        .app
        .delete_branch(branch_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_branch_plu_keys(
    state: State<'_, DesktopState>,
    branch_id: Uuid,
) -> Result<Vec<PluKey>, String> {
    state
        .app
        .list_branch_plu_keys(branch_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_branch_plu_keys(
    state: State<'_, DesktopState>,
    branch_id: Uuid,
    keys: Vec<PluKey>,
) -> Result<usize, String> {
    state
        .app
        .set_branch_plu_keys(branch_id, keys)
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
async fn delete_scale(state: State<'_, DesktopState>, scale_id: Uuid) -> Result<bool, String> {
    state
        .app
        .delete_scale(scale_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_scale_health(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
) -> Result<ScaleHealthCheckResult, String> {
    state
        .app
        .check_scale_health(scale_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_all_scales_health(
    state: State<'_, DesktopState>,
) -> Result<Vec<ScaleHealthCheckResult>, String> {
    state
        .app
        .check_all_scales_health()
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
async fn preview_import_from_scale(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    selection: ImportSelection,
) -> Result<ScaleImportPreview, String> {
    state
        .app
        .preview_import_from_scale(scale_id, selection)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn apply_scale_import(
    state: State<'_, DesktopState>,
    request: ScaleImportApplyRequest,
) -> Result<ScaleImportResult, String> {
    state
        .app
        .apply_scale_import(request)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_label_formats(state: State<'_, DesktopState>) -> Result<Vec<LabelFormat>, String> {
    state
        .app
        .list_label_formats()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn upsert_label_format(
    state: State<'_, DesktopState>,
    fmt: LabelFormat,
) -> Result<LabelFormat, String> {
    state
        .app
        .upsert_label_format(fmt)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn scale_list_label_formats(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
) -> Result<Vec<LabelFormat>, String> {
    state
        .app
        .list_scale_label_formats(scale_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn scale_write_label_format(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    fmt: LabelFormat,
) -> Result<LabelFormat, String> {
    state
        .app
        .write_scale_label_format(scale_id, fmt)
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

// --- Hito v0.3: XsServicePort (28822), MACHINE_PARAMETERS & LABEL_FORMATS ---

#[tauri::command]
async fn xs_service_read_weight(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
) -> Result<ServiceWeightReading, String> {
    run_service_op!(state, host, port, |c| c.read_weight())
}

#[tauri::command]
async fn xs_service_balance(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
) -> Result<String, String> {
    run_service_op!(state, host, port, |c| c.balance_scale())
}

#[tauri::command]
async fn xs_service_read_settings(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
) -> Result<ScaleSettings748, String> {
    run_service_op!(state, host, port, |c| c.read_scale_settings())
}

#[tauri::command]
async fn xs_service_write_settings(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
    settings: ScaleSettings748,
) -> Result<String, String> {
    run_service_op!(state, host, port, |c| c.write_scale_settings(&settings))
}

#[tauri::command]
async fn xs_service_read_lcd(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
) -> Result<LcdFrame, String> {
    run_service_op!(state, host, port, |c| c.read_display_bitmap())
}

#[tauri::command]
async fn xs_service_send_key(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
    scan_code: u8,
    key_state: Option<u8>,
) -> Result<String, String> {
    match key_state {
        Some(ks) => run_service_op!(state, host, port, |c| c.write_keystroke(scan_code, ks)),
        None => run_service_op!(state, host, port, |c| c.tap_keystroke(scan_code)),
    }
}

#[tauri::command]
async fn xs_service_read_diag_log(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
    log_type: String,
) -> Result<Vec<String>, String> {
    run_service_op!(state, host, port, |c| c
        .read_diagnostic_log(&log_type, false))
}

#[tauri::command]
async fn xs_service_action(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
    action: String,
) -> Result<String, String> {
    match action.as_str() {
        "print_test" => run_service_op!(state, host, port, |c| c.print_test_report()),
        "cash_drawer" => run_service_op!(state, host, port, |c| c.activate_cash_drawer()),
        "reboot" => run_service_op!(state, host, port, |c| c.reboot()),
        "memory_plu" => run_service_op!(state, host, port, |c| c.read_memory_details("PLU")),
        other => Err(format!("unknown service action: {other}")),
    }
}

#[tauri::command]
async fn xs_read_machine_params(
    host: String,
    port: Option<u16>,
) -> Result<MachineParameters, String> {
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver.connect().await.map_err(|e| e.to_string())?;
    let res = driver
        .read_machine_parameters()
        .await
        .map_err(|e| e.to_string());
    let _ = driver.disconnect().await;
    res
}

#[tauri::command]
async fn xs_write_machine_params(
    host: String,
    port: Option<u16>,
    params: MachineParameters,
) -> Result<(), String> {
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver.connect().await.map_err(|e| e.to_string())?;
    let res = driver
        .write_machine_parameters(&params)
        .await
        .map_err(|e| e.to_string());
    let _ = driver.disconnect().await;
    res
}

#[tauri::command]
async fn xs_list_label_formats(
    host: String,
    port: Option<u16>,
) -> Result<Vec<LabelFormatRecord>, String> {
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver.connect().await.map_err(|e| e.to_string())?;
    let res = driver
        .list_label_format_records()
        .await
        .map_err(|e| e.to_string());
    let _ = driver.disconnect().await;
    res
}

#[tauri::command]
async fn xs_read_label_format(
    host: String,
    port: Option<u16>,
    pfid: u8,
) -> Result<LabelFormatRecord, String> {
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver.connect().await.map_err(|e| e.to_string())?;
    let res = driver
        .read_label_format(pfid)
        .await
        .map_err(|e| e.to_string());
    let _ = driver.disconnect().await;
    res
}

#[tauri::command]
async fn xs_write_label_format(
    host: String,
    port: Option<u16>,
    record: LabelFormatRecord,
) -> Result<(), String> {
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver.connect().await.map_err(|e| e.to_string())?;
    let res = driver
        .write_label_format(&record)
        .await
        .map_err(|e| e.to_string());
    let _ = driver.disconnect().await;
    res
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

            // Start periodic background connectivity monitor for all registered scales
            let monitor_app = core.clone();
            tauri::async_runtime::spawn(async move {
                let _ = monitor_app
                    .start_health_monitor(DEFAULT_HEALTH_CHECK_INTERVAL)
                    .await;
            });

            app.manage(DesktopState {
                app: core,
                service_sessions: Arc::new(Mutex::new(HashMap::new())),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover_scales,
            list_branches,
            upsert_branch,
            delete_branch,
            list_branch_plu_keys,
            set_branch_plu_keys,
            list_scales,
            upsert_scale,
            delete_scale,
            check_scale_health,
            check_all_scales_health,
            list_plus,
            get_plu,
            upsert_plu,
            delete_plu,
            import_from_scale,
            preview_import_from_scale,
            apply_scale_import,
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
            list_label_formats,
            upsert_label_format,
            scale_list_label_formats,
            scale_write_label_format,
            sync_master_tables,
            capture_sales_snapshot,
            list_sales_snapshots,
            xs_service_read_weight,
            xs_service_balance,
            xs_service_read_settings,
            xs_service_write_settings,
            xs_service_read_lcd,
            xs_service_send_key,
            xs_service_read_diag_log,
            xs_service_action,
            xs_read_machine_params,
            xs_write_machine_params,
            xs_list_label_formats,
            xs_read_label_format,
            xs_write_label_format,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenScale desktop application");
}
