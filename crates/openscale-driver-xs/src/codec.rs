use bytes::{BufMut, BytesMut};
use openscale_domain::types::new_id;
use openscale_domain::{MoneyCents, Plu, PluKey, ProductType, SyncMetadata};
use openscale_driver::{DriverError, ScaleDateTime};
use uuid::Uuid;

use crate::protocol::{decode_cp1252_cstrs, decode_cp1252_zstr, encode_cp1252_cstr};

pub const SEARCH_HEADER_LEN: usize = 41;
pub const PLU_FIXED_REC_LEN: usize = 83;
pub const PLU_MIN_PAYLOAD_LEN: usize = SEARCH_HEADER_LEN + PLU_FIXED_REC_LEN; // 124 (0x7C)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDetails {
    pub product: String,
    pub version: String,
    pub datetime: String,
    pub firmware: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetInfo {
    pub serial_number: String,
    pub hardware_part: String,
    pub model: String,
    pub region: String,
    pub app_firmware_part: String,
    pub app_version: String,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachNetConfig {
    pub ip_address: String,
    pub subnet_mask: String,
    pub gateway: String,
    pub dns: String,
    pub hostname: String,
    pub mac_address: String,
    pub scale_port: u16,
    pub host_port: u16,
    pub dhcp: bool,
    pub wifi: bool,
    pub net_id: u8,
}

/// Builds the 41-byte search/key header used by `PLU:READ`, `PLU:WRITE`, and `PLU:DELETEONE`.
pub fn encode_plu_search_header(
    from: PluKey,
    to: PluKey,
    ean: u64,
    search_index: u32,
    is_write: bool,
) -> Result<[u8; SEARCH_HEADER_LEN], DriverError> {
    let from_bytes = from.to_xs_bytes()?;
    let to_bytes = to.to_xs_bytes()?;

    let mut buf = BytesMut::with_capacity(SEARCH_HEADER_LEN);
    buf.put_slice(&from_bytes); // 0x00..0x07
    buf.put_slice(&to_bytes); // 0x08..0x0F
    if is_write {
        buf.put_u64(ean); // 0x10..0x17
        buf.put_u64(ean); // 0x18..0x1F
        buf.put_u32(0); // 0x20..0x23
        buf.put_bytes(0, 3); // 0x24..0x26
        buf.put_slice(&[0x00, 0x00]); // 0x27..0x28
    } else {
        buf.put_u64(0); // 0x10..0x17
        buf.put_u64(0); // 0x18..0x1F
        buf.put_u32(search_index); // 0x20..0x23
        buf.put_bytes(0, 3); // 0x24..0x26
        buf.put_slice(&[0x00, 0x63]); // 0x27..0x28
    }

    let mut out = [0u8; SEARCH_HEADER_LEN];
    out.copy_from_slice(&buf);
    Ok(out)
}

pub fn product_type_to_xs_byte(pt: ProductType) -> u8 {
    match pt {
        ProductType::ByWeight => 0,
        ProductType::ByCount => 1,
        ProductType::NegativeByCount => 2,
        ProductType::FixedWeight => 3,
        ProductType::FixedPrice => 4,
        ProductType::ManualWeight => 5,
        ProductType::ByVolume => 6,
    }
}

pub fn xs_byte_to_product_type(byte: u8) -> ProductType {
    match byte {
        1 => ProductType::ByCount,
        2 => ProductType::NegativeByCount,
        3 => ProductType::FixedWeight,
        4 => ProductType::FixedPrice,
        5 => ProductType::ManualWeight,
        6 => ProductType::ByVolume,
        _ => ProductType::ByWeight,
    }
}

/// Encodes a full `PLU:WRITE` payload (41 B header + 83 B fixed record + CP1252 strings).
pub fn encode_plu_write(
    plu: &Plu,
    preserved_internal_flags: Option<[u8; 2]>,
) -> Result<Vec<u8>, DriverError> {
    let ean_num: u64 = plu
        .ean
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u64>())
        .transpose()
        .map_err(|_| DriverError::Protocol {
            code: None,
            message: format!("XS requires numeric EAN, got {:?}", plu.ean),
        })?
        .unwrap_or(0);

    let key_bytes = plu.key.to_xs_bytes()?;
    let search_hdr = encode_plu_search_header(plu.key, plu.key, ean_num, 0, true)?;

    let mut rec = [0u8; PLU_FIXED_REC_LEN];
    rec[0x00..0x08].copy_from_slice(&key_bytes);
    rec[0x0C] = u8::try_from(plu.group_id).unwrap_or(0);
    rec[0x0D..0x11].copy_from_slice(&plu.price_1.as_u32()?.to_be_bytes());
    rec[0x11] = 1; // PMN default = 1
    rec[0x12..0x16].copy_from_slice(&plu.price_2.as_u32()?.to_be_bytes());
    rec[0x16..0x1A].copy_from_slice(&plu.price_3.as_u32()?.to_be_bytes());
    rec[0x1A] = plu.tare_id.and_then(|v| u8::try_from(v).ok()).unwrap_or(0);
    rec[0x1B] = u8::try_from(plu.label_format_id).unwrap_or(0);
    rec[0x1C..0x24].copy_from_slice(&ean_num.to_be_bytes());
    rec[0x26..0x28].copy_from_slice(&plu.date_offset_1.to_be_bytes());
    rec[0x28] = plu.date_print_control_1;
    rec[0x29..0x2B].copy_from_slice(&plu.date_offset_2.to_be_bytes());
    rec[0x2B] = plu.date_print_control_2;
    rec[0x2D] = ((plu.tax_rate_id_1 & 0x0F) << 4) | (plu.tax_rate_id_2 & 0x0F);
    rec[0x2F] = 0x02; // Normal: BPC=0 (print barcode), SOPLU=0, PMD=1
    rec[0x32..0x34].copy_from_slice(&0xFFFFu16.to_be_bytes()); // Trace/Promo Ref default
    rec[0x40] = 1; // TSID default = 1
    rec[0x41] = u8::try_from(plu.barcode_format_id).unwrap_or(0);
    rec[0x46] = product_type_to_xs_byte(plu.product_type);
    if let Some(flags) = preserved_internal_flags {
        rec[0x4F..0x51].copy_from_slice(&flags);
    }
    // rec[0x51..0x53] = 0x00, 0x00 (hash calculated by scale)

    let mut payload = Vec::with_capacity(PLU_MIN_PAYLOAD_LEN + 128);
    payload.extend_from_slice(&search_hdr);
    payload.extend_from_slice(&rec);

    // DYT\0 + GPTA1..GPTA6\0 + final \0
    payload.extend_from_slice(&encode_cp1252_cstr(&plu.description));
    for i in 0..6 {
        let area = plu.text_areas.get(i).map(String::as_str).unwrap_or("");
        payload.extend_from_slice(&encode_cp1252_cstr(area));
    }
    payload.push(0);

    Ok(payload)
}

