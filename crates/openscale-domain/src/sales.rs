use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{MoneyCents, PluKey};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrandTotals {
    pub period: u8,
    pub counter_sales: MoneyCents,
    pub prepack_sales: MoneyCents,
    pub transactions: u32,
    pub gross_total: MoneyCents,
    pub last_cleared_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeptTotals {
    pub period: u8,
    pub did: u32,
    pub counter_sales: MoneyCents,
    pub prepack_sales: MoneyCents,
    pub transactions: u32,
    pub last_cleared_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupTotals {
    pub period: u8,
    pub group_id: u32,
    pub counter_sales: MoneyCents,
    pub prepack_sales: MoneyCents,
    pub transactions: u32,
    pub last_cleared_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorTotals {
    pub period: u8,
    pub operator_id: u32,
    pub counter_sales: MoneyCents,
    pub prepack_sales: MoneyCents,
    pub transactions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachTotals {
    pub period: u8,
    pub counter_sales: MoneyCents,
    pub prepack_sales: MoneyCents,
    pub transactions: u32,
    pub gross_total: MoneyCents,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluTotals {
    pub period: u8,
    pub key: PluKey,
    pub transactions: u32,
    pub counter_sales: MoneyCents,
    pub counter_quantity: u32,
    pub counter_transactions: u32,
    pub prepack_sales: MoneyCents,
    pub prepack_quantity: u32,
    pub last_cleared_at: Option<String>,
}

/// Complete point-in-time sales report captured from a scale for a given period (`0` or `1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleSalesReport {
    pub scale_id: Uuid,
    pub period: u8,
    pub captured_at: DateTime<Utc>,
    pub grand_totals: Option<GrandTotals>,
    pub mach_totals: Option<MachTotals>,
    pub dept_totals: Vec<DeptTotals>,
    pub plu_totals: Vec<PluTotals>,
}
