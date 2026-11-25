use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::types::{MoneyCents, PluKey, ProductType, ScaleFamily, ScaleStatus, SyncMetadata};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Store {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub timezone: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchCatalogMode {
    /// Branch inherits every active PLU in the Master Catalog.
    #[default]
    All,
    /// Branch uses only the subset of PLUs explicitly assigned in `branch_plus`.
    Selected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub address: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub catalog_mode: BranchCatalogMode,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleGroup {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scale {
    pub id: Uuid,
    #[serde(default)]
    pub branch_id: Option<Uuid>,
    pub group_id: Option<Uuid>,
    pub name: String,
    pub family: ScaleFamily,
    pub ip_address: String,
    pub data_port: u16,
    pub service_port: Option<u16>,
    pub serial_number: Option<String>,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
    pub mac_address: Option<String>,
    pub status: ScaleStatus,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Department {
    pub id: Uuid,
    pub did: u32,
    pub name: String,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plu {
    pub id: Uuid,
    pub key: PluKey,
    pub description: String,
    pub button_text: Option<String>,
    pub price_1: MoneyCents,
    pub price_2: MoneyCents,
    pub price_3: MoneyCents,
    pub product_type: ProductType,
    pub ean: Option<String>,
    pub sku: Option<String>,
    pub group_id: u32,
    pub barcode_format_id: u16,
    pub label_format_id: u16,
    pub tare_id: Option<u16>,
    pub tare_grams: Option<u32>,
    pub date_offset_1: u16,
    pub date_print_control_1: u8,
    pub date_offset_2: u16,
    pub date_print_control_2: u8,
    pub tax_rate_id_1: u8,
    pub tax_rate_id_2: u8,
    pub text_areas: Vec<String>,
    pub delta_hash: String,
    pub meta: SyncMetadata,
}

impl Plu {
    /// Computes a deterministic SHA-256 hex digest of all sync-relevant PLU fields.
    pub fn compute_delta_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.key.did.to_be_bytes());
        hasher.update(self.key.plu.to_be_bytes());
        hasher.update(self.description.as_bytes());
        hasher.update([0u8]);
        if let Some(ref btn) = self.button_text {
            hasher.update(btn.as_bytes());
        }
        hasher.update([0u8]);
        hasher.update(self.price_1.0.to_be_bytes());
        hasher.update(self.price_2.0.to_be_bytes());
        hasher.update(self.price_3.0.to_be_bytes());
        hasher.update(format!("{:?}", self.product_type).as_bytes());
        hasher.update([0u8]);
        if let Some(ref ean) = self.ean {
            hasher.update(ean.as_bytes());
        }
        hasher.update([0u8]);
        if let Some(ref sku) = self.sku {
            hasher.update(sku.as_bytes());
        }
        hasher.update([0u8]);
        hasher.update(self.group_id.to_be_bytes());
        hasher.update(self.barcode_format_id.to_be_bytes());
        hasher.update(self.label_format_id.to_be_bytes());
        hasher.update(self.tare_id.unwrap_or(0).to_be_bytes());
        hasher.update(self.tare_grams.unwrap_or(0).to_be_bytes());
        hasher.update(self.date_offset_1.to_be_bytes());
        hasher.update([self.date_print_control_1]);
        hasher.update(self.date_offset_2.to_be_bytes());
        hasher.update([self.date_print_control_2]);
        hasher.update([self.tax_rate_id_1, self.tax_rate_id_2]);
        for area in &self.text_areas {
            hasher.update(area.as_bytes());
            hasher.update([0u8]);
        }
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use std::fmt::Write;
            let _ = write!(&mut hex, "{byte:02x}");
        }
        hex
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tare {
    pub id: Uuid,
    pub cid: u16,
    pub name: String,
    pub weight_grams: u32,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxRate {
    pub id: Uuid,
    pub tax_id: u8,
    pub name: String,
    pub rate_basis_points: u32,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredText {
    pub id: Uuid,
    pub text_id: u32,
    pub title: String,
    pub content: String,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAsset {
    pub id: Uuid,
    pub media_id: u32,
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarcodeFormat {
    pub id: Uuid,
    pub bid: u16,
    pub name: String,
    pub definition_json: String,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelFormat {
    pub id: Uuid,
    pub pfid: u16,
    pub name: String,
    pub width_tenths_mm: u16,
    pub height_tenths_mm: u16,
    pub definition_json: String,
    pub raw_binary: Option<Vec<u8>>,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesSnapshot {
    pub id: Uuid,
    pub scale_id: Uuid,
    pub period: u8,
    pub captured_at: DateTime<Utc>,
    pub payload_json: String,
    pub meta: SyncMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncJobStatus {
    Pending,
    Running,
    Completed,
    PartialFailure,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncJob {
    pub id: Uuid,
    pub store_id: Uuid,
    pub mode: String,
    pub status: SyncJobStatus,
    pub total_items: i64,
    pub processed_items: i64,
    pub failed_items: i64,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncJobItem {
    pub id: Uuid,
    pub job_id: Uuid,
    pub scale_id: Uuid,
    pub entity_type: String,
    pub entity_key: String,
    pub action: String,
    pub status: String,
    pub error_message: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub store_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_name: String,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub details_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::new_id;

    #[test]
    fn plu_delta_hash_is_deterministic_and_sensitive_to_price() {
        let store_id = new_id();
        let mut plu = Plu {
            id: new_id(),
            key: PluKey::new(1, 100).unwrap(),
            description: "POLLO FRESCO".to_string(),
            button_text: None,
            price_1: MoneyCents(4500),
            price_2: MoneyCents(0),
            price_3: MoneyCents(0),
            product_type: ProductType::ByWeight,
            ean: Some("2000100000000".to_string()),
            sku: None,
            group_id: 0,
            barcode_format_id: 1,
            label_format_id: 1,
            tare_id: Some(0),
            tare_grams: None,
            date_offset_1: 7,
            date_print_control_1: 1,
            date_offset_2: 0,
            date_print_control_2: 0,
            tax_rate_id_1: 0,
            tax_rate_id_2: 0,
            text_areas: vec!["Mantener refrigerado".to_string()],
            delta_hash: String::new(),
            meta: SyncMetadata::new(store_id),
        };

        let h1 = plu.compute_delta_hash();
        let h2 = plu.compute_delta_hash();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);

        plu.price_1 = MoneyCents(4600);
        let h3 = plu.compute_delta_hash();
        assert_ne!(h1, h3);
    }
}
