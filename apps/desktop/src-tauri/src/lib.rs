mod logging;

use std::collections::HashMap;
use std::error::Error as StdError;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use logging::{DEFAULT_LOG_TAIL_LINES, LogInfo, LogManager, LogReadResult, format_error_chain};
use openscale_app::{
    DEFAULT_HEALTH_CHECK_INTERVAL, DeltaSyncPreview, DiscoveredScaleMatch, ImportSelection,
    OpenScaleApp, PriceChangeItem, PriceChangeReport, ScaleHealthCheckResult,
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
use openscale_storage::repos::ScaleConfigProfile;
use openscale_storage::{DEFAULT_LOCAL_STORE_ID, Database, StorageConfig};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;
use uuid::Uuid;

struct DesktopState {
    app: OpenScaleApp,
    logger: Arc<LogManager>,
    service_sessions: Arc<Mutex<HashMap<String, XsServiceClient>>>,
}

fn log_cmd_err<E>(command: &str, context: &str, err: E) -> String
where
    E: StdError + std::fmt::Debug + 'static,
{
    let err_msg = err.to_string();
    let chain = format_error_chain(&err);
    tracing::error!(
        target: "openscale_desktop::ipc",
        command = command,
        context = context,
        error = %err_msg,
        error_chain = %chain,
        error_debug = ?err,
        "IPC command failed"
    );
    err_msg
}

macro_rules! run_service_op {
    ($state:expr, $op_name:expr, $host:expr, $port:expr, |$client:ident| $op:expr) => {{
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
                        result = Err(log_cmd_err($op_name, &format!("connect target={key}"), e));
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
                    if should_retry {
                        tracing::warn!(
                            target: "openscale_desktop::service_port",
                            command = $op_name,
                            target = %key,
                            error = %err,
                            "Cached XsServicePort session failed; retrying with fresh connection"
                        );
                    } else {
                        result = Err(log_cmd_err($op_name, &format!("target={key}"), err));
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
        .map_err(|e| log_cmd_err("discover_scales", "", e))
}

#[tauri::command]
async fn discover_scales_with_identity(
    state: State<'_, DesktopState>,
) -> Result<Vec<DiscoveredScaleMatch>, String> {
    state
        .app
        .discover_scales_with_identity(Duration::from_secs(2))
        .await
        .map_err(|e| log_cmd_err("discover_scales_with_identity", "", e))
}

#[tauri::command]
async fn reassign_scale_ip(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    new_ip: String,
) -> Result<Scale, String> {
    let ctx = format!("scale_id={scale_id} new_ip={new_ip}");
    state
        .app
        .reassign_scale_ip(scale_id, new_ip)
        .await
        .map_err(|e| log_cmd_err("reassign_scale_ip", &ctx, e))
}

#[tauri::command]
async fn swap_scale_ips(
    state: State<'_, DesktopState>,
    scale_a_id: Uuid,
    scale_b_id: Uuid,
) -> Result<(Scale, Scale), String> {
    let ctx = format!("scale_a_id={scale_a_id} scale_b_id={scale_b_id}");
    state
        .app
        .swap_scale_ips(scale_a_id, scale_b_id)
        .await
        .map_err(|e| log_cmd_err("swap_scale_ips", &ctx, e))
}

#[tauri::command]
async fn get_app_setting(
    state: State<'_, DesktopState>,
    key: String,
) -> Result<Option<String>, String> {
    state
        .app
        .get_app_setting(&key)
        .await
        .map_err(|e| log_cmd_err("get_app_setting", &format!("key={key}"), e))
}

#[tauri::command]
async fn set_app_setting(
    state: State<'_, DesktopState>,
    key: String,
    value_json: String,
) -> Result<(), String> {
    state
        .app
        .set_app_setting(&key, &value_json)
        .await
        .map_err(|e| log_cmd_err("set_app_setting", &format!("key={key}"), e))
}

#[tauri::command]
async fn get_scale_config_profile(
    state: State<'_, DesktopState>,
    scope: String,
    target_id: String,
    family: String,
) -> Result<Option<ScaleConfigProfile>, String> {
    let ctx = format!("scope={scope} target_id={target_id} family={family}");
    state
        .app
        .get_scale_config_profile(&scope, &target_id, &family)
        .await
        .map_err(|e| log_cmd_err("get_scale_config_profile", &ctx, e))
}

#[tauri::command]
async fn save_scale_config_profile(
    state: State<'_, DesktopState>,
    profile: ScaleConfigProfile,
) -> Result<ScaleConfigProfile, String> {
    let ctx = format!(
        "id={} scope={} target_id={}",
        profile.id, profile.scope, profile.target_id
    );
    state
        .app
        .save_scale_config_profile(profile)
        .await
        .map_err(|e| log_cmd_err("save_scale_config_profile", &ctx, e))
}

#[tauri::command]
async fn list_branches(state: State<'_, DesktopState>) -> Result<Vec<Branch>, String> {
    state
        .app
        .list_branches()
        .await
        .map_err(|e| log_cmd_err("list_branches", "", e))
}

#[tauri::command]
async fn upsert_branch(state: State<'_, DesktopState>, branch: Branch) -> Result<Branch, String> {
    let ctx = format!("branch_id={} code={}", branch.id, branch.code);
    state
        .app
        .upsert_branch(branch)
        .await
        .map_err(|e| log_cmd_err("upsert_branch", &ctx, e))
}

#[tauri::command]
async fn delete_branch(state: State<'_, DesktopState>, branch_id: Uuid) -> Result<bool, String> {
    state
        .app
        .delete_branch(branch_id)
        .await
        .map_err(|e| log_cmd_err("delete_branch", &format!("branch_id={branch_id}"), e))
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
        .map_err(|e| log_cmd_err("list_branch_plu_keys", &format!("branch_id={branch_id}"), e))
}

#[tauri::command]
async fn set_branch_plu_keys(
    state: State<'_, DesktopState>,
    branch_id: Uuid,
    keys: Vec<PluKey>,
) -> Result<usize, String> {
    let ctx = format!("branch_id={branch_id} keys_count={}", keys.len());
    state
        .app
        .set_branch_plu_keys(branch_id, keys)
        .await
        .map_err(|e| log_cmd_err("set_branch_plu_keys", &ctx, e))
}

#[tauri::command]
async fn list_scales(state: State<'_, DesktopState>) -> Result<Vec<Scale>, String> {
    state
        .app
        .list_scales()
        .await
        .map_err(|e| log_cmd_err("list_scales", "", e))
}

#[tauri::command]
async fn upsert_scale(state: State<'_, DesktopState>, scale: Scale) -> Result<Scale, String> {
    let ctx = format!("scale_id={} ip={}", scale.id, scale.ip_address);
    state
        .app
        .upsert_scale(scale)
        .await
        .map_err(|e| log_cmd_err("upsert_scale", &ctx, e))
}

#[tauri::command]
async fn delete_scale(state: State<'_, DesktopState>, scale_id: Uuid) -> Result<bool, String> {
    state
        .app
        .delete_scale(scale_id)
        .await
        .map_err(|e| log_cmd_err("delete_scale", &format!("scale_id={scale_id}"), e))
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
        .map_err(|e| log_cmd_err("check_scale_health", &format!("scale_id={scale_id}"), e))
}

#[tauri::command]
async fn check_all_scales_health(
    state: State<'_, DesktopState>,
) -> Result<Vec<ScaleHealthCheckResult>, String> {
    state
        .app
        .check_all_scales_health()
        .await
        .map_err(|e| log_cmd_err("check_all_scales_health", "", e))
}

#[tauri::command]
async fn list_plus(state: State<'_, DesktopState>) -> Result<Vec<Plu>, String> {
    state
        .app
        .list_plus()
        .await
        .map_err(|e| log_cmd_err("list_plus", "", e))
}

#[tauri::command]
async fn get_plu(state: State<'_, DesktopState>, key: PluKey) -> Result<Option<Plu>, String> {
    let ctx = format!("did={} plu={}", key.did, key.plu);
    state
        .app
        .get_plu(key)
        .await
        .map_err(|e| log_cmd_err("get_plu", &ctx, e))
}

#[tauri::command]
async fn upsert_plu(state: State<'_, DesktopState>, plu: Plu) -> Result<Plu, String> {
    let ctx = format!("did={} plu={}", plu.key.did, plu.key.plu);
    state
        .app
        .upsert_plu(plu)
        .await
        .map_err(|e| log_cmd_err("upsert_plu", &ctx, e))
}

#[tauri::command]
async fn delete_plu(state: State<'_, DesktopState>, key: PluKey) -> Result<bool, String> {
    let ctx = format!("did={} plu={}", key.did, key.plu);
    state
        .app
        .delete_plu(key)
        .await
        .map_err(|e| log_cmd_err("delete_plu", &ctx, e))
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
        .map_err(|e| log_cmd_err("import_from_scale", &format!("scale_id={scale_id}"), e))
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
        .map_err(|e| {
            log_cmd_err(
                "preview_import_from_scale",
                &format!("scale_id={scale_id}"),
                e,
            )
        })
}

#[tauri::command]
async fn apply_scale_import(
    state: State<'_, DesktopState>,
    request: ScaleImportApplyRequest,
) -> Result<ScaleImportResult, String> {
    let ctx = format!("scale_id={}", request.scale_id);
    state
        .app
        .apply_scale_import(request)
        .await
        .map_err(|e| log_cmd_err("apply_scale_import", &ctx, e))
}

#[tauri::command]
async fn list_label_formats(state: State<'_, DesktopState>) -> Result<Vec<LabelFormat>, String> {
    state
        .app
        .list_label_formats()
        .await
        .map_err(|e| log_cmd_err("list_label_formats", "", e))
}

#[tauri::command]
async fn upsert_label_format(
    state: State<'_, DesktopState>,
    fmt: LabelFormat,
) -> Result<LabelFormat, String> {
    let ctx = format!("label_format_id={}", fmt.pfid);
    state
        .app
        .upsert_label_format(fmt)
        .await
        .map_err(|e| log_cmd_err("upsert_label_format", &ctx, e))
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
        .map_err(|e| log_cmd_err("scale_list_label_formats", &format!("scale_id={scale_id}"), e))
}

#[tauri::command]
async fn scale_write_label_format(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    fmt: LabelFormat,
) -> Result<LabelFormat, String> {
    let ctx = format!("scale_id={scale_id} label_format_id={}", fmt.pfid);
    state
        .app
        .write_scale_label_format(scale_id, fmt)
        .await
        .map_err(|e| log_cmd_err("scale_write_label_format", &ctx, e))
}

#[tauri::command]
async fn sync_scale(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    mode: SyncMode,
) -> Result<SyncReport, String> {
    let ctx = format!("scale_id={scale_id} mode={mode:?}");
    state
        .app
        .sync_scale(scale_id, mode)
        .await
        .map_err(|e| log_cmd_err("sync_scale", &ctx, e))
}

#[tauri::command]
async fn preview_delta_sync(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
) -> Result<DeltaSyncPreview, String> {
    state
        .app
        .preview_delta_sync(scale_id)
        .await
        .map_err(|e| log_cmd_err("preview_delta_sync", &format!("scale_id={scale_id}"), e))
}

#[tauri::command]
async fn sync_scale_with_options(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    mode: SyncMode,
    include_deletes: bool,
) -> Result<SyncReport, String> {
    let ctx = format!("scale_id={scale_id} mode={mode:?} include_deletes={include_deletes}");
    state
        .app
        .sync_scale_with_options(scale_id, mode, include_deletes)
        .await
        .map_err(|e| log_cmd_err("sync_scale_with_options", &ctx, e))
}

#[tauri::command]
async fn apply_price_changes(
    state: State<'_, DesktopState>,
    items: Vec<PriceChangeItem>,
    target_scale_ids: Vec<Uuid>,
    send_immediately: bool,
) -> Result<PriceChangeReport, String> {
    let ctx = format!(
        "items={} target_scales={} send_immediately={send_immediately}",
        items.len(),
        target_scale_ids.len()
    );
    state
        .app
        .apply_price_changes(items, target_scale_ids, send_immediately)
        .await
        .map_err(|e| log_cmd_err("apply_price_changes", &ctx, e))
}

#[tauri::command]
async fn list_departments(state: State<'_, DesktopState>) -> Result<Vec<Department>, String> {
    state
        .app
        .list_departments()
        .await
        .map_err(|e| log_cmd_err("list_departments", "", e))
}

#[tauri::command]
async fn upsert_department(
    state: State<'_, DesktopState>,
    dept: Department,
) -> Result<Department, String> {
    let ctx = format!("dept_id={}", dept.id);
    state
        .app
        .upsert_department(dept)
        .await
        .map_err(|e| log_cmd_err("upsert_department", &ctx, e))
}

#[tauri::command]
async fn list_tares(state: State<'_, DesktopState>) -> Result<Vec<Tare>, String> {
    state
        .app
        .list_tares()
        .await
        .map_err(|e| log_cmd_err("list_tares", "", e))
}

#[tauri::command]
async fn upsert_tare(state: State<'_, DesktopState>, tare: Tare) -> Result<Tare, String> {
    let ctx = format!("tare_id={}", tare.id);
    state
        .app
        .upsert_tare(tare)
        .await
        .map_err(|e| log_cmd_err("upsert_tare", &ctx, e))
}

#[tauri::command]
async fn list_tax_rates(state: State<'_, DesktopState>) -> Result<Vec<TaxRate>, String> {
    state
        .app
        .list_tax_rates()
        .await
        .map_err(|e| log_cmd_err("list_tax_rates", "", e))
}

#[tauri::command]
async fn upsert_tax_rate(state: State<'_, DesktopState>, tax: TaxRate) -> Result<TaxRate, String> {
    let ctx = format!("tax_id={}", tax.id);
    state
        .app
        .upsert_tax_rate(tax)
        .await
        .map_err(|e| log_cmd_err("upsert_tax_rate", &ctx, e))
}

#[tauri::command]
async fn list_stored_texts(state: State<'_, DesktopState>) -> Result<Vec<StoredText>, String> {
    state
        .app
        .list_stored_texts()
        .await
        .map_err(|e| log_cmd_err("list_stored_texts", "", e))
}

#[tauri::command]
async fn upsert_stored_text(
    state: State<'_, DesktopState>,
    text: StoredText,
) -> Result<StoredText, String> {
    let ctx = format!("text_id={}", text.id);
    state
        .app
        .upsert_stored_text(text)
        .await
        .map_err(|e| log_cmd_err("upsert_stored_text", &ctx, e))
}

#[tauri::command]
async fn list_barcode_formats(
    state: State<'_, DesktopState>,
) -> Result<Vec<BarcodeFormat>, String> {
    state
        .app
        .list_barcode_formats()
        .await
        .map_err(|e| log_cmd_err("list_barcode_formats", "", e))
}

#[tauri::command]
async fn upsert_barcode_format(
    state: State<'_, DesktopState>,
    fmt: BarcodeFormat,
) -> Result<BarcodeFormat, String> {
    let ctx = format!("barcode_id={}", fmt.bid);
    state
        .app
        .upsert_barcode_format(fmt)
        .await
        .map_err(|e| log_cmd_err("upsert_barcode_format", &ctx, e))
}

#[tauri::command]
async fn scale_list_barcode_formats(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
) -> Result<Vec<BarcodeFormat>, String> {
    state
        .app
        .list_scale_barcode_formats(scale_id)
        .await
        .map_err(|e| {
            log_cmd_err(
                "scale_list_barcode_formats",
                &format!("scale_id={scale_id}"),
                e,
            )
        })
}

#[tauri::command]
async fn scale_write_barcode_format(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    fmt: BarcodeFormat,
) -> Result<BarcodeFormat, String> {
    let ctx = format!("scale_id={scale_id} barcode_id={}", fmt.bid);
    state
        .app
        .write_scale_barcode_format(scale_id, fmt)
        .await
        .map_err(|e| log_cmd_err("scale_write_barcode_format", &ctx, e))
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
        .map_err(|e| log_cmd_err("sync_master_tables", &format!("scale_id={scale_id}"), e))
}

#[tauri::command]
async fn capture_sales_snapshot(
    state: State<'_, DesktopState>,
    scale_id: Uuid,
    period: u8,
) -> Result<ScaleSalesReport, String> {
    let ctx = format!("scale_id={scale_id} period={period}");
    state
        .app
        .capture_sales_snapshot(scale_id, period)
        .await
        .map_err(|e| log_cmd_err("capture_sales_snapshot", &ctx, e))
}

#[tauri::command]
async fn list_sales_snapshots(
    state: State<'_, DesktopState>,
    scale_id: Option<Uuid>,
) -> Result<Vec<SalesSnapshot>, String> {
    let ctx = format!("scale_id={scale_id:?}");
    state
        .app
        .list_sales_snapshots(scale_id, 50)
        .await
        .map_err(|e| log_cmd_err("list_sales_snapshots", &ctx, e))
}

// --- Hito v0.3: XsServicePort (28822), MACHINE_PARAMETERS & LABEL_FORMATS ---

#[tauri::command]
async fn xs_service_read_weight(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
) -> Result<ServiceWeightReading, String> {
    run_service_op!(state, "xs_service_read_weight", host, port, |c| c.read_weight())
}

#[tauri::command]
async fn xs_service_balance(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
) -> Result<String, String> {
    run_service_op!(state, "xs_service_balance", host, port, |c| c.balance_scale())
}

#[tauri::command]
async fn xs_service_read_settings(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
) -> Result<ScaleSettings748, String> {
    run_service_op!(state, "xs_service_read_settings", host, port, |c| c.read_scale_settings())
}

#[tauri::command]
async fn xs_service_write_settings(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
    settings: ScaleSettings748,
) -> Result<String, String> {
    run_service_op!(state, "xs_service_write_settings", host, port, |c| c.write_scale_settings(&settings))
}

#[tauri::command]
async fn xs_service_read_lcd(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
) -> Result<LcdFrame, String> {
    run_service_op!(state, "xs_service_read_lcd", host, port, |c| c.read_display_bitmap())
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
        Some(ks) => run_service_op!(state, "xs_service_send_key", host, port, |c| c.write_keystroke(scan_code, ks)),
        None => run_service_op!(state, "xs_service_send_key", host, port, |c| c.tap_keystroke(scan_code)),
    }
}

#[tauri::command]
async fn xs_service_read_diag_log(
    state: State<'_, DesktopState>,
    host: String,
    port: Option<u16>,
    log_type: String,
) -> Result<Vec<String>, String> {
    run_service_op!(state, "xs_service_read_diag_log", host, port, |c| c
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
        "print_test" => run_service_op!(state, "xs_service_action:print_test", host, port, |c| c.print_test_report()),
        "cash_drawer" => run_service_op!(state, "xs_service_action:cash_drawer", host, port, |c| c.activate_cash_drawer()),
        "reboot" => run_service_op!(state, "xs_service_action:reboot", host, port, |c| c.reboot()),
        "memory_plu" => run_service_op!(state, "xs_service_action:memory_plu", host, port, |c| c.read_memory_details("PLU")),
        other => {
            tracing::warn!(
                target: "openscale_desktop::ipc",
                command = "xs_service_action",
                action = other,
                "Unknown service action requested"
            );
            Err(format!("unknown service action: {other}"))
        }
    }
}

#[tauri::command]
async fn xs_read_machine_params(
    host: String,
    port: Option<u16>,
) -> Result<MachineParameters, String> {
    let ctx = format!("host={host} port={port:?}");
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver
        .connect()
        .await
        .map_err(|e| log_cmd_err("xs_read_machine_params:connect", &ctx, e))?;
    let res = driver
        .read_machine_parameters()
        .await
        .map_err(|e| log_cmd_err("xs_read_machine_params", &ctx, e));
    let _ = driver.disconnect().await;
    res
}

#[tauri::command]
async fn xs_write_machine_params(
    host: String,
    port: Option<u16>,
    params: MachineParameters,
) -> Result<(), String> {
    let ctx = format!("host={host} port={port:?}");
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver
        .connect()
        .await
        .map_err(|e| log_cmd_err("xs_write_machine_params:connect", &ctx, e))?;
    let res = driver
        .write_machine_parameters(&params)
        .await
        .map_err(|e| log_cmd_err("xs_write_machine_params", &ctx, e));
    let _ = driver.disconnect().await;
    res
}

#[tauri::command]
async fn xs_list_label_formats(
    host: String,
    port: Option<u16>,
) -> Result<Vec<LabelFormatRecord>, String> {
    let ctx = format!("host={host} port={port:?}");
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver
        .connect()
        .await
        .map_err(|e| log_cmd_err("xs_list_label_formats:connect", &ctx, e))?;
    let res = driver
        .list_label_format_records()
        .await
        .map_err(|e| log_cmd_err("xs_list_label_formats", &ctx, e));
    let _ = driver.disconnect().await;
    res
}

#[tauri::command]
async fn xs_read_label_format(
    host: String,
    port: Option<u16>,
    pfid: u8,
) -> Result<LabelFormatRecord, String> {
    let ctx = format!("host={host} port={port:?} pfid={pfid}");
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver
        .connect()
        .await
        .map_err(|e| log_cmd_err("xs_read_label_format:connect", &ctx, e))?;
    let res = driver
        .read_label_format(pfid)
        .await
        .map_err(|e| log_cmd_err("xs_read_label_format", &ctx, e));
    let _ = driver.disconnect().await;
    res
}

#[tauri::command]
async fn xs_write_label_format(
    host: String,
    port: Option<u16>,
    record: LabelFormatRecord,
) -> Result<(), String> {
    let ctx = format!("host={host} port={port:?} pfid={}", record.pfid);
    let mut cfg = XsDriverConfig::new(host, DEFAULT_LOCAL_STORE_ID);
    if let Some(p) = port {
        cfg.port = p;
    }
    let mut driver = XsDriver::new(cfg);
    driver
        .connect()
        .await
        .map_err(|e| log_cmd_err("xs_write_label_format:connect", &ctx, e))?;
    let res = driver
        .write_label_format(&record)
        .await
        .map_err(|e| log_cmd_err("xs_write_label_format", &ctx, e));
    let _ = driver.disconnect().await;
    res
}

// --- Application Logging & Diagnostics ---

#[tauri::command]
async fn log_frontend_event(
    state: State<'_, DesktopState>,
    level: String,
    source: String,
    message: String,
    stack: Option<String>,
    context: Option<String>,
) -> Result<(), String> {
    state.logger.log_frontend(
        &level,
        &source,
        &message,
        stack.as_deref(),
        context.as_deref(),
    );
    Ok(())
}

#[tauri::command]
async fn get_log_info(state: State<'_, DesktopState>) -> Result<LogInfo, String> {
    state.logger.info()
}

#[tauri::command]
async fn read_log_entries(
    state: State<'_, DesktopState>,
    file_name: Option<String>,
    max_lines: Option<usize>,
    level_filter: Option<String>,
) -> Result<LogReadResult, String> {
    state.logger.read_entries(
        file_name.as_deref(),
        max_lines.unwrap_or(DEFAULT_LOG_TAIL_LINES),
        level_filter.as_deref(),
    )
}

#[tauri::command]
async fn open_logs_dir(state: State<'_, DesktopState>) -> Result<(), String> {
    let dir = state.logger.log_dir().to_path_buf();
    #[cfg(target_os = "windows")]
    let spawn_res = std::process::Command::new("explorer").arg(&dir).spawn();
    #[cfg(target_os = "macos")]
    let spawn_res = std::process::Command::new("open").arg(&dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let spawn_res = std::process::Command::new("xdg-open").arg(&dir).spawn();

    spawn_res.map(|_| ()).map_err(|e| {
        log_cmd_err(
            "open_logs_dir",
            &format!("dir={}", dir.display()),
            e,
        )
    })
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

            let log_dir = std::env::var("OPENSCALE_LOG_DIR")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join("logs"));
            let logger = LogManager::init(log_dir)?;
            logger.install_global();

            let db_path = data_dir.join("openscale.db");
            tracing::info!(
                target: "openscale_desktop::startup",
                version = env!("CARGO_PKG_VERSION"),
                os = std::env::consts::OS,
                arch = std::env::consts::ARCH,
                data_dir = %data_dir.display(),
                db_path = %db_path.display(),
                log_file = %logger.current_log_path().display(),
                "Starting OpenScale desktop application"
            );

            let db_path_for_init = db_path.clone();
            let core = tauri::async_runtime::block_on(async move {
                let db = match Database::connect(&StorageConfig::new(db_path_for_init)).await {
                    Ok(db) => db,
                    Err(err) => {
                        tracing::error!(
                            target: "openscale_desktop::startup",
                            error = %err,
                            error_chain = %format_error_chain(&err),
                            error_debug = ?err,
                            "Failed to initialize SQLite database"
                        );
                        return Err(err);
                    }
                };
                let app_core = OpenScaleApp::new(db);
                match app_core.list_scales().await {
                    Ok(saved_scales) => {
                        tracing::info!(
                            target: "openscale_desktop::startup",
                            scale_count = saved_scales.len(),
                            "Loaded registered scales from database"
                        );
                        for scale in &saved_scales {
                            app_core.scales.register_scale(scale).await;
                        }
                    }
                    Err(err) => {
                        tracing::error!(
                            target: "openscale_desktop::startup",
                            error = %err,
                            error_debug = ?err,
                            "Failed to load registered scales on startup"
                        );
                    }
                }
                Ok::<OpenScaleApp, openscale_storage::StorageError>(app_core)
            })?;

            // Forward real-time ScaleManager events to Tauri frontend
            let mut rx = core.scales.subscribe();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    if let Err(err) = handle.emit("openscale://event", &event) {
                        tracing::warn!(
                            target: "openscale_desktop::events",
                            error = %err,
                            "Failed to emit openscale://event to frontend"
                        );
                    }
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
                logger,
                service_sessions: Arc::new(Mutex::new(HashMap::new())),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover_scales,
            discover_scales_with_identity,
            reassign_scale_ip,
            swap_scale_ips,
            get_app_setting,
            set_app_setting,
            get_scale_config_profile,
            save_scale_config_profile,
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
            preview_delta_sync,
            sync_scale_with_options,
            apply_price_changes,
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
            scale_list_barcode_formats,
            scale_write_barcode_format,
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
            log_frontend_event,
            get_log_info,
            read_log_entries,
            open_logs_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenScale desktop application");
}
