use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::DomainError;

/// Maximum 56-bit PLU number supported by Avery Berkel XS/M-Series (13 digits).
pub const XS_MAX_PLU_NUMBER: u64 = 9_999_999_999_999;
/// Maximum Department ID supported by XS/M-Series.
pub const XS_MAX_DID: u32 = 99;
/// Maximum PLU number supported by XTi/XTRA (14 digits).
pub const XTI_MAX_PLU_NUMBER: u64 = 99_999_999_999_999;
/// Maximum Department ID supported by XTi/XTRA.
pub const XTI_MAX_DID: u32 = 999_999;

/// Generates a time-ordered UUIDv7 identifier.
pub fn new_id() -> Uuid {
    Uuid::now_v7()
}

/// Composite PLU key: Department ID (`did`) + PLU number (`plu`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PluKey {
    pub did: u32,
    pub plu: u64,
}

impl PluKey {
    pub fn new(did: u32, plu: u64) -> Result<Self, DomainError> {
        if did > XTI_MAX_DID {
            return Err(DomainError::InvalidDepartmentId(did));
        }
        if plu == 0 || plu > XTI_MAX_PLU_NUMBER {
            return Err(DomainError::InvalidPluNumber(plu));
        }
        Ok(Self { did, plu })
    }

    /// Encodes into the 8-byte Big-Endian key used by XS/M-Series: `((DID << 56) | PLU)`.
    pub fn to_xs_bytes(&self) -> Result<[u8; 8], DomainError> {
        if self.did > XS_MAX_DID || self.plu > XS_MAX_PLU_NUMBER {
            return Err(DomainError::PluKeyOutOfXsRange {
                did: self.did,
                plu: self.plu,
            });
        }
        let encoded: u64 = ((self.did as u64) << 56) | (self.plu & 0x00FF_FFFF_FFFF_FFFF);
        Ok(encoded.to_be_bytes())
    }

    /// Decodes an 8-byte Big-Endian XS/M-Series `PluKey`.
    pub fn from_xs_bytes(bytes: [u8; 8]) -> Result<Self, DomainError> {
        let raw = u64::from_be_bytes(bytes);
        let did = (raw >> 56) as u32;
        let plu = raw & 0x00FF_FFFF_FFFF_FFFF;
        Self::new(did, plu)
    }
}

/// Monetary amount stored in minor units (cents), avoiding floating-point drift.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct MoneyCents(pub i64);

impl MoneyCents {
    pub fn new(cents: i64) -> Result<Self, DomainError> {
        if cents < 0 {
            return Err(DomainError::InvalidPrice(cents));
        }
        Ok(Self(cents))
    }

    pub fn as_i64(self) -> i64 {
        self.0
    }

    pub fn as_u32(self) -> Result<u32, DomainError> {
        u32::try_from(self.0).map_err(|_| DomainError::InvalidPrice(self.0))
    }
}

/// Unified product pricing/weighing type across XS and XTi families.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductType {
    #[default]
    ByWeight,
    ByCount,
    NegativeByCount,
    FixedWeight,
    FixedPrice,
    ManualWeight,
    ByVolume,
}

/// Tare storage strategy exposed by a scale driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TareMode {
    /// XS/M-Series references a stored container table ID (`CID` 0..=39).
    TableReference,
    /// XTi/XTRA stores the container tare weight directly in grams (`CTV`).
    DirectGrams,
}

/// Supported Avery Berkel (and future) scale protocol families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleFamily {
    AveryBerkelXs,
    AveryBerkelXti,
}

/// Live connectivity state for a registered scale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleStatus {
    #[default]
    Unknown,
    Online,
    Offline,
    Syncing,
    Error,
}

/// Common multi-store and delta-sync metadata attached to persisted entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncMetadata {
    pub store_id: Uuid,
    pub sync_version: i64,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl SyncMetadata {
    pub fn new(store_id: Uuid) -> Self {
        Self {
            store_id,
            sync_version: 1,
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plu_key_xs_roundtrip() {
        let key = PluKey::new(12, 123_456).unwrap();
        let bytes = key.to_xs_bytes().unwrap();
        assert_eq!(bytes[0], 12);
        let decoded = PluKey::from_xs_bytes(bytes).unwrap();
        assert_eq!(decoded, key);
    }

    #[test]
    fn plu_key_xs_rejects_out_of_range_did() {
        let key = PluKey::new(100, 1).unwrap();
        assert!(matches!(
            key.to_xs_bytes(),
            Err(DomainError::PluKeyOutOfXsRange { did: 100, plu: 1 })
        ));
    }

    #[test]
    fn money_cents_rejects_negative() {
        assert!(MoneyCents::new(-1).is_err());
        let price = MoneyCents::new(4500).unwrap();
        assert_eq!(price.as_i64(), 4500);
        assert_eq!(price.as_u32().unwrap(), 4500);
        assert!(MoneyCents(i64::MAX).as_u32().is_err());
    }

    #[test]
    fn plu_key_validates_bounds() {
        assert!(matches!(
            PluKey::new(0, 0),
            Err(DomainError::InvalidPluNumber(0))
        ));
        assert!(matches!(
            PluKey::new(XTI_MAX_DID + 1, 1),
            Err(DomainError::InvalidDepartmentId(_))
        ));
        assert!(matches!(
            PluKey::new(1, XTI_MAX_PLU_NUMBER + 1),
            Err(DomainError::InvalidPluNumber(_))
        ));
    }
}
