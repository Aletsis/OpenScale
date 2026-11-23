use async_trait::async_trait;
use openscale_domain::{
    BarcodeFormat, Department, DeptTotals, GrandTotals, GroupTotals, LabelFormat, MachTotals,
    MediaAsset, MoneyCents, OperatorTotals, Plu, PluKey, PluTotals, StoredText, Tare, TaxRate,
};

use crate::capabilities::DriverCapabilities;
use crate::error::DriverError;
use crate::events::{ScaleDateTime, ScaleInfo};

/// Unified asynchronous interface implemented by all scale protocol drivers.
#[async_trait]
pub trait ScaleDriver: Send + Sync {
    /// Returns the static capability matrix for this driver.
    fn capabilities(&self) -> DriverCapabilities;

    /// Opens the persistent TCP session and performs the initial protocol handshake.
    async fn connect(&mut self) -> Result<ScaleInfo, DriverError>;

    /// Gracefully closes the active session.
    async fn disconnect(&mut self) -> Result<(), DriverError>;

    /// Lightweight health check against the scale.
    async fn ping(&mut self) -> Result<(), DriverError>;

    /// Reads scale identity, hardware, and firmware metadata.
    async fn get_info(&mut self) -> Result<ScaleInfo, DriverError>;

    /// Reads the scale's internal real-time clock.
    async fn get_datetime(&mut self) -> Result<ScaleDateTime, DriverError>;

    /// Updates the scale's internal real-time clock.
    async fn set_datetime(&mut self, dt: ScaleDateTime) -> Result<(), DriverError>;

    /// Reads a single PLU by composite key (`DID`, `PLU`).
    async fn read_plu(&mut self, key: PluKey) -> Result<Option<Plu>, DriverError>;

    /// Lists PLUs in an optional inclusive department/PLU range.
    async fn list_plus(
        &mut self,
        from: Option<PluKey>,
        to: Option<PluKey>,
    ) -> Result<Vec<Plu>, DriverError>;

    /// Fast PLU count (uses `O(log N)` binary search on XS or query on XTi).
    async fn count_plus(&mut self) -> Result<u64, DriverError>;

    /// Creates or updates a PLU record on the scale.
    async fn upsert_plu(&mut self, plu: &Plu) -> Result<(), DriverError>;

    /// Updates PLU prices (uses `CHANGE_PRICE` on XS, `READONE + WRITE` on XTi).
    async fn change_price(
        &mut self,
        key: PluKey,
        price_1: MoneyCents,
        price_2: Option<MoneyCents>,
        price_3: Option<MoneyCents>,
    ) -> Result<(), DriverError>;

    /// Deletes a single PLU from the scale.
    async fn delete_plu(&mut self, key: PluKey) -> Result<(), DriverError>;

    // --- Master Tables (v0.2) ---

    async fn read_department(&mut self, _did: u32) -> Result<Option<Department>, DriverError> {
        Err(DriverError::Unsupported("read_department"))
    }

    async fn upsert_department(&mut self, _dept: &Department) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("upsert_department"))
    }

    async fn read_tare(&mut self, _cid: u16) -> Result<Option<Tare>, DriverError> {
        Err(DriverError::Unsupported("read_tare"))
    }

    async fn upsert_tare(&mut self, _tare: &Tare) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("upsert_tare"))
    }

    async fn read_tax_rate(&mut self, _tax_id: u8) -> Result<Option<TaxRate>, DriverError> {
        Err(DriverError::Unsupported("read_tax_rate"))
    }

    async fn upsert_tax_rate(&mut self, _tax: &TaxRate) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("upsert_tax_rate"))
    }

    async fn read_stored_text(
        &mut self,
        _text_id: u32,
        _table: u8,
    ) -> Result<Option<StoredText>, DriverError> {
        Err(DriverError::Unsupported("read_stored_text"))
    }

    async fn upsert_stored_text(
        &mut self,
        _text: &StoredText,
        _table: u8,
    ) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("upsert_stored_text"))
    }

    async fn read_barcode_format(
        &mut self,
        _bid: u16,
    ) -> Result<Option<BarcodeFormat>, DriverError> {
        Err(DriverError::Unsupported("read_barcode_format"))
    }

    async fn upsert_barcode_format(&mut self, _fmt: &BarcodeFormat) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("upsert_barcode_format"))
    }

    async fn read_media_asset(
        &mut self,
        _media_id: u32,
    ) -> Result<Option<MediaAsset>, DriverError> {
        Err(DriverError::Unsupported("read_media_asset"))
    }

    async fn read_label_format(
        &mut self,
        _pfid: u16,
    ) -> Result<Option<LabelFormat>, DriverError> {
        Err(DriverError::Unsupported("read_label_format"))
    }

    async fn upsert_label_format(&mut self, _fmt: &LabelFormat) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("upsert_label_format"))
    }

    async fn list_departments(&mut self) -> Result<Vec<Department>, DriverError> {
        Err(DriverError::Unsupported("list_departments"))
    }

    async fn list_tares(&mut self) -> Result<Vec<Tare>, DriverError> {
        Err(DriverError::Unsupported("list_tares"))
    }

    async fn list_tax_rates(&mut self) -> Result<Vec<TaxRate>, DriverError> {
        Err(DriverError::Unsupported("list_tax_rates"))
    }

    async fn list_stored_texts(&mut self) -> Result<Vec<StoredText>, DriverError> {
        Err(DriverError::Unsupported("list_stored_texts"))
    }

    async fn list_barcode_formats(&mut self) -> Result<Vec<BarcodeFormat>, DriverError> {
        Err(DriverError::Unsupported("list_barcode_formats"))
    }

    async fn list_label_formats(&mut self) -> Result<Vec<LabelFormat>, DriverError> {
        Err(DriverError::Unsupported("list_label_formats"))
    }

    // --- Sales & Totals (v0.2) ---

    async fn read_grand_totals(&mut self, _period: u8) -> Result<Option<GrandTotals>, DriverError> {
        Err(DriverError::Unsupported("read_grand_totals"))
    }

    async fn list_dept_totals(&mut self, _period: u8) -> Result<Vec<DeptTotals>, DriverError> {
        Err(DriverError::Unsupported("list_dept_totals"))
    }

    async fn read_group_totals(
        &mut self,
        _period: u8,
        _group_id: u32,
    ) -> Result<Option<GroupTotals>, DriverError> {
        Err(DriverError::Unsupported("read_group_totals"))
    }

    async fn read_operator_totals(
        &mut self,
        _period: u8,
        _operator_id: u32,
    ) -> Result<Option<OperatorTotals>, DriverError> {
        Err(DriverError::Unsupported("read_operator_totals"))
    }

    async fn read_mach_totals(&mut self, _period: u8) -> Result<Option<MachTotals>, DriverError> {
        Err(DriverError::Unsupported("read_mach_totals"))
    }

    async fn list_plu_totals(&mut self, _period: u8) -> Result<Vec<PluTotals>, DriverError> {
        Err(DriverError::Unsupported("list_plu_totals"))
    }
}