/// Decodes a `PLU:READ` response payload, returning the domain `Plu` and the 2 preserved
/// internal scale flag bytes (`rec[0x4F..0x51]`).
pub fn decode_plu_read(payload: &[u8], store_id: Uuid) -> Result<(Plu, [u8; 2]), DriverError> {
    if payload.len() < PLU_MIN_PAYLOAD_LEN {
        return Err(DriverError::Protocol {
            code: None,
            message: format!(
                "PLU_RECORD payload too short: {} bytes (min {PLU_MIN_PAYLOAD_LEN})",
                payload.len()
            ),
        });
    }

    let rec = &payload[SEARCH_HEADER_LEN..PLU_MIN_PAYLOAD_LEN];
    let mut key_raw = [0u8; 8];
    key_raw.copy_from_slice(&rec[0x00..0x08]);
    let key = PluKey::from_xs_bytes(key_raw)?;

    let price_1 = MoneyCents(i64::from(u32::from_be_bytes([
        rec[0x0D], rec[0x0E], rec[0x0F], rec[0x10],
    ])));
    let price_2 = MoneyCents(i64::from(u32::from_be_bytes([
        rec[0x12], rec[0x13], rec[0x14], rec[0x15],
    ])));
    let price_3 = MoneyCents(i64::from(u32::from_be_bytes([
        rec[0x16], rec[0x17], rec[0x18], rec[0x19],
    ])));
    let ean_num = u64::from_be_bytes([
        rec[0x1C], rec[0x1D], rec[0x1E], rec[0x1F], rec[0x20], rec[0x21], rec[0x22], rec[0x23],
    ]);
    let ean = if ean_num == 0 {
        None
    } else {
        Some(ean_num.to_string())
    };

    let tax_byte = rec[0x2D];
    let tax_rate_id_1 = (tax_byte >> 4) & 0x0F;
    let tax_rate_id_2 = tax_byte & 0x0F;
    let internal_flags = [rec[0x4F], rec[0x50]];

    let strings = decode_cp1252_cstrs(&payload[PLU_MIN_PAYLOAD_LEN..]);
    let description = strings.first().cloned().unwrap_or_default();
    let mut text_areas = Vec::new();
    for s in strings.iter().skip(1).take(6) {
        if !s.is_empty() {
            text_areas.push(s.clone());
        }
    }

    let mut plu = Plu {
        id: new_id(),
        key,
        description,
        button_text: None,
        price_1,
        price_2,
        price_3,
        product_type: xs_byte_to_product_type(rec[0x46]),
        ean,
        sku: None,
        group_id: u32::from(rec[0x0C]),
        barcode_format_id: u16::from(rec[0x41]),
        label_format_id: u16::from(rec[0x1B]),
        tare_id: Some(u16::from(rec[0x1A])),
        tare_grams: None,
        date_offset_1: u16::from_be_bytes([rec[0x26], rec[0x27]]),
        date_print_control_1: rec[0x28],
        date_offset_2: u16::from_be_bytes([rec[0x29], rec[0x2A]]),
        date_print_control_2: rec[0x2B],
        tax_rate_id_1,
        tax_rate_id_2,
        text_areas,
        delta_hash: String::new(),
        meta: SyncMetadata::new(store_id),
    };
    plu.delta_hash = plu.compute_delta_hash();

    Ok((plu, internal_flags))
}

