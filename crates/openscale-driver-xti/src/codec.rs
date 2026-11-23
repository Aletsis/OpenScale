use std::collections::BTreeMap;

use openscale_domain::types::new_id;
use openscale_domain::{MoneyCents, Plu, PluKey, ProductType, SyncMetadata};
use openscale_driver::DriverError;
use uuid::Uuid;

pub fn product_type_to_xti_flag(pt: ProductType) -> &'static str {
    match pt {
        ProductType::ByCount | ProductType::NegativeByCount => "1",
        ProductType::FixedWeight => "4",
        ProductType::ByVolume => "6",
        ProductType::ByWeight | ProductType::FixedPrice | ProductType::ManualWeight => "0",
    }
}

pub fn xti_flag_to_product_type(flag: &str) -> ProductType {
    match flag.trim() {
        "1" => ProductType::ByCount,
        "4" => ProductType::FixedWeight,
        "6" => ProductType::ByVolume,
        _ => ProductType::ByWeight,
    }
}

/// Converts a domain `Plu` into `XtraDataPort` `PLU:WRITE` key/value fields.
pub fn plu_to_xti_fields(plu: &Plu) -> Result<Vec<(String, Vec<u8>)>, DriverError> {
    if plu.key.did == 0 {
        return Err(DriverError::Protocol {
            code: None,
            message: "XTi requires DID >= 1 (department 0 is not supported on XTRA)".to_string(),
        });
    }

    let mut fields: Vec<(String, Vec<u8>)> = vec![
        ("DID".to_string(), plu.key.did.to_string().into_bytes()),
        ("PLU".to_string(), plu.key.plu.to_string().into_bytes()),
        ("DYT".to_string(), plu.description.as_bytes().to_vec()),
        (
            "P1".to_string(),
            plu.price_1.as_i64().to_string().into_bytes(),
        ),
        (
            "P2FS".to_string(),
            plu.price_2.as_i64().to_string().into_bytes(),
        ),
        (
            "PT".to_string(),
            product_type_to_xti_flag(plu.product_type)
                .as_bytes()
                .to_vec(),
        ),
        (
            "BID".to_string(),
            plu.barcode_format_id.to_string().into_bytes(),
        ),
        (
            "PFID".to_string(),
            plu.label_format_id.to_string().into_bytes(),
        ),
        (
            "DO1".to_string(),
            plu.date_offset_1.to_string().into_bytes(),
        ),
        (
            "DPC1".to_string(),
            plu.date_print_control_1.to_string().into_bytes(),
        ),
        (
            "DO2".to_string(),
            plu.date_offset_2.to_string().into_bytes(),
        ),
        (
            "DPC2".to_string(),
            plu.date_print_control_2.to_string().into_bytes(),
        ),
        ("LGID".to_string(), plu.group_id.to_string().into_bytes()),
        (
            "TXID1".to_string(),
            plu.tax_rate_id_1.to_string().into_bytes(),
        ),
        (
            "TXID2".to_string(),
            plu.tax_rate_id_2.to_string().into_bytes(),
        ),
    ];

    if let Some(ref btn) = plu.button_text {
        fields.push(("DBT".to_string(), btn.as_bytes().to_vec()));
    }
    if let Some(ref ean) = plu.ean {
        fields.push(("EAN".to_string(), ean.as_bytes().to_vec()));
    }
    if let Some(ref sku) = plu.sku {
        fields.push(("SKU".to_string(), sku.as_bytes().to_vec()));
    }
    if let Some(ctv) = plu.tare_grams {
        fields.push(("CTV".to_string(), ctv.to_string().into_bytes()));
    }
    for (idx, area) in plu.text_areas.iter().take(12).enumerate() {
        fields.push((format!("GPTA{}", idx + 1), area.as_bytes().to_vec()));
    }

    Ok(fields)
}

