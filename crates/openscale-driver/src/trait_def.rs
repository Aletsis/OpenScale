use async_trait::async_trait;
use openscale_domain::{MoneyCents, Plu, PluKey};

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
}