/// Encodes the 28-byte `CHANGE_PRICE` (`CMD=5, CVN=3400`) payload.
pub fn encode_change_price(
    key: PluKey,
    price_1: MoneyCents,
    price_2: MoneyCents,
    price_3: MoneyCents,
) -> Result<[u8; 28], DriverError> {
    let key_bytes = key.to_xs_bytes()?;
    let mut out = [0u8; 28];
    out[0..8].copy_from_slice(&key_bytes);
    out[8..16].copy_from_slice(&key_bytes);
    out[16..20].copy_from_slice(&price_1.as_u32()?.to_be_bytes());
    out[20..24].copy_from_slice(&price_2.as_u32()?.to_be_bytes());
    out[24..28].copy_from_slice(&price_3.as_u32()?.to_be_bytes());
    Ok(out)
}

pub fn decode_version_details(payload: &[u8]) -> VersionDetails {
    let parts = decode_cp1252_cstrs(payload);
    VersionDetails {
        product: parts.first().cloned().unwrap_or_default(),
        version: parts.get(1).cloned().unwrap_or_default(),
        datetime: parts.get(2).cloned().unwrap_or_default(),
        firmware: parts.get(3).cloned().unwrap_or_default(),
    }
}

pub fn decode_asset_info(payload: &[u8]) -> Result<AssetInfo, DriverError> {
    if payload.len() < 0x133 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("ASSETINFO payload too short: {} bytes", payload.len()),
        });
    }
    let model_suffix = decode_cp1252_zstr(&payload[0x69..0x73]);
    let model = if model_suffix.is_empty() {
        "XS".to_string()
    } else if model_suffix.starts_with("XS") {
        model_suffix
    } else {
        format!("XS{model_suffix}")
    };

    Ok(AssetInfo {
        serial_number: decode_cp1252_zstr(&payload[0x33..0x4B]),
        hardware_part: decode_cp1252_zstr(&payload[0x54..0x64]),
        model,
        region: decode_cp1252_zstr(&payload[0x88..0x96]),
        app_firmware_part: decode_cp1252_zstr(&payload[0x9D..0xBA]),
        app_version: decode_cp1252_zstr(&payload[0xBC..0xCC]),
        protocol: decode_cp1252_zstr(&payload[0x125..0x133]),
    })
}

