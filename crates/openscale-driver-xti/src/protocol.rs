use std::collections::BTreeMap;

use openscale_driver::DriverError;
use tokio::io::{AsyncRead, AsyncReadExt};

pub const XTI_DATA_PORT: u16 = 3002;
pub const XTI_DISCOVERY_PORT: u16 = 23453;

/// Maximum single frame / field allocation guard (16 MiB).
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XtiBanner {
    pub raw_xml: String,
    pub fields: BTreeMap<String, String>,
}

impl XtiBanner {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    pub fn requires_auth(&self) -> bool {
        self.get("Authenticate").is_some_and(|v| {
            v.eq_ignore_ascii_case("yes") || v == "1" || v.eq_ignore_ascii_case("true")
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseBatch {
    pub metadata: BTreeMap<String, String>,
    pub records: Vec<BTreeMap<String, Vec<u8>>>,
}

impl ResponseBatch {
    pub fn status(&self) -> &str {
        self.metadata
            .get("STATUS")
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn message(&self) -> Option<&str> {
        self.metadata.get("MESSAGE").map(String::as_str)
    }

    pub fn is_ok(&self) -> bool {
        self.status() == "0"
    }
}

/// Encodes a byte slice as `[uint64 LE len][bytes]`.
pub fn encode_bytes(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u64).to_le_bytes());
    out.extend_from_slice(data);
}

/// Encodes a key/value map as `[uint64 LE numPairs]([len+key][len+val])*`.
pub fn encode_map(out: &mut Vec<u8>, pairs: &[(String, Vec<u8>)]) {
    out.extend_from_slice(&(pairs.len() as u64).to_le_bytes());
    for (key, val) in pairs {
        encode_bytes(out, key.as_bytes());
        encode_bytes(out, val);
    }
}

/// Encodes an `XtraDataPort` instruction (`MxApiOutputStream`):
/// 1. Metadata map (`TABLE_ID`, `ACTION_ID`) WITHOUT outer length prefix
/// 2. Inner payload map WITH `uint64 LE` total byte-length prefix
pub fn encode_instruction(
    table_id: &str,
    action_id: &str,
    payload_fields: &[(String, Vec<u8>)],
) -> Vec<u8> {
    let meta = vec![
        ("TABLE_ID".to_string(), table_id.as_bytes().to_vec()),
        ("ACTION_ID".to_string(), action_id.as_bytes().to_vec()),
    ];

    let mut inner = Vec::new();
    encode_map(&mut inner, payload_fields);

    let mut out = Vec::with_capacity(64 + inner.len());
    encode_map(&mut out, &meta);
    out.extend_from_slice(&(inner.len() as u64).to_le_bytes());
    out.extend_from_slice(&inner);
    out
}

pub async fn read_u64_le<R: AsyncRead + Unpin>(reader: &mut R) -> Result<u64, DriverError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).await?;
    Ok(u64::from_le_bytes(buf))
}

pub async fn read_bytes<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, DriverError> {
    let len = read_u64_le(reader).await? as usize;
    if len > MAX_FRAME_BYTES {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("XTi frame length {len} exceeds limit {MAX_FRAME_BYTES}"),
        });
    }
    let mut buf = vec![0u8; len];
    if len > 0 {
        reader.read_exact(&mut buf).await?;
    }
    Ok(buf)
}

pub async fn read_map<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<BTreeMap<String, Vec<u8>>, DriverError> {
    let count = read_u64_le(reader).await? as usize;
    if count > 10_000 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("XTi map field count {count} exceeds limit"),
        });
    }
    let mut map = BTreeMap::new();
    for _ in 0..count {
        let key_bytes = read_bytes(reader).await?;
        let val_bytes = read_bytes(reader).await?;
        let key = String::from_utf8_lossy(&key_bytes).into_owned();
        map.insert(key, val_bytes);
    }
    Ok(map)
}

/// Reads the initial `[uint64 LE xmlLen][UTF-8 XML]` banner sent immediately by XTi on connect.
pub async fn read_banner<R: AsyncRead + Unpin>(reader: &mut R) -> Result<XtiBanner, DriverError> {
    let raw = read_bytes(reader).await?;
    let xml = String::from_utf8_lossy(&raw).into_owned();
    let fields = parse_xml_fields(&xml);
    Ok(XtiBanner {
        raw_xml: xml,
        fields,
    })
}

/// Reads a `ResponseBatch` (`MxApiInputStream` with `m_bDirectToScale = true`).
pub async fn read_response_batch<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<ResponseBatch, DriverError> {
    let meta_raw = read_map(reader).await?;
    let metadata = meta_raw
        .into_iter()
        .map(|(k, v)| (k, String::from_utf8_lossy(&v).into_owned()))
        .collect();

    let num_records = read_u64_le(reader).await? as usize;
    if num_records > 100_000 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("XTi response record count {num_records} exceeds limit"),
        });
    }

    let mut records = Vec::with_capacity(num_records);
    for _ in 0..num_records {
        records.push(read_map(reader).await?);
    }

    Ok(ResponseBatch { metadata, records })
}

/// Extracts `<Field name="Key">Value</Field>` pairs from XTi XML banners and UDP discovery replies.
pub fn parse_xml_fields(xml: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut rest = xml;

    while let Some(field_start) = rest.find("<Field") {
        rest = &rest[field_start + 6..];
        let Some(tag_end) = rest.find('>') else {
            break;
        };
        let attrs = &rest[..tag_end];
        rest = &rest[tag_end + 1..];

        let Some(close_idx) = rest.find("</Field>") else {
            break;
        };
        let value = unescape_xml(rest[..close_idx].trim());
        rest = &rest[close_idx + 8..];

        if let Some(name) = extract_attr(attrs, "name") {
            map.insert(name, value);
        }
    }

    map
}

fn extract_attr(attrs: &str, attr_name: &str) -> Option<String> {
    let needle = format!("{attr_name}=");
    let pos = attrs.find(&needle)?;
    let after = attrs[pos + needle.len()..].trim_start();
    let quote = after.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = &after[1..];
    let end = inner.find(quote)?;
    Some(inner[..end].to_string())
}

fn unescape_xml(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_empty_instruction_prefixes_8_bytes_for_zero_fields() {
        let bytes = encode_instruction("PING", "READ", &[]);
        // Last 16 bytes: [totalInnerPayloadBytes = 8 (u64 LE)][numFields = 0 (u64 LE)]
        let len = bytes.len();
        assert_eq!(
            u64::from_le_bytes(bytes[len - 16..len - 8].try_into().unwrap()),
            8
        );
        assert_eq!(
            u64::from_le_bytes(bytes[len - 8..len].try_into().unwrap()),
            0
        );
    }

    #[test]
    fn parses_xti_banner_xml_fields() {
        let xml = r#"<Banner>
            <Field name="ScaleType">XT</Field>
            <Field name="ModelNumber">XTi400</Field>
            <Field name="SerialNumber">12345678</Field>
            <Field name="ScaleVersion">5.3.0.0</Field>
            <Field name="FirmwareNumber">ABR30-000341</Field>
            <Field name="MACAddress">00:11:22:33:44:55</Field>
            <Field name="Authenticate">No</Field>
        </Banner>"#;

        let fields = parse_xml_fields(xml);
        let banner = XtiBanner {
            raw_xml: xml.to_string(),
            fields,
        };
        assert_eq!(banner.get("ModelNumber"), Some("XTi400"));
        assert_eq!(banner.get("SerialNumber"), Some("12345678"));
        assert!(!banner.requires_auth());
    }
}
