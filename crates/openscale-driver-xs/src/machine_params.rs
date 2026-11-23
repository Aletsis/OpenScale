use openscale_driver::DriverError;
use serde::{Deserialize, Serialize};

pub const MACHINE_PARAMS_LEN: usize = 80;

/// Safe 80-byte wrapper for `MACHINE_PARAMETERS` (`CMD=24, CVN=8000`).
/// Preserves all untouched bytes when patching individual parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineParameters {
    pub raw: Vec<u8>,
    pub default_label_p1: u8,
    pub default_label_p2: u8,
    pub print_mode: u8,
    pub default_barcode_id: u8,
    pub label_detect: bool,
    pub forced_tare: u8,
    pub checkout_mode: u8,
    pub pos_mode: bool,
    pub self_service_mode: bool,
    pub one_shot_label_mode: bool,
    pub label_print_speed: u8,
    pub receipt_print_speed: u8,
    pub tear_bar_fitted: bool,
}

impl MachineParameters {
    pub fn decode(bytes: &[u8]) -> Result<Self, DriverError> {
        if bytes.len() < MACHINE_PARAMS_LEN {
            return Err(DriverError::Protocol {
                code: None,
                message: format!(
                    "MACHINE_PARAMETERS payload too short: {} bytes (expected {MACHINE_PARAMS_LEN})",
                    bytes.len()
                ),
            });
        }
        let raw = bytes[..MACHINE_PARAMS_LEN].to_vec();
        Ok(Self {
            default_label_p1: raw[0x05],
            default_label_p2: raw[0x07],
            print_mode: raw[0x09],
            default_barcode_id: raw[0x0C],
            label_detect: raw[0x13] != 0,
            forced_tare: raw[0x14],
            checkout_mode: raw[0x18],
            pos_mode: raw[0x19] != 0,
            self_service_mode: raw[0x20] != 0,
            one_shot_label_mode: raw[0x28] != 0,
            label_print_speed: raw[0x30],
            receipt_print_speed: raw[0x32],
            tear_bar_fitted: raw[0x4E] != 0,
            raw,
        })
    }

    /// Patches the typed fields back into the preserved 80-byte buffer.
    pub fn encode(&self) -> Result<[u8; MACHINE_PARAMS_LEN], DriverError> {
        if self.raw.len() < MACHINE_PARAMS_LEN {
            return Err(DriverError::Protocol {
                code: None,
                message: format!(
                    "MACHINE_PARAMETERS raw buffer too short: {} bytes",
                    self.raw.len()
                ),
            });
        }
        let mut out = [0u8; MACHINE_PARAMS_LEN];
        out.copy_from_slice(&self.raw[..MACHINE_PARAMS_LEN]);
        out[0x05] = self.default_label_p1;
        out[0x07] = self.default_label_p2;
        out[0x09] = self.print_mode;
        out[0x0C] = self.default_barcode_id;
        out[0x13] = u8::from(self.label_detect);
        out[0x14] = self.forced_tare;
        out[0x18] = self.checkout_mode;
        out[0x19] = u8::from(self.pos_mode);
        out[0x20] = u8::from(self.self_service_mode);
        out[0x28] = u8::from(self.one_shot_label_mode);
        out[0x30] = self.label_print_speed;
        out[0x32] = self.receipt_print_speed;
        out[0x4E] = u8::from(self.tear_bar_fitted);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_parameters_preserves_unknown_bytes_on_roundtrip() {
        let mut raw = [0u8; MACHINE_PARAMS_LEN];
        raw[0x00] = 0xAB;
        raw[0x4F] = 0xCD;
        raw[0x09] = 2; // Pre-pack
        raw[0x20] = 1; // Self-service
        raw[0x30] = 80; // Label print speed 80%

        let mut params = MachineParameters::decode(&raw).unwrap();
        assert_eq!(params.print_mode, 2);
        assert!(params.self_service_mode);
        assert_eq!(params.label_print_speed, 80);

        params.print_mode = 0; // On request
        let encoded = params.encode().unwrap();
        assert_eq!(encoded[0x00], 0xAB);
        assert_eq!(encoded[0x4F], 0xCD);
        assert_eq!(encoded[0x09], 0);
        assert_eq!(encoded[0x20], 1);
    }
}