pub fn decode_mach_net_config(payload: &[u8]) -> Result<MachNetConfig, DriverError> {
    if payload.len() < 45 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("MACHNETCONFIG payload too short: {} bytes", payload.len()),
        });
    }
    let fmt_ip = |s: &[u8]| format!("{}.{}.{}.{}", s[0], s[1], s[2], s[3]);
    let mac = format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        payload[0x20], payload[0x21], payload[0x22], payload[0x23], payload[0x24], payload[0x25]
    );

    Ok(MachNetConfig {
        ip_address: fmt_ip(&payload[0x00..0x04]),
        subnet_mask: fmt_ip(&payload[0x04..0x08]),
        gateway: fmt_ip(&payload[0x08..0x0C]),
        dns: fmt_ip(&payload[0x0C..0x10]),
        hostname: decode_cp1252_zstr(&payload[0x10..0x20]),
        mac_address: mac,
        scale_port: u16::from_be_bytes([payload[0x26], payload[0x27]]),
        host_port: u16::from_be_bytes([payload[0x28], payload[0x29]]),
        dhcp: payload[0x2A] != 0,
        wifi: payload[0x2B] != 0,
        net_id: payload[0x2C],
    })
}

pub fn decode_datetime(payload: &[u8]) -> Result<ScaleDateTime, DriverError> {
    if payload.len() < 12 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("DATE_AND_TIME payload too short: {} bytes", payload.len()),
        });
    }
    Ok(ScaleDateTime {
        day: u16::from_be_bytes([payload[0], payload[1]]) as u8,
        month: u16::from_be_bytes([payload[2], payload[3]]) as u8,
        year: u16::from_be_bytes([payload[4], payload[5]]),
        hour: u16::from_be_bytes([payload[6], payload[7]]) as u8,
        minute: u16::from_be_bytes([payload[8], payload[9]]) as u8,
        second: u16::from_be_bytes([payload[10], payload[11]]) as u8,
    })
}

