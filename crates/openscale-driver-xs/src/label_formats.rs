use openscale_driver::DriverError;
use serde::{Deserialize, Serialize};

use crate::protocol::{decode_cp1252_zstr, encode_cp1252_cstr};

pub const LABEL_FORMAT_LEN: usize = 898;
pub const LABEL_FIELD_SLOTS: usize = 50;
pub const LABEL_FIELD_LEN: usize = 17;
pub const LABEL_FIELDS_OFFSET: usize = 0x2D;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelField {
    pub slot: u8,
    pub field_type: u8,
    pub orientation: u8,
    pub style: u8,
    pub x1: u16,
    pub y1: u16,
    pub x2: u16,
    pub y2: u16,
    pub font: char,
    pub dsr: u8,
    /// Trailing 4 bytes (`+13..+16`) of each 17-byte slot preserved for lossless roundtrip.
    #[serde(default)]
    pub extra: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelFormatRecord {
    pub pfid: u8,
    pub orientation: u8,
    pub active: bool,
    pub width_hundredths_mm: u16,
    pub name: String,
    pub fields: Vec<LabelField>,
    /// Preserved 898-byte binary image for lossless backup & cloning.
    pub raw: Vec<u8>,
}

impl LabelFormatRecord {
    pub fn decode(bytes: &[u8]) -> Result<Self, DriverError> {
        if bytes.len() < LABEL_FORMAT_LEN {
            return Err(DriverError::Protocol {
                code: None,
                message: format!(
                    "LABEL_FORMATS payload too short: {} bytes (expected {LABEL_FORMAT_LEN})",
                    bytes.len()
                ),
            });
        }
        let raw = bytes[..LABEL_FORMAT_LEN].to_vec();
        let pfid = raw[0x00];
        let orientation = raw[0x01];
        let active = raw[0x02] != 0;
        let width_hundredths_mm = u16::from_be_bytes([raw[0x05], raw[0x06]]);
        let name = decode_cp1252_zstr(&raw[0x08..0x24]);

        let mut fields = Vec::new();
        for slot in 0..LABEL_FIELD_SLOTS {
            let base = LABEL_FIELDS_OFFSET + slot * LABEL_FIELD_LEN;
            let ft = raw[base];
            if ft == 0 {
                continue;
            }
            fields.push(LabelField {
                slot: slot as u8,
                field_type: ft,
                orientation: raw[base + 1],
                style: raw[base + 2],
                x1: u16::from_be_bytes([raw[base + 3], raw[base + 4]]),
                y1: u16::from_be_bytes([raw[base + 5], raw[base + 6]]),
                x2: u16::from_be_bytes([raw[base + 7], raw[base + 8]]),
                y2: u16::from_be_bytes([raw[base + 9], raw[base + 10]]),
                font: char::from(raw[base + 11]),
                dsr: raw[base + 12],
                extra: [
                    raw[base + 13],
                    raw[base + 14],
                    raw[base + 15],
                    raw[base + 16],
                ],
            });
        }

        Ok(Self {
            pfid,
            orientation,
            active,
            width_hundredths_mm,
            name,
            fields,
            raw,
        })
    }

    /// Encodes the 898-byte `LABEL_FORMATS` write buffer with checksum bytes `896..898` zeroed
    /// so the XS scale recalculates them automatically.
    pub fn encode(&self) -> [u8; LABEL_FORMAT_LEN] {
        let mut out = [0u8; LABEL_FORMAT_LEN];
        if self.raw.len() >= LABEL_FORMAT_LEN {
            out.copy_from_slice(&self.raw[..LABEL_FORMAT_LEN]);
        }
        out[0x00] = self.pfid;
        out[0x01] = self.orientation;
        out[0x02] = u8::from(self.active);
        out[0x05..0x07].copy_from_slice(&self.width_hundredths_mm.to_be_bytes());

        // Write 28-byte CP1252 name slot
        out[0x08..0x24].fill(0);
        let name_bytes = encode_cp1252_cstr(&self.name);
        let copy_len = name_bytes.len().min(28);
        out[0x08..0x08 + copy_len].copy_from_slice(&name_bytes[..copy_len]);

        // Clear and re-encode 50 field slots
        out[LABEL_FIELDS_OFFSET..LABEL_FIELDS_OFFSET + LABEL_FIELD_SLOTS * LABEL_FIELD_LEN].fill(0);
        for field in &self.fields {
            let slot = usize::from(field.slot);
            if slot >= LABEL_FIELD_SLOTS {
                continue;
            }
            let base = LABEL_FIELDS_OFFSET + slot * LABEL_FIELD_LEN;
            out[base] = field.field_type;
            out[base + 1] = field.orientation;
            out[base + 2] = field.style;
            out[base + 3..base + 5].copy_from_slice(&field.x1.to_be_bytes());
            out[base + 5..base + 7].copy_from_slice(&field.y1.to_be_bytes());
            out[base + 7..base + 9].copy_from_slice(&field.x2.to_be_bytes());
            out[base + 9..base + 11].copy_from_slice(&field.y2.to_be_bytes());
            out[base + 11] = field.font as u8;
            out[base + 12] = field.dsr;
            out[base + 13..base + 17].copy_from_slice(&field.extra);
        }

        // Scale recalculates final 2-byte checksum when zeroed
        out[896] = 0;
        out[897] = 0;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_format_898_bytes_roundtrip_and_zeroes_checksum() {
        let mut raw = [0u8; LABEL_FORMAT_LEN];
        raw[0x00] = 1;
        raw[0x01] = 0;
        raw[0x02] = 1;
        raw[0x05..0x07].copy_from_slice(&5900u16.to_be_bytes());
        raw[0x08..0x14].copy_from_slice(b"STD 59x40mm\0");
        let base = LABEL_FIELDS_OFFSET;
        raw[base] = 10; // Barcode field
        raw[base + 3..base + 5].copy_from_slice(&50u16.to_be_bytes());
        raw[base + 5..base + 7].copy_from_slice(&200u16.to_be_bytes());
        raw[base + 7..base + 9].copy_from_slice(&500u16.to_be_bytes());
        raw[base + 9..base + 11].copy_from_slice(&350u16.to_be_bytes());
        raw[base + 11] = b'A';
        raw[base + 12] = 1;
        raw[896] = 0xFE;
        raw[897] = 0xED;

        let mut decoded = LabelFormatRecord::decode(&raw).unwrap();
        assert_eq!(decoded.pfid, 1);
        assert!(decoded.active);
        assert_eq!(decoded.width_hundredths_mm, 5900);
        assert_eq!(decoded.name, "STD 59x40mm");
        assert_eq!(decoded.fields.len(), 1);
        assert_eq!(decoded.fields[0].field_type, 10);
        assert_eq!(decoded.fields[0].font, 'A');

        decoded.pfid = 98; // Clone to PFID 98
        let encoded = decoded.encode();
        assert_eq!(encoded.len(), 898);
        assert_eq!(encoded[0x00], 98);
        assert_eq!(&encoded[896..898], &[0, 0]);
    }
}
