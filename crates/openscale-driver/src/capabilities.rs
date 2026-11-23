use openscale_domain::TareMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DidRange {
    pub min: u32,
    pub max: u32,
}

impl DidRange {
    pub const fn new(min: u32, max: u32) -> Self {
        Self { min, max }
    }

    pub const fn contains(self, did: u32) -> bool {
        did >= self.min && did <= self.max
    }
}

/// Declarative capability matrix exposed by every scale protocol driver.
/// Eliminates model-specific conditionals in application services, API, and UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverCapabilities {
    pub supports_fast_price_change: bool,
    pub supports_service_port: bool,
    pub supports_live_weight: bool,
    pub supports_lcd_mirror: bool,
    pub supports_remote_keys: bool,
    pub supports_sql_filter: bool,
    pub did_range: DidRange,
    pub plu_max_digits: u8,
    pub text_areas_count: u8,
    pub tare_mode: TareMode,
}

impl DriverCapabilities {
    /// Default capabilities for Avery Berkel XS / M-Series (`MSeriesProtocol`).
    pub const fn xs_default() -> Self {
        Self {
            supports_fast_price_change: true,
            supports_service_port: true,
            supports_live_weight: true,
            supports_lcd_mirror: true,
            supports_remote_keys: true,
            supports_sql_filter: false,
            did_range: DidRange::new(0, 99),
            plu_max_digits: 13,
            text_areas_count: 6,
            tare_mode: TareMode::TableReference,
        }
    }

    /// Default capabilities for Avery Berkel XTi / XTRA (`XtraDataPort`).
    pub const fn xti_default() -> Self {
        Self {
            supports_fast_price_change: false,
            supports_service_port: false,
            supports_live_weight: false,
            supports_lcd_mirror: false,
            supports_remote_keys: false,
            supports_sql_filter: true,
            did_range: DidRange::new(1, 999_999),
            plu_max_digits: 14,
            text_areas_count: 12,
            tare_mode: TareMode::DirectGrams,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xs_and_xti_presets_match_protocol_specs() {
        let xs = DriverCapabilities::xs_default();
        assert!(xs.supports_fast_price_change);
        assert!(xs.supports_service_port);
        assert!(!xs.supports_sql_filter);
        assert!(xs.did_range.contains(0));
        assert!(xs.did_range.contains(99));
        assert!(!xs.did_range.contains(100));
        assert_eq!(xs.text_areas_count, 6);
        assert_eq!(xs.tare_mode, TareMode::TableReference);

        let xti = DriverCapabilities::xti_default();
        assert!(!xti.supports_fast_price_change);
        assert!(xti.supports_sql_filter);
        assert!(!xti.did_range.contains(0));
        assert!(xti.did_range.contains(1));
        assert_eq!(xti.plu_max_digits, 14);
        assert_eq!(xti.text_areas_count, 12);
        assert_eq!(xti.tare_mode, TareMode::DirectGrams);
    }
}
