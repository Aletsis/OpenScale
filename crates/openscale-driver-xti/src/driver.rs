use std::time::Duration;

use async_trait::async_trait;
use openscale_domain::types::{XTI_MAX_DID, XTI_MAX_PLU_NUMBER};
use openscale_domain::{MoneyCents, Plu, PluKey, ScaleFamily};
use openscale_driver::{DriverCapabilities, DriverError, ScaleDateTime, ScaleDriver, ScaleInfo};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::codec::{plu_to_xti_fields, xti_record_to_plu};
use crate::protocol::{
    ResponseBatch, XTI_DATA_PORT, XtiBanner, encode_instruction, read_banner, read_response_batch,
};

#[derive(Debug, Clone)]
pub struct XtiDriverConfig {
    pub host: String,
    pub port: u16,
    pub io_timeout: Duration,
    pub store_id: Uuid,
}

impl XtiDriverConfig {
    pub fn new(host: impl Into<String>, store_id: Uuid) -> Self {
        Self {
            host: host.into(),
            port: XTI_DATA_PORT,
            io_timeout: Duration::from_secs(5),
            store_id,
        }
    }
}

/// Persistent TCP driver for Avery Berkel XTi / XTRA scales (`XtraDataPort`).
pub struct XtiDriver {
    config: XtiDriverConfig,
    stream: Option<TcpStream>,
    banner: Option<XtiBanner>,
}

impl XtiDriver {
    pub fn new(config: XtiDriverConfig) -> Self {
        Self {
            config,
            stream: None,
            banner: None,
        }
    }

    pub fn banner(&self) -> Option<&XtiBanner> {
        self.banner.as_ref()
    }

    async fn send_instruction(
        &mut self,
        table_id: &str,
        action_id: &str,
        fields: &[(String, Vec<u8>)],
    ) -> Result<ResponseBatch, DriverError> {
        let payload = encode_instruction(table_id, action_id, fields);
        let timeout = self.config.io_timeout;
        let stream = self.stream.as_mut().ok_or(DriverError::NotConnected)?;

        tokio::time::timeout(timeout, stream.write_all(&payload))
            .await
            .map_err(|_| DriverError::Timeout)??;
        tokio::time::timeout(timeout, stream.flush())
            .await
            .map_err(|_| DriverError::Timeout)??;

        let batch = tokio::time::timeout(timeout, read_response_batch(stream))
            .await
            .map_err(|_| DriverError::Timeout)??;

        if !batch.is_ok() {
            return Err(DriverError::Protocol {
                code: batch.status().parse::<i32>().ok(),
                message: batch.message().map(ToOwned::to_owned).unwrap_or_else(|| {
                    format!(
                        "{table_id}:{action_id} failed with STATUS={}",
                        batch.status()
                    )
                }),
            });
        }

        Ok(batch)
    }

    /// Lists PLUs with an optional `SQLW` WHERE clause filter (XTi native capability).
    pub async fn list_plus_filtered(
        &mut self,
        from: Option<PluKey>,
        to: Option<PluKey>,
        sql_where: Option<&str>,
    ) -> Result<Vec<Plu>, DriverError> {
        let from_key = from.unwrap_or(PluKey::new(1, 1)?);
        let to_key = to.unwrap_or(PluKey::new(XTI_MAX_DID, XTI_MAX_PLU_NUMBER)?);

        let mut fields = vec![
            ("F1".to_string(), from_key.did.to_string().into_bytes()),
            ("F2".to_string(), from_key.plu.to_string().into_bytes()),
            ("T1".to_string(), to_key.did.to_string().into_bytes()),
            ("T2".to_string(), to_key.plu.to_string().into_bytes()),
        ];
        if let Some(where_clause) = sql_where {
            fields.push(("SQLW".to_string(), where_clause.as_bytes().to_vec()));
        }

        let batch = self.send_instruction("PLU", "READ", &fields).await?;
        batch
            .records
            .iter()
            .map(|rec| xti_record_to_plu(rec, self.config.store_id))
            .collect()
    }
}