/// Converts an `XtraDataPort` `ResponseBatch` PLU record into a domain `Plu`.
pub fn xti_record_to_plu(
    record: &BTreeMap<String, Vec<u8>>,
    store_id: Uuid,
) -> Result<Plu, DriverError> {
    let get_str = |k: &str| {
        record
            .get(k)
            .map(|b| String::from_utf8_lossy(b).trim().to_string())
    };
    let get_u64 = |k: &str| get_str(k).and_then(|s| s.parse::<u64>().ok());
    let get_i64 = |k: &str| get_str(k).and_then(|s| s.parse::<i64>().ok());

    let did = get_u64("DID").ok_or_else(|| DriverError::Protocol {
        code: None,
        message: "XTi PLU record missing DID".to_string(),
    })? as u32;
    let plu_num = get_u64("PLU").ok_or_else(|| DriverError::Protocol {
        code: None,
        message: "XTi PLU record missing PLU".to_string(),
    })?;

    let mut text_areas = Vec::new();
    for idx in 1..=12 {
        if let Some(val) = get_str(&format!("GPTA{idx}"))
            && !val.is_empty()
        {
            text_areas.push(val);
        }
    }

    let mut plu = Plu {
        id: new_id(),
        key: PluKey::new(did, plu_num)?,
        description: get_str("DYT").unwrap_or_default(),
        button_text: get_str("DBT").filter(|s| !s.is_empty()),
        price_1: MoneyCents(get_i64("P1").unwrap_or(0)),
        price_2: MoneyCents(get_i64("P2FS").unwrap_or(0)),
        price_3: MoneyCents(0),
        product_type: xti_flag_to_product_type(&get_str("PT").unwrap_or_default()),
        ean: get_str("EAN").filter(|s| !s.is_empty()),
        sku: get_str("SKU").filter(|s| !s.is_empty()),
        group_id: get_u64("LGID").unwrap_or(0) as u32,
        barcode_format_id: get_u64("BID").unwrap_or(0) as u16,
        label_format_id: get_u64("PFID").unwrap_or(0) as u16,
        tare_id: None,
        tare_grams: get_u64("CTV").map(|v| v as u32),
        date_offset_1: get_u64("DO1").unwrap_or(0) as u16,
        date_print_control_1: get_u64("DPC1").unwrap_or(0) as u8,
        date_offset_2: get_u64("DO2").unwrap_or(0) as u16,
        date_print_control_2: get_u64("DPC2").unwrap_or(0) as u8,
        tax_rate_id_1: get_u64("TXID1").unwrap_or(0) as u8,
        tax_rate_id_2: get_u64("TXID2").unwrap_or(0) as u8,
        text_areas,
        delta_hash: String::new(),
        meta: SyncMetadata::new(store_id),
    };
    plu.delta_hash = plu.compute_delta_hash();
    Ok(plu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xti_plu_roundtrip_preserves_ctv_dbt_sku_and_text_areas() {
        let store_id = new_id();
        let plu = Plu {
            id: new_id(),
            key: PluKey::new(1, 100).unwrap(),
            description: "POLLO FRESCO".to_string(),
            button_text: Some("Pollo".to_string()),
            price_1: MoneyCents(4500),
            price_2: MoneyCents(4200),
            price_3: MoneyCents(0),
            product_type: ProductType::ByWeight,
            ean: Some("2000100000000".to_string()),
            sku: Some("SKU-100".to_string()),
            group_id: 2,
            barcode_format_id: 1,
            label_format_id: 5,
            tare_id: None,
            tare_grams: Some(25),
            date_offset_1: 7,
            date_print_control_1: 1,
            date_offset_2: 0,
            date_print_control_2: 0,
            tax_rate_id_1: 1,
            tax_rate_id_2: 0,
            text_areas: vec!["Ingredientes: pollo".to_string(), "Sin gluten".to_string()],
            delta_hash: String::new(),
            meta: SyncMetadata::new(store_id),
        };

        let fields = plu_to_xti_fields(&plu).unwrap();
        let map: BTreeMap<String, Vec<u8>> = fields.into_iter().collect();
        let decoded = xti_record_to_plu(&map, store_id).unwrap();

        assert_eq!(decoded.key, plu.key);
        assert_eq!(decoded.description, "POLLO FRESCO");
        assert_eq!(decoded.button_text.as_deref(), Some("Pollo"));
        assert_eq!(decoded.price_1, MoneyCents(4500));
        assert_eq!(decoded.price_2, MoneyCents(4200));
        assert_eq!(decoded.tare_grams, Some(25));
        assert_eq!(decoded.sku.as_deref(), Some("SKU-100"));
        assert_eq!(decoded.text_areas, plu.text_areas);
    }

    #[test]
    fn xti_rejects_department_zero() {
        let store_id = new_id();
        let plu = Plu {
            id: new_id(),
            key: PluKey::new(0, 1).unwrap(),
            description: "INVALID".to_string(),
            button_text: None,
            price_1: MoneyCents(100),
            price_2: MoneyCents(0),
            price_3: MoneyCents(0),
            product_type: ProductType::ByWeight,
            ean: None,
            sku: None,
            group_id: 0,
            barcode_format_id: 0,
            label_format_id: 0,
            tare_id: None,
            tare_grams: None,
            date_offset_1: 0,
            date_print_control_1: 0,
            date_offset_2: 0,
            date_print_control_2: 0,
            tax_rate_id_1: 0,
            tax_rate_id_2: 0,
            text_areas: vec![],
            delta_hash: String::new(),
            meta: SyncMetadata::new(store_id),
        };
        assert!(plu_to_xti_fields(&plu).is_err());
    }
}