pub fn encode_datetime(dt: ScaleDateTime) -> [u8; 13] {
    let mut out = [0u8; 13];
    out[0..2].copy_from_slice(&u16::from(dt.day).to_be_bytes());
    out[2..4].copy_from_slice(&u16::from(dt.month).to_be_bytes());
    out[4..6].copy_from_slice(&dt.year.to_be_bytes());
    out[6..8].copy_from_slice(&u16::from(dt.hour).to_be_bytes());
    out[8..10].copy_from_slice(&u16::from(dt.minute).to_be_bytes());
    out[10..12].copy_from_slice(&u16::from(dt.second).to_be_bytes());
    out[12] = 0x00;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plu_encode_decode_preserves_fields_and_internal_flags() {
        let store_id = new_id();
        let plu = Plu {
            id: new_id(),
            key: PluKey::new(1, 77).unwrap(),
            description: "ALITA DE POLLO ADOBADA".to_string(),
            button_text: None,
            price_1: MoneyCents(7000),
            price_2: MoneyCents(6500),
            price_3: MoneyCents(6000),
            product_type: ProductType::ByWeight,
            ean: Some("2000077000000".to_string()),
            sku: None,
            group_id: 3,
            barcode_format_id: 1,
            label_format_id: 2,
            tare_id: Some(4),
            tare_grams: None,
            date_offset_1: 7,
            date_print_control_1: 1,
            date_offset_2: 14,
            date_print_control_2: 2,
            tax_rate_id_1: 1,
            tax_rate_id_2: 2,
            text_areas: vec!["Pollo marinado".to_string(), "Mantener en frío".to_string()],
            delta_hash: String::new(),
            meta: SyncMetadata::new(store_id),
        };

        let payload = encode_plu_write(&plu, Some([0x12, 0x34])).unwrap();
        let (decoded, flags) = decode_plu_read(&payload, store_id).unwrap();

        assert_eq!(decoded.key, plu.key);
        assert_eq!(decoded.description, "ALITA DE POLLO ADOBADA");
        assert_eq!(decoded.price_1, MoneyCents(7000));
        assert_eq!(decoded.price_2, MoneyCents(6500));
        assert_eq!(decoded.price_3, MoneyCents(6000));
        assert_eq!(decoded.ean.as_deref(), Some("2000077000000"));
        assert_eq!(decoded.tax_rate_id_1, 1);
        assert_eq!(decoded.tax_rate_id_2, 2);
        assert_eq!(decoded.text_areas, plu.text_areas);
        assert_eq!(flags, [0x12, 0x34]);
    }

    #[test]
    fn change_price_payload_matches_28_byte_spec() {
        let key = PluKey::new(1, 500).unwrap();
        let bytes =
            encode_change_price(key, MoneyCents(7500), MoneyCents(7000), MoneyCents(0)).unwrap();
        assert_eq!(bytes.len(), 28);
        assert_eq!(&bytes[0..8], &bytes[8..16]);
        assert_eq!(
            u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            7500
        );
        assert_eq!(
            u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
            7000
        );
        assert_eq!(
            u32::from_be_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            0
        );
    }

    #[test]
    fn datetime_roundtrip() {
        let dt = ScaleDateTime {
            year: 2026,
            month: 11,
            day: 22,
            hour: 15,
            minute: 11,
            second: 52,
        };
        let encoded = encode_datetime(dt);
        assert_eq!(encoded[12], 0x00);
        assert_eq!(decode_datetime(&encoded).unwrap(), dt);
    }

    #[test]
    fn mach_net_config_and_asset_info_decode() {
        let mut net = vec![0u8; 48];
        net[0..4].copy_from_slice(&[192, 168, 9, 113]);
        net[4..8].copy_from_slice(&[255, 255, 255, 0]);
        net[0x20..0x26].copy_from_slice(&[0x00, 0xE0, 0x8A, 0x0D, 0x72, 0xAF]);
        net[0x26..0x28].copy_from_slice(&25000u16.to_be_bytes());
        net[0x28..0x2A].copy_from_slice(&3001u16.to_be_bytes());
        let parsed = decode_mach_net_config(&net).unwrap();
        assert_eq!(parsed.ip_address, "192.168.9.113");
        assert_eq!(parsed.mac_address, "00:E0:8A:0D:72:AF");
        assert_eq!(parsed.host_port, 3001);

        let mut asset = vec![0u8; 316];
        asset[0x33..0x3B].copy_from_slice(b"22320310");
        asset[0x69..0x6C].copy_from_slice(b"200");
        asset[0xBC..0xC9].copy_from_slice(b"APP. v4.1.7.0");
        asset[0x125..0x132].copy_from_slice(b"PROTOCOL 8000");
        let info = decode_asset_info(&asset).unwrap();
        assert_eq!(info.serial_number, "22320310");
        assert_eq!(info.model, "XS200");
        assert_eq!(info.app_version, "APP. v4.1.7.0");
        assert_eq!(info.protocol, "PROTOCOL 8000");
    }
}
