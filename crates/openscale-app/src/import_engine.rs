use std::collections::HashMap;

use chrono::Utc;
use openscale_domain::{
    BarcodeFormat, Department, LabelFormat, MoneyCents, Plu, PluKey, StoredText, Tare, TaxRate,
};
use openscale_driver::DriverError;
use openscale_storage::Database;
use openscale_storage::repos::{MasterTablesRepository, PluRepository};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::scale_manager::{ScaleHandle, ScaleManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportCategory {
    Plu,
    Department,
    Tare,
    TaxRate,
    StoredText,
    BarcodeFormat,
    LabelFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSelection {
    pub plus: bool,
    pub departments: bool,
    pub tares: bool,
    pub tax_rates: bool,
    pub stored_texts: bool,
    pub barcode_formats: bool,
    pub label_formats: bool,
}

impl Default for ImportSelection {
    fn default() -> Self {
        Self {
            plus: true,
            departments: true,
            tares: true,
            tax_rates: true,
            stored_texts: true,
            barcode_formats: true,
            label_formats: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportCategoryCounts {
    pub plus: usize,
    pub departments: usize,
    pub tares: usize,
    pub tax_rates: usize,
    pub stored_texts: usize,
    pub barcode_formats: usize,
    pub label_formats: usize,
}

impl ImportCategoryCounts {
    pub fn total(&self) -> usize {
        self.plus
            + self.departments
            + self.tares
            + self.tax_rates
            + self.stored_texts
            + self.barcode_formats
            + self.label_formats
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDiff {
    pub field: String,
    pub local_value: String,
    pub remote_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportConflict {
    pub category: ImportCategory,
    pub key: String,
    pub label: String,
    pub diffs: Vec<FieldDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleImportPreview {
    pub scale_id: Uuid,
    pub new_plus: Vec<Plu>,
    pub new_departments: Vec<Department>,
    pub new_tares: Vec<Tare>,
    pub new_tax_rates: Vec<TaxRate>,
    pub new_stored_texts: Vec<StoredText>,
    pub new_barcode_formats: Vec<BarcodeFormat>,
    pub new_label_formats: Vec<LabelFormat>,
    pub conflict_plus: Vec<Plu>,
    pub conflict_departments: Vec<Department>,
    pub conflict_tares: Vec<Tare>,
    pub conflict_tax_rates: Vec<TaxRate>,
    pub conflict_stored_texts: Vec<StoredText>,
    pub conflict_barcode_formats: Vec<BarcodeFormat>,
    pub conflict_label_formats: Vec<LabelFormat>,
    pub conflicts: Vec<ImportConflict>,
    pub unchanged_counts: ImportCategoryCounts,
    pub unsupported_categories: Vec<ImportCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleImportApplyRequest {
    pub scale_id: Uuid,
    pub plus: Vec<Plu>,
    pub departments: Vec<Department>,
    pub tares: Vec<Tare>,
    pub tax_rates: Vec<TaxRate>,
    pub stored_texts: Vec<StoredText>,
    pub barcode_formats: Vec<BarcodeFormat>,
    pub label_formats: Vec<LabelFormat>,
    pub kept_local_conflicts: usize,
    pub skipped_unchanged: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleImportResult {
    pub scale_id: Uuid,
    pub imported_plus: usize,
    pub imported_departments: usize,
    pub imported_tares: usize,
    pub imported_tax_rates: usize,
    pub imported_stored_texts: usize,
    pub imported_barcode_formats: usize,
    pub imported_label_formats: usize,
    pub total_imported: usize,
    pub kept_local_conflicts: usize,
    pub skipped_unchanged: usize,
}

async fn ensure_connected(handle: &ScaleHandle) -> Result<(), AppError> {
    if handle.ping().await.is_err() {
        handle.connect().await?;
    }
    Ok(())
}

/// Fetches selected data categories from a physical scale and compares them against local SQLite,
/// separating brand-new records, identical records, and conflicting records with field-level diffs.
pub async fn preview_import_from_scale(
    db: &Database,
    scales: &ScaleManager,
    store_id: Uuid,
    scale_id: Uuid,
    selection: &ImportSelection,
) -> Result<ScaleImportPreview, AppError> {
    let handle = scales.get(scale_id).await?;
    ensure_connected(&handle).await?;

    let plu_repo = PluRepository::new(db.pool().clone());
    let master_repo = MasterTablesRepository::new(db.pool().clone());

    let mut preview = ScaleImportPreview {
        scale_id,
        new_plus: Vec::new(),
        new_departments: Vec::new(),
        new_tares: Vec::new(),
        new_tax_rates: Vec::new(),
        new_stored_texts: Vec::new(),
        new_barcode_formats: Vec::new(),
        new_label_formats: Vec::new(),
        conflict_plus: Vec::new(),
        conflict_departments: Vec::new(),
        conflict_tares: Vec::new(),
        conflict_tax_rates: Vec::new(),
        conflict_stored_texts: Vec::new(),
        conflict_barcode_formats: Vec::new(),
        conflict_label_formats: Vec::new(),
        conflicts: Vec::new(),
        unchanged_counts: ImportCategoryCounts::default(),
        unsupported_categories: Vec::new(),
    };

    if selection.plus {
        match handle.list_plus(None, None).await {
            Ok(remote_plus) => {
                let local_map: HashMap<PluKey, Plu> = plu_repo
                    .list_active(store_id)
                    .await?
                    .into_iter()
                    .map(|p| (p.key, p))
                    .collect();

                for mut remote in remote_plus {
                    remote.delta_hash = remote.compute_delta_hash();
                    if let Some(local) = local_map.get(&remote.key) {
                        let diffs = diff_plu(local, &remote);
                        if diffs.is_empty() {
                            preview.unchanged_counts.plus += 1;
                        } else {
                            let key = format!("{}:{}", remote.key.did, remote.key.plu);
                            let label = format!("PLU {key} — {}", remote.description);
                            preview.conflicts.push(ImportConflict {
                                category: ImportCategory::Plu,
                                key,
                                label,
                                diffs,
                            });
                            preview.conflict_plus.push(remote);
                        }
                    } else {
                        preview.new_plus.push(remote);
                    }
                }
            }
            Err(AppError::Driver(DriverError::Unsupported(_))) => {
                preview.unsupported_categories.push(ImportCategory::Plu);
            }
            Err(err) => return Err(err),
        }
    }

    if selection.departments {
        match handle.list_departments().await {
            Ok(remote_depts) => {
                let local_map: HashMap<u32, Department> = master_repo
                    .list_departments(store_id)
                    .await?
                    .into_iter()
                    .map(|d| (d.did, d))
                    .collect();

                for remote in remote_depts {
                    if let Some(local) = local_map.get(&remote.did) {
                        let diffs = diff_department(local, &remote);
                        if diffs.is_empty() {
                            preview.unchanged_counts.departments += 1;
                        } else {
                            let key = remote.did.to_string();
                            preview.conflicts.push(ImportConflict {
                                category: ImportCategory::Department,
                                key,
                                label: format!("Depto {}", remote.did),
                                diffs,
                            });
                            preview.conflict_departments.push(remote);
                        }
                    } else {
                        preview.new_departments.push(remote);
                    }
                }
            }
            Err(AppError::Driver(DriverError::Unsupported(_))) => {
                preview
                    .unsupported_categories
                    .push(ImportCategory::Department);
            }
            Err(err) => return Err(err),
        }
    }

    if selection.tares {
        match handle.list_tares().await {
            Ok(remote_tares) => {
                let local_map: HashMap<u16, Tare> = master_repo
                    .list_tares(store_id)
                    .await?
                    .into_iter()
                    .map(|t| (t.cid, t))
                    .collect();

                for mut remote in remote_tares {
                    if let Some(local) = local_map.get(&remote.cid) {
                        let diffs = diff_tare(local, &remote);
                        if diffs.is_empty() {
                            preview.unchanged_counts.tares += 1;
                        } else {
                            if remote.name == format!("Container {}", remote.cid) {
                                remote.name = local.name.clone();
                            }
                            let key = remote.cid.to_string();
                            preview.conflicts.push(ImportConflict {
                                category: ImportCategory::Tare,
                                key,
                                label: format!("Tara #{} ({})", remote.cid, local.name),
                                diffs,
                            });
                            preview.conflict_tares.push(remote);
                        }
                    } else {
                        preview.new_tares.push(remote);
                    }
                }
            }
            Err(AppError::Driver(DriverError::Unsupported(_))) => {
                preview.unsupported_categories.push(ImportCategory::Tare);
            }
            Err(err) => return Err(err),
        }
    }

    if selection.tax_rates {
        match handle.list_tax_rates().await {
            Ok(remote_taxes) => {
                let local_map: HashMap<u8, TaxRate> = master_repo
                    .list_tax_rates(store_id)
                    .await?
                    .into_iter()
                    .map(|t| (t.tax_id, t))
                    .collect();

                for mut remote in remote_taxes {
                    if let Some(local) = local_map.get(&remote.tax_id) {
                        let diffs = diff_tax_rate(local, &remote);
                        if diffs.is_empty() {
                            preview.unchanged_counts.tax_rates += 1;
                        } else {
                            if remote.name == format!("Tax {}", remote.tax_id) {
                                remote.name = local.name.clone();
                            }
                            let key = remote.tax_id.to_string();
                            preview.conflicts.push(ImportConflict {
                                category: ImportCategory::TaxRate,
                                key,
                                label: format!("Impuesto #{} ({})", remote.tax_id, local.name),
                                diffs,
                            });
                            preview.conflict_tax_rates.push(remote);
                        }
                    } else {
                        preview.new_tax_rates.push(remote);
                    }
                }
            }
            Err(AppError::Driver(DriverError::Unsupported(_))) => {
                preview.unsupported_categories.push(ImportCategory::TaxRate);
            }
            Err(err) => return Err(err),
        }
    }

    if selection.stored_texts {
        match handle.list_stored_texts().await {
            Ok(remote_texts) => {
                let local_map: HashMap<u32, StoredText> = master_repo
                    .list_stored_texts(store_id)
                    .await?
                    .into_iter()
                    .map(|t| (t.text_id, t))
                    .collect();

                for mut remote in remote_texts {
                    if let Some(local) = local_map.get(&remote.text_id) {
                        let diffs = diff_stored_text(local, &remote);
                        if diffs.is_empty() {
                            preview.unchanged_counts.stored_texts += 1;
                        } else {
                            if remote.title == format!("Message {}", remote.text_id) {
                                remote.title = local.title.clone();
                            }
                            let key = remote.text_id.to_string();
                            preview.conflicts.push(ImportConflict {
                                category: ImportCategory::StoredText,
                                key,
                                label: format!("Texto #{} ({})", remote.text_id, local.title),
                                diffs,
                            });
                            preview.conflict_stored_texts.push(remote);
                        }
                    } else {
                        preview.new_stored_texts.push(remote);
                    }
                }
            }
            Err(AppError::Driver(DriverError::Unsupported(_))) => {
                preview
                    .unsupported_categories
                    .push(ImportCategory::StoredText);
            }
            Err(err) => return Err(err),
        }
    }

    if selection.barcode_formats {
        match handle.list_barcode_formats().await {
            Ok(remote_barcodes) => {
                let local_map: HashMap<u16, BarcodeFormat> = master_repo
                    .list_barcode_formats(store_id)
                    .await?
                    .into_iter()
                    .map(|b| (b.bid, b))
                    .collect();

                for mut remote in remote_barcodes {
                    if let Some(local) = local_map.get(&remote.bid) {
                        let diffs = diff_barcode_format(local, &remote);
                        if diffs.is_empty() {
                            preview.unchanged_counts.barcode_formats += 1;
                        } else {
                            if remote.name == format!("Barcode {}", remote.bid) {
                                remote.name = local.name.clone();
                            }
                            let key = remote.bid.to_string();
                            preview.conflicts.push(ImportConflict {
                                category: ImportCategory::BarcodeFormat,
                                key,
                                label: format!("Código barras #{} ({})", remote.bid, local.name),
                                diffs,
                            });
                            preview.conflict_barcode_formats.push(remote);
                        }
                    } else {
                        preview.new_barcode_formats.push(remote);
                    }
                }
            }
            Err(AppError::Driver(DriverError::Unsupported(_))) => {
                preview
                    .unsupported_categories
                    .push(ImportCategory::BarcodeFormat);
            }
            Err(err) => return Err(err),
        }
    }

    if selection.label_formats {
        match handle.list_label_formats().await {
            Ok(remote_labels) => {
                let local_map: HashMap<u16, LabelFormat> = master_repo
                    .list_label_formats(store_id)
                    .await?
                    .into_iter()
                    .map(|l| (l.pfid, l))
                    .collect();

                for remote in remote_labels {
                    if let Some(local) = local_map.get(&remote.pfid) {
                        let diffs = diff_label_format(local, &remote);
                        if diffs.is_empty() {
                            preview.unchanged_counts.label_formats += 1;
                        } else {
                            let key = remote.pfid.to_string();
                            preview.conflicts.push(ImportConflict {
                                category: ImportCategory::LabelFormat,
                                key,
                                label: format!("Etiqueta PFID {} ({})", remote.pfid, local.name),
                                diffs,
                            });
                            preview.conflict_label_formats.push(remote);
                        }
                    } else {
                        preview.new_label_formats.push(remote);
                    }
                }
            }
            Err(AppError::Driver(DriverError::Unsupported(_))) => {
                preview
                    .unsupported_categories
                    .push(ImportCategory::LabelFormat);
            }
            Err(err) => return Err(err),
        }
    }

    Ok(preview)
}

/// Applies the user's resolved import plan to the local SQLite database.
pub async fn apply_scale_import(
    db: &Database,
    req: ScaleImportApplyRequest,
) -> Result<ScaleImportResult, AppError> {
    let plu_repo = PluRepository::new(db.pool().clone());
    let master_repo = MasterTablesRepository::new(db.pool().clone());
    let now = Utc::now().to_rfc3339();

    let mut imported_plus = 0usize;
    for mut plu in req.plus {
        plu.delta_hash = plu.compute_delta_hash();
        plu_repo.upsert(&plu).await?;
        sqlx::query(
            r#"
            INSERT INTO scale_plu_checksums (scale_id, did, plu_number, delta_hash, synced_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(scale_id, did, plu_number) DO UPDATE SET
                delta_hash = excluded.delta_hash,
                synced_at = excluded.synced_at
            "#,
        )
        .bind(req.scale_id.to_string())
        .bind(i64::from(plu.key.did))
        .bind(plu.key.plu as i64)
        .bind(&plu.delta_hash)
        .bind(&now)
        .execute(db.pool())
        .await?;
        imported_plus += 1;
    }

    let mut imported_departments = 0usize;
    for dept in &req.departments {
        master_repo.upsert_department(dept).await?;
        imported_departments += 1;
    }

    let mut imported_tares = 0usize;
    for tare in &req.tares {
        master_repo.upsert_tare(tare).await?;
        imported_tares += 1;
    }

    let mut imported_tax_rates = 0usize;
    for tax in &req.tax_rates {
        master_repo.upsert_tax_rate(tax).await?;
        imported_tax_rates += 1;
    }

    let mut imported_stored_texts = 0usize;
    for text in &req.stored_texts {
        master_repo.upsert_stored_text(text).await?;
        imported_stored_texts += 1;
    }

    let mut imported_barcode_formats = 0usize;
    for fmt in &req.barcode_formats {
        master_repo.upsert_barcode_format(fmt).await?;
        imported_barcode_formats += 1;
    }

    let mut imported_label_formats = 0usize;
    for fmt in &req.label_formats {
        master_repo.upsert_label_format(fmt).await?;
        imported_label_formats += 1;
    }

    let total_imported = imported_plus
        + imported_departments
        + imported_tares
        + imported_tax_rates
        + imported_stored_texts
        + imported_barcode_formats
        + imported_label_formats;

    Ok(ScaleImportResult {
        scale_id: req.scale_id,
        imported_plus,
        imported_departments,
        imported_tares,
        imported_tax_rates,
        imported_stored_texts,
        imported_barcode_formats,
        imported_label_formats,
        total_imported,
        kept_local_conflicts: req.kept_local_conflicts,
        skipped_unchanged: req.skipped_unchanged,
    })
}

fn fmt_money(cents: MoneyCents) -> String {
    format!("${:.2}", cents.0 as f64 / 100.0)
}

fn diff_plu(local: &Plu, remote: &Plu) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();
    let mut push = |field: &str, l: String, r: String| {
        if l != r {
            diffs.push(FieldDiff {
                field: field.to_string(),
                local_value: l,
                remote_value: r,
            });
        }
    };

    push(
        "description",
        local.description.clone(),
        remote.description.clone(),
    );
    push(
        "button_text",
        local.button_text.clone().unwrap_or_default(),
        remote.button_text.clone().unwrap_or_default(),
    );
    push(
        "price_1",
        fmt_money(local.price_1),
        fmt_money(remote.price_1),
    );
    push(
        "price_2",
        fmt_money(local.price_2),
        fmt_money(remote.price_2),
    );
    push(
        "price_3",
        fmt_money(local.price_3),
        fmt_money(remote.price_3),
    );
    push(
        "product_type",
        format!("{:?}", local.product_type),
        format!("{:?}", remote.product_type),
    );
    push(
        "ean",
        local.ean.clone().unwrap_or_default(),
        remote.ean.clone().unwrap_or_default(),
    );
    push(
        "sku",
        local.sku.clone().unwrap_or_default(),
        remote.sku.clone().unwrap_or_default(),
    );
    push(
        "group_id",
        local.group_id.to_string(),
        remote.group_id.to_string(),
    );
    push(
        "barcode_format_id",
        local.barcode_format_id.to_string(),
        remote.barcode_format_id.to_string(),
    );
    push(
        "label_format_id",
        local.label_format_id.to_string(),
        remote.label_format_id.to_string(),
    );
    push(
        "tare_id",
        local.tare_id.unwrap_or(0).to_string(),
        remote.tare_id.unwrap_or(0).to_string(),
    );
    push(
        "tare_grams",
        local.tare_grams.unwrap_or(0).to_string(),
        remote.tare_grams.unwrap_or(0).to_string(),
    );
    push(
        "date_offset_1",
        local.date_offset_1.to_string(),
        remote.date_offset_1.to_string(),
    );
    push(
        "date_offset_2",
        local.date_offset_2.to_string(),
        remote.date_offset_2.to_string(),
    );
    push(
        "tax_rate_id_1",
        local.tax_rate_id_1.to_string(),
        remote.tax_rate_id_1.to_string(),
    );
    push(
        "tax_rate_id_2",
        local.tax_rate_id_2.to_string(),
        remote.tax_rate_id_2.to_string(),
    );

    let local_texts = local.text_areas.join(" | ");
    let remote_texts = remote.text_areas.join(" | ");
    push("text_areas", local_texts, remote_texts);

    diffs
}

fn diff_department(local: &Department, remote: &Department) -> Vec<FieldDiff> {
    if local.name.trim() == remote.name.trim() {
        return Vec::new();
    }
    vec![FieldDiff {
        field: "name".to_string(),
        local_value: local.name.clone(),
        remote_value: remote.name.clone(),
    }]
}

fn diff_tare(local: &Tare, remote: &Tare) -> Vec<FieldDiff> {
    if local.weight_grams == remote.weight_grams {
        return Vec::new();
    }
    vec![FieldDiff {
        field: "weight_grams".to_string(),
        local_value: format!("{} g", local.weight_grams),
        remote_value: format!("{} g", remote.weight_grams),
    }]
}

fn diff_tax_rate(local: &TaxRate, remote: &TaxRate) -> Vec<FieldDiff> {
    if local.rate_basis_points == remote.rate_basis_points {
        return Vec::new();
    }
    vec![FieldDiff {
        field: "rate".to_string(),
        local_value: format!("{:.2}%", local.rate_basis_points as f64 / 100.0),
        remote_value: format!("{:.2}%", remote.rate_basis_points as f64 / 100.0),
    }]
}

fn diff_stored_text(local: &StoredText, remote: &StoredText) -> Vec<FieldDiff> {
    if local.content == remote.content {
        return Vec::new();
    }
    vec![FieldDiff {
        field: "content".to_string(),
        local_value: local.content.clone(),
        remote_value: remote.content.clone(),
    }]
}

fn diff_barcode_format(local: &BarcodeFormat, remote: &BarcodeFormat) -> Vec<FieldDiff> {
    let local_val = serde_json::from_str::<serde_json::Value>(&local.definition_json).ok();
    let remote_val = serde_json::from_str::<serde_json::Value>(&remote.definition_json).ok();
    if local_val.is_some() && local_val == remote_val {
        return Vec::new();
    }
    if local.definition_json == remote.definition_json {
        return Vec::new();
    }

    let fmt_summary = |raw: &str, val: &Option<serde_json::Value>| -> String {
        if let Some(v) = val
            && let Some(tpl) = v.get("template").and_then(|t| t.as_str())
        {
            let bt = v.get("bt").and_then(|x| x.as_u64()).unwrap_or(0);
            return format!("{tpl} (bt={bt})");
        }
        raw.to_string()
    };

    vec![FieldDiff {
        field: "definition".to_string(),
        local_value: fmt_summary(&local.definition_json, &local_val),
        remote_value: fmt_summary(&remote.definition_json, &remote_val),
    }]
}

fn diff_label_format(local: &LabelFormat, remote: &LabelFormat) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();
    if local.name.trim() != remote.name.trim() {
        diffs.push(FieldDiff {
            field: "name".to_string(),
            local_value: local.name.clone(),
            remote_value: remote.name.clone(),
        });
    }
    if local.width_tenths_mm != remote.width_tenths_mm {
        diffs.push(FieldDiff {
            field: "width".to_string(),
            local_value: format!("{:.1} mm", local.width_tenths_mm as f64 / 10.0),
            remote_value: format!("{:.1} mm", remote.width_tenths_mm as f64 / 10.0),
        });
    }

    let local_val = serde_json::from_str::<serde_json::Value>(&local.definition_json).ok();
    let remote_val = serde_json::from_str::<serde_json::Value>(&remote.definition_json).ok();
    let def_equal = if local_val.is_some() && remote_val.is_some() {
        local_val == remote_val
    } else {
        local.definition_json == remote.definition_json
    };

    if !def_equal {
        let count_fields = |v: &Option<serde_json::Value>| -> String {
            if let Some(val) = v
                && let Some(arr) = val.get("fields").and_then(|f| f.as_array())
            {
                return format!("{} campos", arr.len());
            }
            "Definición distinta".to_string()
        };
        diffs.push(FieldDiff {
            field: "fields".to_string(),
            local_value: count_fields(&local_val),
            remote_value: count_fields(&remote_val),
        });
    }

    diffs
}