#[async_trait]
impl ScaleDriver for XtiDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities::xti_default()
    }

    async fn connect(&mut self) -> Result<ScaleInfo, DriverError> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let mut stream = tokio::time::timeout(self.config.io_timeout, TcpStream::connect(&addr))
            .await
            .map_err(|_| DriverError::Timeout)?
            .map_err(|e| DriverError::ConnectionFailed(format!("{addr}: {e}")))?;

        // XTi sends its XML Banner immediately upon TCP connection
        let banner = tokio::time::timeout(self.config.io_timeout, read_banner(&mut stream))
            .await
            .map_err(|_| DriverError::Timeout)??;

        self.stream = Some(stream);
        self.banner = Some(banner);
        self.get_info().await
    }

    async fn disconnect(&mut self) -> Result<(), DriverError> {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.shutdown().await;
        }
        self.banner = None;
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DriverError> {
        // Real XTi firmware (e.g. 5.1.4.270) executes TABLE_ID directly against its internal
        // SQLite DB and does not have a "PING" table. A zero-row PLU query with SQLW="1=0"
        // verifies live DB + TCP roundtrip without transferring any PLU records.
        let fields = vec![("SQLW".to_string(), b"1=0".to_vec())];
        let _ = self.send_instruction("PLU", "READ", &fields).await?;
        Ok(())
    }

    async fn get_info(&mut self) -> Result<ScaleInfo, DriverError> {
        let banner = self.banner.as_ref().ok_or(DriverError::NotConnected)?;
        let model_prefix = banner
            .get("Model")
            .or_else(|| banner.get("ScaleType"))
            .unwrap_or("XTi");
        let model = match banner.get("ModelNumber") {
            Some(m) if !m.is_empty() && m.chars().all(|c| c.is_ascii_digit()) => {
                format!("{model_prefix}{m}")
            }
            Some(m) if !m.is_empty() => m.to_string(),
            _ => model_prefix.to_string(),
        };
        Ok(ScaleInfo {
            family: ScaleFamily::AveryBerkelXti,
            model,
            serial_number: banner.get("SerialNumber").map(ToOwned::to_owned),
            firmware_version: banner.get("ScaleVersion").unwrap_or_default().to_string(),
            protocol_version: banner.get("Schema Number").map(ToOwned::to_owned),
            mac_address: banner.get("MACAddress").map(ToOwned::to_owned),
            requires_auth: banner.requires_auth(),
        })
    }

    async fn get_datetime(&mut self) -> Result<ScaleDateTime, DriverError> {
        Err(DriverError::Unsupported(
            "get_datetime on XTi pending hardware validation",
        ))
    }

    async fn set_datetime(&mut self, _dt: ScaleDateTime) -> Result<(), DriverError> {
        Err(DriverError::Unsupported(
            "set_datetime on XTi pending hardware validation",
        ))
    }

    async fn read_plu(&mut self, key: PluKey) -> Result<Option<Plu>, DriverError> {
        let fields = vec![
            ("F1".to_string(), key.did.to_string().into_bytes()),
            ("F2".to_string(), key.plu.to_string().into_bytes()),
        ];
        let batch = self.send_instruction("PLU", "READONE", &fields).await?;
        match batch.records.first() {
            Some(rec) => Ok(Some(xti_record_to_plu(rec, self.config.store_id)?)),
            None => Ok(None),
        }
    }

    async fn list_plus(
        &mut self,
        from: Option<PluKey>,
        to: Option<PluKey>,
    ) -> Result<Vec<Plu>, DriverError> {
        self.list_plus_filtered(from, to, None).await
    }

    async fn count_plus(&mut self) -> Result<u64, DriverError> {
        let all = self.list_plus(None, None).await?;
        Ok(all.len() as u64)
    }

    async fn upsert_plu(&mut self, plu: &Plu) -> Result<(), DriverError> {
        let fields = plu_to_xti_fields(plu)?;
        let _ = self.send_instruction("PLU", "WRITE", &fields).await?;
        Ok(())
    }

    /// Safe XTi price change: `READONE` -> patch `P1` / `P2FS` -> `PLU:WRITE`.
    async fn change_price(
        &mut self,
        key: PluKey,
        price_1: MoneyCents,
        price_2: Option<MoneyCents>,
        _price_3: Option<MoneyCents>,
    ) -> Result<(), DriverError> {
        let mut plu = self.read_plu(key).await?.ok_or(DriverError::NotFound)?;
        plu.price_1 = price_1;
        if let Some(p2) = price_2 {
            plu.price_2 = p2;
        }
        self.upsert_plu(&plu).await
    }

    async fn delete_plu(&mut self, key: PluKey) -> Result<(), DriverError> {
        let fields = vec![
            ("F1".to_string(), key.did.to_string().into_bytes()),
            ("F2".to_string(), key.plu.to_string().into_bytes()),
        ];
        let _ = self.send_instruction("PLU", "DELETEONE", &fields).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{encode_bytes, encode_map, read_bytes, read_map};
    use openscale_domain::types::new_id;
    use openscale_domain::{ProductType, SyncMetadata};
    use std::collections::BTreeMap;
    use tokio::net::TcpListener;

    type FieldPairs = Vec<(String, Vec<u8>)>;

    async fn send_response(socket: &mut TcpStream, status: &str, records: &[FieldPairs]) {
        let mut out = Vec::new();
        let meta = vec![("STATUS".to_string(), status.as_bytes().to_vec())];
        encode_map(&mut out, &meta);
        out.extend_from_slice(&(records.len() as u64).to_le_bytes());
        for rec in records {
            encode_map(&mut out, rec);
        }
        socket.write_all(&out).await.unwrap();
    }

    async fn spawn_mock_xti_server() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();

            // Send Banner XML immediately upon accept
            let banner = r#"<Banner>
                <Field name="ScaleType">XT</Field>
                <Field name="ModelNumber">XTi400</Field>
                <Field name="SerialNumber">12345678</Field>
                <Field name="ScaleVersion">5.3.0.0</Field>
                <Field name="Schema Number">5300</Field>
                <Field name="MACAddress">00:11:22:33:44:55</Field>
                <Field name="Authenticate">No</Field>
            </Banner>"#;
            let mut banner_buf = Vec::new();
            encode_bytes(&mut banner_buf, banner.as_bytes());
            socket.write_all(&banner_buf).await.unwrap();

            let mut store: BTreeMap<(u32, u64), FieldPairs> = BTreeMap::new();

            while let Ok(meta) = read_map(&mut socket).await {
                let inner_bytes = read_bytes(&mut socket).await.unwrap();
                let mut cursor = std::io::Cursor::new(inner_bytes);
                let payload = read_map(&mut cursor).await.unwrap();

                let table = String::from_utf8_lossy(meta.get("TABLE_ID").unwrap()).into_owned();
                let action = String::from_utf8_lossy(meta.get("ACTION_ID").unwrap()).into_owned();

                match (table.as_str(), action.as_str()) {
                    ("PING", "READ") => send_response(&mut socket, "0", &[]).await,
                    ("PLU", "WRITE") => {
                        let did: u32 = String::from_utf8_lossy(payload.get("DID").unwrap())
                            .parse()
                            .unwrap();
                        let plu: u64 = String::from_utf8_lossy(payload.get("PLU").unwrap())
                            .parse()
                            .unwrap();
                        store.insert((did, plu), payload.into_iter().collect());
                        send_response(&mut socket, "0", &[]).await;
                    }
                    ("PLU", "READONE") => {
                        let did: u32 = String::from_utf8_lossy(payload.get("F1").unwrap())
                            .parse()
                            .unwrap();
                        let plu: u64 = String::from_utf8_lossy(payload.get("F2").unwrap())
                            .parse()
                            .unwrap();
                        if let Some(rec) = store.get(&(did, plu)) {
                            send_response(&mut socket, "0", std::slice::from_ref(rec)).await;
                        } else {
                            send_response(&mut socket, "0", &[]).await;
                        }
                    }
                    ("PLU", "READ") => {
                        let all: Vec<FieldPairs> = store.values().cloned().collect();
                        send_response(&mut socket, "0", &all).await;
                    }
                    ("PLU", "DELETEONE") => {
                        let did: u32 = String::from_utf8_lossy(payload.get("F1").unwrap())
                            .parse()
                            .unwrap();
                        let plu: u64 = String::from_utf8_lossy(payload.get("F2").unwrap())
                            .parse()
                            .unwrap();
                        store.remove(&(did, plu));
                        send_response(&mut socket, "0", &[]).await;
                    }
                    _ => send_response(&mut socket, "1", &[]).await,
                }
            }
        });

        port
    }

    #[tokio::test]
    async fn xti_driver_full_lifecycle_against_mock_tcp() {
        let port = spawn_mock_xti_server().await;
        let store_id = new_id();
        let mut cfg = XtiDriverConfig::new("127.0.0.1", store_id);
        cfg.port = port;

        let mut driver = XtiDriver::new(cfg);
        let info = driver.connect().await.unwrap();
        assert_eq!(info.model, "XTi400");
        assert_eq!(info.serial_number.as_deref(), Some("12345678"));
        assert!(!info.requires_auth);

        driver.ping().await.unwrap();

        let key = PluKey::new(1, 100).unwrap();
        let plu = Plu {
            id: new_id(),
            key,
            description: "POLLO FRESCO".to_string(),
            button_text: Some("Pollo".to_string()),
            price_1: MoneyCents(4500),
            price_2: MoneyCents(4200),
            price_3: MoneyCents(0),
            product_type: ProductType::ByWeight,
            ean: Some("2000100000000".to_string()),
            sku: Some("SKU-100".to_string()),
            group_id: 1,
            barcode_format_id: 1,
            label_format_id: 1,
            tare_id: None,
            tare_grams: Some(20),
            date_offset_1: 5,
            date_print_control_1: 1,
            date_offset_2: 0,
            date_print_control_2: 0,
            tax_rate_id_1: 0,
            tax_rate_id_2: 0,
            text_areas: vec!["Fresco".to_string()],
            delta_hash: String::new(),
            meta: SyncMetadata::new(store_id),
        };

        driver.upsert_plu(&plu).await.unwrap();
        assert_eq!(driver.count_plus().await.unwrap(), 1);

        // Safe change_price uses READONE + WRITE, preserving button_text & tare_grams
        driver
            .change_price(key, MoneyCents(4900), Some(MoneyCents(4600)), None)
            .await
            .unwrap();
        let updated = driver.read_plu(key).await.unwrap().unwrap();
        assert_eq!(updated.price_1, MoneyCents(4900));
        assert_eq!(updated.price_2, MoneyCents(4600));
        assert_eq!(updated.button_text.as_deref(), Some("Pollo"));
        assert_eq!(updated.tare_grams, Some(20));

        driver.delete_plu(key).await.unwrap();
        assert!(driver.read_plu(key).await.unwrap().is_none());
        driver.disconnect().await.unwrap();
    }
}
