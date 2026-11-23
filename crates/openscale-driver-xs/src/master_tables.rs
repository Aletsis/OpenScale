use openscale_domain::types::new_id;
use openscale_domain::{
    BarcodeFormat, Department, MediaAsset, StoredText, SyncMetadata, Tare, TaxRate,
};
use openscale_driver::DriverError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::protocol::{decode_cp1252_zstr, encode_cp1252_cstr};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct XsBarcodeFormatDef {
    pub bmc: u8,
    pub bt: u8,
    pub bpd: u8,
    pub subtype: u8,
    pub template: String,
}

pub fn encode_department_write(dept: &Department) -> Result<Vec<u8>, DriverError> {
    let did = u8::try_from(dept.did).map_err(|_| DriverError::Protocol {
        code: None,
        message: format!("XS department DID {} out of range (0..=99)", dept.did),
    })?;
    let mut out = Vec::with_capacity(2 + dept.name.len());
    out.push(did);
    out.extend_from_slice(&encode_cp1252_cstr(&dept.name));
    Ok(out)
}

pub fn decode_department_read(payload: &[u8], store_id: Uuid) -> Result<Department, DriverError> {
    if payload.is_empty() {
        return Err(DriverError::Protocol {
            code: None,
            message: "DEPT_NAMES payload is empty".to_string(),
        });
    }
    Ok(Department {
        id: new_id(),
        did: u32::from(payload[0]),
        name: decode_cp1252_zstr(&payload[1..]),
        meta: SyncMetadata::new(store_id),
    })
}

pub fn encode_tare_read(cid: u16) -> Result<[u8; 5], DriverError> {
    let cid_u8 = u8::try_from(cid).map_err(|_| DriverError::Protocol {
        code: None,
        message: format!("XS tare CID {cid} out of range (0..=39)"),
    })?;
    Ok([0, 0, 0, 0, cid_u8])
}

pub fn encode_tare_write(tare: &Tare) -> Result<[u8; 5], DriverError> {
    let cid_u8 = u8::try_from(tare.cid).map_err(|_| DriverError::Protocol {
        code: None,
        message: format!("XS tare CID {} out of range (0..=39)", tare.cid),
    })?;
    let mut out = [0u8; 5];
    out[0..4].copy_from_slice(&tare.weight_grams.to_be_bytes());
    out[4] = cid_u8;
    Ok(out)
}

pub fn decode_tare_read(payload: &[u8], store_id: Uuid) -> Result<Tare, DriverError> {
    if payload.len() < 5 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("STORED_TARE payload too short: {} bytes", payload.len()),
        });
    }
    let weight_grams = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let cid = u16::from(payload[4]);
    Ok(Tare {
        id: new_id(),
        cid,
        name: format!("Container {cid}"),
        weight_grams,
        meta: SyncMetadata::new(store_id),
    })
}

pub fn encode_tax_read(tax_id: u8) -> [u8; 5] {
    [0, 0, 0, 0, tax_id]
}

pub fn encode_tax_write(tax: &TaxRate) -> [u8; 5] {
    let mut out = [0u8; 5];
    out[0..4].copy_from_slice(&tax.rate_basis_points.to_be_bytes());
    out[4] = tax.tax_id;
    out
}

pub fn decode_tax_read(payload: &[u8], store_id: Uuid) -> Result<TaxRate, DriverError> {
    if payload.len() < 5 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("TAX_RATES payload too short: {} bytes", payload.len()),
        });
    }
    let rate_basis_points = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let tax_id = payload[4];
    Ok(TaxRate {
        id: new_id(),
        tax_id,
        name: format!("Tax {tax_id}"),
        rate_basis_points,
        meta: SyncMetadata::new(store_id),
    })
}

pub fn encode_stored_text_write(text: &StoredText, table: u8) -> Result<Vec<u8>, DriverError> {
    let id = u8::try_from(text.text_id).map_err(|_| DriverError::Protocol {
        code: None,
        message: format!("XS stored text ID {} out of range", text.text_id),
    })?;
    let mut out = Vec::with_capacity(3 + text.content.len());
    out.push(id);
    out.push(table);
    out.extend_from_slice(&encode_cp1252_cstr(&text.content));
    Ok(out)
}

pub fn decode_stored_text_read(payload: &[u8], store_id: Uuid) -> Result<StoredText, DriverError> {
    if payload.is_empty() {
        return Err(DriverError::Protocol {
            code: None,
            message: "STORED_TEXT payload is empty".to_string(),
        });
    }
    let text_id = u32::from(payload[0]);
    Ok(StoredText {
        id: new_id(),
        text_id,
        title: format!("Message {text_id}"),
        content: decode_cp1252_zstr(&payload[1..]),
        meta: SyncMetadata::new(store_id),
    })
}

pub fn encode_barcode_format_write(fmt: &BarcodeFormat) -> Result<Vec<u8>, DriverError> {
    let bid = u8::try_from(fmt.bid).map_err(|_| DriverError::Protocol {
        code: None,
        message: format!("XS barcode BID {} out of range (1..=32)", fmt.bid),
    })?;
    let def: XsBarcodeFormatDef =
        serde_json::from_str(&fmt.definition_json).map_err(|e| DriverError::Protocol {
            code: None,
            message: format!("invalid barcode definition JSON: {e}"),
        })?;
    let mut out = Vec::with_capacity(6 + def.template.len());
    out.push(bid);
    out.push(def.bmc);
    out.push(def.bt);
    out.push(def.bpd);
    out.push(if def.subtype == 0 { 0x03 } else { def.subtype });
    out.extend_from_slice(&encode_cp1252_cstr(&def.template));
    Ok(out)
}

pub fn decode_barcode_format_read(
    payload: &[u8],
    store_id: Uuid,
) -> Result<BarcodeFormat, DriverError> {
    if payload.len() < 6 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("BARCODE_FORMATS payload too short: {} bytes", payload.len()),
        });
    }
    let bid = u16::from(payload[0]);
    let def = XsBarcodeFormatDef {
        bmc: payload[1],
        bt: payload[2],
        bpd: payload[3],
        subtype: payload[4],
        template: decode_cp1252_zstr(&payload[5..]),
    };
    let definition_json = serde_json::to_string(&def).map_err(|e| DriverError::Protocol {
        code: None,
        message: format!("failed to serialize barcode definition: {e}"),
    })?;

    Ok(BarcodeFormat {
        id: new_id(),
        bid,
        name: format!("Barcode {bid}"),
        definition_json,
        meta: SyncMetadata::new(store_id),
    })
}

pub fn decode_media_asset_read(
    media_id: u32,
    payload: &[u8],
    store_id: Uuid,
) -> Result<MediaAsset, DriverError> {
    if payload.len() <= 33 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("MEDIA payload too short: {} bytes", payload.len()),
        });
    }
    let name = decode_cp1252_zstr(&payload[2..32]);
    let data = payload[33..].to_vec();
    Ok(MediaAsset {
        id: new_id(),
        media_id,
        name,
        mime_type: "image/bmp".to_string(),
        data,
        meta: SyncMetadata::new(store_id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_table_codecs_roundtrip() {
        let store_id = new_id();

        // Department
        let dept = Department {
            id: new_id(),
            did: 1,
            name: "CARNICERÍA".to_string(),
            meta: SyncMetadata::new(store_id),
        };
        let dept_bytes = encode_department_write(&dept).unwrap();
        let dept_dec = decode_department_read(&dept_bytes, store_id).unwrap();
        assert_eq!(dept_dec.did, 1);
        assert_eq!(dept_dec.name, "CARNICERÍA");

        // Tare
        let tare = Tare {
            id: new_id(),
            cid: 5,
            name: "Charola".to_string(),
            weight_grams: 25,
            meta: SyncMetadata::new(store_id),
        };
        let tare_bytes = encode_tare_write(&tare).unwrap();
        assert_eq!(tare_bytes, [0, 0, 0, 25, 5]);
        let tare_dec = decode_tare_read(&tare_bytes, store_id).unwrap();
        assert_eq!(tare_dec.cid, 5);
        assert_eq!(tare_dec.weight_grams, 25);

        // Tax Rate (16.00% = 1600)
        let tax = TaxRate {
            id: new_id(),
            tax_id: 1,
            name: "IVA 16%".to_string(),
            rate_basis_points: 1600,
            meta: SyncMetadata::new(store_id),
        };
        let tax_bytes = encode_tax_write(&tax);
        let tax_dec = decode_tax_read(&tax_bytes, store_id).unwrap();
        assert_eq!(tax_dec.tax_id, 1);
        assert_eq!(tax_dec.rate_basis_points, 1600);

        // Barcode Format
        let bf = BarcodeFormat {
            id: new_id(),
            bid: 1,
            name: "EAN-13 Weight".to_string(),
            definition_json: serde_json::to_string(&XsBarcodeFormatDef {
                bmc: 0,
                bt: 3,
                bpd: 1,
                subtype: 3,
                template: "2NNAAAAWWWWWC".to_string(),
            })
            .unwrap(),
            meta: SyncMetadata::new(store_id),
        };
        let bf_bytes = encode_barcode_format_write(&bf).unwrap();
        let bf_dec = decode_barcode_format_read(&bf_bytes, store_id).unwrap();
        assert_eq!(bf_dec.bid, 1);
        let def_dec: XsBarcodeFormatDef = serde_json::from_str(&bf_dec.definition_json).unwrap();
        assert_eq!(def_dec.template, "2NNAAAAWWWWWC");

        // Media BMP
        let mut media_payload = vec![0u8; 40];
        media_payload[2..9].copy_from_slice(b"calimax");
        media_payload[33..35].copy_from_slice(b"BM");
        let media = decode_media_asset_read(0, &media_payload, store_id).unwrap();
        assert_eq!(media.name, "calimax");
        assert_eq!(&media.data[0..2], b"BM");
    }
}
