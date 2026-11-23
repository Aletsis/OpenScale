use std::time::Duration;

use async_trait::async_trait;
use openscale_domain::types::{XS_MAX_DID, XS_MAX_PLU_NUMBER};
use openscale_domain::{
    BarcodeFormat, Department, DeptTotals, GrandTotals, GroupTotals, MachTotals, MediaAsset,
    MoneyCents, OperatorTotals, Plu, PluKey, PluTotals, ScaleFamily, StoredText, Tare, TaxRate,
};
use openscale_driver::{DriverCapabilities, DriverError, ScaleDateTime, ScaleDriver, ScaleInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::codec::{
    decode_asset_info, decode_datetime, decode_mach_net_config, decode_plu_read,
    decode_version_details, encode_change_price, encode_datetime, encode_plu_search_header,
    encode_plu_write,
};
use crate::master_tables::{
    decode_barcode_format_read, decode_department_read, decode_media_asset_read,
    decode_stored_text_read, decode_tare_read, decode_tax_read, encode_barcode_format_write,
    encode_department_write, encode_stored_text_write, encode_tare_read, encode_tare_write,
    encode_tax_read, encode_tax_write,
};
use crate::protocol::{
    ACTION_CLEAR, ACTION_REQUEST, ACTION_UPDATE, AbCmdHeader, CMD_ASSETINFO, CMD_BARCODE_FORMATS,
    CMD_CHANGE_PRICE, CMD_DATE_AND_TIME, CMD_DEPT_NAMES, CMD_DEPT_TOTALS, CMD_GRAND_TOTALS,
    CMD_GROUP_TOTALS, CMD_MACH_TOTALS, CMD_MACHNETCONFIG, CMD_MEDIA, CMD_OPERATOR_TOTALS,
    CMD_PLU_RECORD, CMD_PLU_TOTALS, CMD_STORED_TARE, CMD_STORED_TEXT, CMD_TAX_RATES,
    CMD_VERSION_DETAILS, CVN_ASSETINFO, CVN_BARCODE_FORMATS, CVN_CHANGE_PRICE, CVN_DATE_AND_TIME,
    CVN_DEPT_NAMES, CVN_MACHNETCONFIG, CVN_MEDIA, CVN_PLU_RECORD, CVN_PLU_TOTALS, CVN_STORED_TARE,
    CVN_STORED_TEXT, CVN_TAX_RATES, CVN_TOTALS_BASIC, CVN_VERSION_DETAILS, HEADER_LEN,
    MAX_DATA_BLOCK_SIZE, STATUS_NO_TOTALS_FOUND, STATUS_PLU_LAST_PLU, STATUS_PLU_NOT_FOUND,
    STATUS_SUCCESS, XS_DATA_PORT,
};
use crate::sales::{
    decode_dept_totals, decode_grand_totals, decode_group_totals, decode_mach_totals,
    decode_operator_totals, decode_plu_totals,
};

#[derive(Debug, Clone)]
pub struct XsDriverConfig {
    pub host: String,
    pub port: u16,
    pub io_timeout: Duration,
    pub store_id: Uuid,
}

impl XsDriverConfig {
    pub fn new(host: impl Into<String>, store_id: Uuid) -> Self {
        Self {
            host: host.into(),
            port: XS_DATA_PORT,
            io_timeout: Duration::from_secs(5),
            store_id,
        }
    }
}

/// Persistent TCP driver for Avery Berkel XS / M-Series scales (`MSeriesProtocol`).
pub struct XsDriver {
    config: XsDriverConfig,
    stream: Option<TcpStream>,
    sequence: u32,
}

impl XsDriver {
    pub fn new(config: XsDriverConfig) -> Self {
        Self {
            config,
            stream: None,
            sequence: 0,
        }
    }

    fn next_seq(&mut self) -> u32 {
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }

    /// Sends a command using MSeriesProtocol's lockstep stop-and-wait multi-block framing:
    /// each block (atomic 51 B header + up to 500 B chunk) is sent in a single `write_all()`
    /// and followed immediately by reading the scale's corresponding response block.
    async fn send_command(
        &mut self,
        command_type: u16,
        action: u8,
        cvn: u16,
        payload: &[u8],
    ) -> Result<(i16, Vec<u8>), DriverError> {
        let seq = self.next_seq();
        let req_header = AbCmdHeader::new_request(command_type, action, cvn, seq);
        let timeout = self.config.io_timeout;

        let result = {
            let stream = self.stream.as_mut().ok_or(DriverError::NotConnected)?;
            Self::exchange_blocks(stream, req_header, payload, timeout).await
        };

        if matches!(result, Err(DriverError::Io(_) | DriverError::Timeout)) {
            self.stream = None;
        }

        result
    }

    async fn exchange_blocks(
        stream: &mut TcpStream,
        mut req_header: AbCmdHeader,
        payload: &[u8],
        timeout: Duration,
    ) -> Result<(i16, Vec<u8>), DriverError> {
        let send_total = if payload.is_empty() {
            1usize
        } else {
            payload.len().div_ceil(MAX_DATA_BLOCK_SIZE)
        };
        if send_total > usize::from(u8::MAX) {
            return Err(DriverError::Protocol {
                code: None,
                message: format!("payload requires {send_total} blocks (> 255)"),
            });
        }

        let mut block: u8 = 1;
        let mut total: u8 = send_total as u8;
        let mut assembled = Vec::new();
        let mut last_status: i16 = STATUS_SUCCESS;

        while block <= total {
            let chunk_offset = usize::from(block - 1) * MAX_DATA_BLOCK_SIZE;
            let chunk: &[u8] = if chunk_offset < payload.len() {
                let end = (chunk_offset + MAX_DATA_BLOCK_SIZE).min(payload.len());
                &payload[chunk_offset..end]
            } else {
                &[]
            };

            req_header.block_num = block;
            req_header.num_blocks = total;
            let hdr_bytes = req_header.encode(chunk.len())?;

            // Critical XS rule: header (51 B) + chunk must be sent in a single write_all()
            let mut frame = Vec::with_capacity(HEADER_LEN + chunk.len());
            frame.extend_from_slice(&hdr_bytes);
            frame.extend_from_slice(chunk);

            tokio::time::timeout(timeout, stream.write_all(&frame))
                .await
                .map_err(|_| DriverError::Timeout)??;
            tokio::time::timeout(timeout, stream.flush())
                .await
                .map_err(|_| DriverError::Timeout)??;

            let mut hdr_buf = [0u8; HEADER_LEN];
            tokio::time::timeout(timeout, stream.read_exact(&mut hdr_buf))
                .await
                .map_err(|_| DriverError::Timeout)??;

            let resp_hdr = AbCmdHeader::decode(&hdr_buf)?;
            last_status = resp_hdr.status;

            let chunk_len = resp_hdr.payload_len();
            if chunk_len > 0 {
                let start = assembled.len();
                assembled.resize(start + chunk_len, 0);
                tokio::time::timeout(timeout, stream.read_exact(&mut assembled[start..]))
                    .await
                    .map_err(|_| DriverError::Timeout)??;
            }

            if last_status != STATUS_SUCCESS {
                break;
            }
            if resp_hdr.num_blocks > total {
                total = resp_hdr.num_blocks;
            }
            if block == u8::MAX {
                break;
            }
            block += 1;
        }

        Ok((last_status, assembled))
    }

    async fn read_plu_with_flags(
        &mut self,
        key: PluKey,
    ) -> Result<Option<(Plu, [u8; 2])>, DriverError> {
        let req = encode_plu_search_header(key, key, 0, 0, false)?;
        let (status, payload) = self
            .send_command(CMD_PLU_RECORD, ACTION_REQUEST, CVN_PLU_RECORD, &req)
            .await?;

        match status {
            STATUS_SUCCESS => Ok(Some(decode_plu_read(&payload, self.config.store_id)?)),
            STATUS_PLU_NOT_FOUND | STATUS_PLU_LAST_PLU => Ok(None),
            code => Err(DriverError::Protocol {
                code: Some(i32::from(code)),
                message: format!("PLU:READONE failed with status {code}"),
            }),
        }
    }

    async fn probe_plu_index(
        &mut self,
        from: PluKey,
        to: PluKey,
        index: u32,
    ) -> Result<bool, DriverError> {
        let req = encode_plu_search_header(from, to, 0, index, false)?;
        let (status, _) = self
            .send_command(CMD_PLU_RECORD, ACTION_REQUEST, CVN_PLU_RECORD, &req)
            .await?;

        match status {
            STATUS_SUCCESS => Ok(true),
            STATUS_PLU_NOT_FOUND | STATUS_PLU_LAST_PLU => Ok(false),
            code => Err(DriverError::Protocol {
                code: Some(i32::from(code)),
                message: format!("PLU:READ probe failed at index {index} with status {code}"),
            }),
        }
    }
}

#[async_trait]
impl ScaleDriver for XsDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities::xs_default()
    }

    async fn connect(&mut self) -> Result<ScaleInfo, DriverError> {
        // Close any previous socket cleanly before opening a new one (XS only allows 1 active TCP slot).
        let _ = self.disconnect().await;

        let addr = format!("{}:{}", self.config.host, self.config.port);
        let retry_delays = [Duration::from_millis(400), Duration::from_millis(800)];
        let mut attempt = 0usize;

        let stream = loop {
            match tokio::time::timeout(self.config.io_timeout, TcpStream::connect(&addr)).await {
                Ok(Ok(stream)) => break stream,
                Ok(Err(e))
                    if e.kind() == std::io::ErrorKind::ConnectionRefused
                        && attempt < retry_delays.len() =>
                {
                    tokio::time::sleep(retry_delays[attempt]).await;
                    attempt += 1;
                }
                Ok(Err(e)) => {
                    return Err(DriverError::ConnectionFailed(format!("{addr}: {e}")));
                }
                Err(_) => return Err(DriverError::Timeout),
            }
        };

        // Critical XS rule: TCP_NODELAY = true (paired with atomic write_all)
        stream.set_nodelay(true)?;
        self.stream = Some(stream);
        match self.get_info().await {
            Ok(info) => Ok(info),
            Err(err) => {
                self.stream = None;
                Err(err)
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), DriverError> {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.shutdown().await;
        }
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DriverError> {
        let (status, _) = self
            .send_command(
                CMD_VERSION_DETAILS,
                ACTION_REQUEST,
                CVN_VERSION_DETAILS,
                &[],
            )
            .await?;
        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("VERSION_DETAILS failed with status {status}"),
            })
        }
    }

    async fn get_info(&mut self) -> Result<ScaleInfo, DriverError> {
        let (status, ver_payload) = self
            .send_command(
                CMD_VERSION_DETAILS,
                ACTION_REQUEST,
                CVN_VERSION_DETAILS,
                &[],
            )
            .await?;
        if status != STATUS_SUCCESS {
            return Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("VERSION_DETAILS failed with status {status}"),
            });
        }
        let version = decode_version_details(&ver_payload);

        let asset = match self
            .send_command(CMD_ASSETINFO, ACTION_REQUEST, CVN_ASSETINFO, &[])
            .await
        {
            Ok((STATUS_SUCCESS, data)) => decode_asset_info(&data).ok(),
            _ => None,
        };

        let net = match self
            .send_command(CMD_MACHNETCONFIG, ACTION_REQUEST, CVN_MACHNETCONFIG, &[])
            .await
        {
            Ok((STATUS_SUCCESS, data)) => decode_mach_net_config(&data).ok(),
            _ => None,
        };

        Ok(ScaleInfo {
            family: ScaleFamily::AveryBerkelXs,
            model: asset
                .as_ref()
                .map(|a| a.model.clone())
                .filter(|m| !m.is_empty())
                .unwrap_or(version.product),
            serial_number: asset.as_ref().map(|a| a.serial_number.clone()),
            firmware_version: asset
                .as_ref()
                .map(|a| a.app_version.clone())
                .filter(|v| !v.is_empty())
                .unwrap_or(version.version),
            protocol_version: asset.map(|a| a.protocol),
            mac_address: net.map(|n| n.mac_address),
            requires_auth: false,
        })
    }

    async fn get_datetime(&mut self) -> Result<ScaleDateTime, DriverError> {
        let (status, payload) = self
            .send_command(CMD_DATE_AND_TIME, ACTION_REQUEST, CVN_DATE_AND_TIME, &[])
            .await?;
        if status != STATUS_SUCCESS {
            return Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("DATE_AND_TIME read failed with status {status}"),
            });
        }
        decode_datetime(&payload)
    }

    async fn set_datetime(&mut self, dt: ScaleDateTime) -> Result<(), DriverError> {
        let payload = encode_datetime(dt);
        let (status, _) = self
            .send_command(
                CMD_DATE_AND_TIME,
                ACTION_UPDATE,
                CVN_DATE_AND_TIME,
                &payload,
            )
            .await?;
        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("DATE_AND_TIME write failed with status {status}"),
            })
        }
    }

    async fn read_plu(&mut self, key: PluKey) -> Result<Option<Plu>, DriverError> {
        Ok(self.read_plu_with_flags(key).await?.map(|(plu, _)| plu))
    }

    async fn list_plus(
        &mut self,
        from: Option<PluKey>,
        to: Option<PluKey>,
    ) -> Result<Vec<Plu>, DriverError> {
        let from_key = from.unwrap_or(PluKey::new(0, 1)?);
        let to_key = to.unwrap_or(PluKey::new(XS_MAX_DID, XS_MAX_PLU_NUMBER)?);

        let mut plus = Vec::new();
        let mut index: u32 = 0;

        loop {
            let req = encode_plu_search_header(from_key, to_key, 0, index, false)?;
            let (status, payload) = self
                .send_command(CMD_PLU_RECORD, ACTION_REQUEST, CVN_PLU_RECORD, &req)
                .await?;

            match status {
                STATUS_SUCCESS => {
                    let (plu, _) = decode_plu_read(&payload, self.config.store_id)?;
                    plus.push(plu);
                    index = index.saturating_add(1);
                }
                STATUS_PLU_NOT_FOUND | STATUS_PLU_LAST_PLU => break,
                code => {
                    return Err(DriverError::Protocol {
                        code: Some(i32::from(code)),
                        message: format!("PLU:READ failed at index {index} with status {code}"),
                    });
                }
            }
        }

        Ok(plus)
    }

    /// O(log N) count using exponential + binary search on `SearchIndex`.
    async fn count_plus(&mut self) -> Result<u64, DriverError> {
        let from = PluKey::new(0, 1)?;
        let to = PluKey::new(XS_MAX_DID, XS_MAX_PLU_NUMBER)?;

        if !self.probe_plu_index(from, to, 0).await? {
            return Ok(0);
        }

        let mut low: u32 = 0;
        let mut high: u32 = 1;
        while self.probe_plu_index(from, to, high).await? {
            low = high;
            high = high.saturating_mul(2);
            if high == u32::MAX {
                break;
            }
        }

        while low + 1 < high {
            let mid = low + (high - low) / 2;
            if self.probe_plu_index(from, to, mid).await? {
                low = mid;
            } else {
                high = mid;
            }
        }

        Ok(u64::from(low) + 1)
    }

    async fn upsert_plu(&mut self, plu: &Plu) -> Result<(), DriverError> {
        // Preserve internal scale flags (rec[0x4F..0x51]) when editing an existing PLU
        let existing_flags = self
            .read_plu_with_flags(plu.key)
            .await?
            .map(|(_, flags)| flags);

        let payload = encode_plu_write(plu, existing_flags)?;
        let (status, _) = self
            .send_command(CMD_PLU_RECORD, ACTION_UPDATE, CVN_PLU_RECORD, &payload)
            .await?;

        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("PLU:WRITE failed with status {status}"),
            })
        }
    }

    async fn change_price(
        &mut self,
        key: PluKey,
        price_1: MoneyCents,
        price_2: Option<MoneyCents>,
        price_3: Option<MoneyCents>,
    ) -> Result<(), DriverError> {
        let payload = encode_change_price(
            key,
            price_1,
            price_2.unwrap_or(MoneyCents(0)),
            price_3.unwrap_or(MoneyCents(0)),
        )?;
        let (status, _) = self
            .send_command(CMD_CHANGE_PRICE, ACTION_UPDATE, CVN_CHANGE_PRICE, &payload)
            .await?;

        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("CHANGE_PRICE failed with status {status}"),
            })
        }
    }

    async fn delete_plu(&mut self, key: PluKey) -> Result<(), DriverError> {
        let req = encode_plu_search_header(key, key, 0, 0, false)?;
        let (status, _) = self
            .send_command(CMD_PLU_RECORD, ACTION_CLEAR, CVN_PLU_RECORD, &req)
            .await?;

        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("PLU:DELETEONE failed with status {status}"),
            })
        }
    }

    async fn read_department(&mut self, did: u32) -> Result<Option<Department>, DriverError> {
        let did_u8 = u8::try_from(did).map_err(|_| DriverError::Protocol {
            code: None,
            message: format!("XS department DID {did} out of range"),
        })?;
        let (status, payload) = self
            .send_command(CMD_DEPT_NAMES, ACTION_REQUEST, CVN_DEPT_NAMES, &[did_u8])
            .await?;
        if status == STATUS_SUCCESS {
            Ok(Some(decode_department_read(
                &payload,
                self.config.store_id,
            )?))
        } else {
            Ok(None)
        }
    }

    async fn upsert_department(&mut self, dept: &Department) -> Result<(), DriverError> {
        let payload = encode_department_write(dept)?;
        let (status, _) = self
            .send_command(CMD_DEPT_NAMES, ACTION_UPDATE, CVN_DEPT_NAMES, &payload)
            .await?;
        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("DEPT_NAMES write failed with status {status}"),
            })
        }
    }

    async fn read_tare(&mut self, cid: u16) -> Result<Option<Tare>, DriverError> {
        let req = encode_tare_read(cid)?;
        let (status, payload) = self
            .send_command(CMD_STORED_TARE, ACTION_REQUEST, CVN_STORED_TARE, &req)
            .await?;
        if status == STATUS_SUCCESS {
            Ok(Some(decode_tare_read(&payload, self.config.store_id)?))
        } else {
            Ok(None)
        }
    }

    async fn upsert_tare(&mut self, tare: &Tare) -> Result<(), DriverError> {
        let payload = encode_tare_write(tare)?;
        let (status, _) = self
            .send_command(CMD_STORED_TARE, ACTION_UPDATE, CVN_STORED_TARE, &payload)
            .await?;
        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("STORED_TARE write failed with status {status}"),
            })
        }
    }

    async fn read_tax_rate(&mut self, tax_id: u8) -> Result<Option<TaxRate>, DriverError> {
        let req = encode_tax_read(tax_id);
        let (status, payload) = self
            .send_command(CMD_TAX_RATES, ACTION_REQUEST, CVN_TAX_RATES, &req)
            .await?;
        if status == STATUS_SUCCESS {
            Ok(Some(decode_tax_read(&payload, self.config.store_id)?))
        } else {
            Ok(None)
        }
    }

    async fn upsert_tax_rate(&mut self, tax: &TaxRate) -> Result<(), DriverError> {
        let payload = encode_tax_write(tax);
        let (status, _) = self
            .send_command(CMD_TAX_RATES, ACTION_UPDATE, CVN_TAX_RATES, &payload)
            .await?;
        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("TAX_RATES write failed with status {status}"),
            })
        }
    }

    async fn read_stored_text(
        &mut self,
        text_id: u32,
        table: u8,
    ) -> Result<Option<StoredText>, DriverError> {
        let id_u8 = u8::try_from(text_id).map_err(|_| DriverError::Protocol {
            code: None,
            message: format!("XS stored text ID {text_id} out of range"),
        })?;
        let (status, payload) = self
            .send_command(
                CMD_STORED_TEXT,
                ACTION_REQUEST,
                CVN_STORED_TEXT,
                &[id_u8, table],
            )
            .await?;
        if status == STATUS_SUCCESS {
            Ok(Some(decode_stored_text_read(
                &payload,
                self.config.store_id,
            )?))
        } else {
            Ok(None)
        }
    }

    async fn upsert_stored_text(
        &mut self,
        text: &StoredText,
        table: u8,
    ) -> Result<(), DriverError> {
        let payload = encode_stored_text_write(text, table)?;
        let (status, _) = self
            .send_command(CMD_STORED_TEXT, ACTION_UPDATE, CVN_STORED_TEXT, &payload)
            .await?;
        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("STORED_TEXT write failed with status {status}"),
            })
        }
    }

    async fn read_barcode_format(
        &mut self,
        bid: u16,
    ) -> Result<Option<BarcodeFormat>, DriverError> {
        let bid_u8 = u8::try_from(bid).map_err(|_| DriverError::Protocol {
            code: None,
            message: format!("XS barcode BID {bid} out of range"),
        })?;
        let (status, payload) = self
            .send_command(
                CMD_BARCODE_FORMATS,
                ACTION_REQUEST,
                CVN_BARCODE_FORMATS,
                &[bid_u8],
            )
            .await?;
        if status == STATUS_SUCCESS {
            Ok(Some(decode_barcode_format_read(
                &payload,
                self.config.store_id,
            )?))
        } else {
            Ok(None)
        }
    }

    async fn upsert_barcode_format(&mut self, fmt: &BarcodeFormat) -> Result<(), DriverError> {
        let payload = encode_barcode_format_write(fmt)?;
        let (status, _) = self
            .send_command(
                CMD_BARCODE_FORMATS,
                ACTION_UPDATE,
                CVN_BARCODE_FORMATS,
                &payload,
            )
            .await?;
        if status == STATUS_SUCCESS {
            Ok(())
        } else {
            Err(DriverError::Protocol {
                code: Some(i32::from(status)),
                message: format!("BARCODE_FORMATS write failed with status {status}"),
            })
        }
    }

    async fn read_media_asset(&mut self, media_id: u32) -> Result<Option<MediaAsset>, DriverError> {
        let meid = u8::try_from(media_id).map_err(|_| DriverError::Protocol {
            code: None,
            message: format!("XS media ID {media_id} out of range"),
        })?;
        let (status, payload) = self
            .send_command(CMD_MEDIA, ACTION_REQUEST, CVN_MEDIA, &[meid])
            .await?;
        if status == STATUS_SUCCESS {
            Ok(Some(decode_media_asset_read(
                media_id,
                &payload,
                self.config.store_id,
            )?))
        } else {
            Ok(None)
        }
    }

    async fn read_grand_totals(&mut self, period: u8) -> Result<Option<GrandTotals>, DriverError> {
        let (status, payload) = self
            .send_command(
                CMD_GRAND_TOTALS,
                ACTION_REQUEST,
                CVN_TOTALS_BASIC,
                &[period],
            )
            .await?;
        match status {
            STATUS_SUCCESS => Ok(Some(decode_grand_totals(period, &payload)?)),
            STATUS_NO_TOTALS_FOUND => Ok(None),
            code => Err(DriverError::Protocol {
                code: Some(i32::from(code)),
                message: format!("GRAND_TOTALS failed with status {code}"),
            }),
        }
    }

    async fn list_dept_totals(&mut self, period: u8) -> Result<Vec<DeptTotals>, DriverError> {
        let mut results = Vec::new();
        let mut start_did: u8 = 0;

        loop {
            let (status, payload) = self
                .send_command(
                    CMD_DEPT_TOTALS,
                    ACTION_REQUEST,
                    CVN_TOTALS_BASIC,
                    &[period, start_did, 1],
                )
                .await?;

            match status {
                STATUS_SUCCESS => {
                    let item = decode_dept_totals(period, &payload)?;
                    let next = (item.did as u8).saturating_add(1);
                    results.push(item);
                    if next <= start_did || next > 99 {
                        break;
                    }
                    start_did = next;
                }
                STATUS_NO_TOTALS_FOUND | STATUS_PLU_LAST_PLU | STATUS_PLU_NOT_FOUND => break,
                code => {
                    return Err(DriverError::Protocol {
                        code: Some(i32::from(code)),
                        message: format!("DEPT_TOTALS failed with status {code}"),
                    });
                }
            }
        }

        Ok(results)
    }

    async fn read_group_totals(
        &mut self,
        period: u8,
        group_id: u32,
    ) -> Result<Option<GroupTotals>, DriverError> {
        let gid = u8::try_from(group_id).unwrap_or(0);
        let (status, payload) = self
            .send_command(
                CMD_GROUP_TOTALS,
                ACTION_REQUEST,
                CVN_TOTALS_BASIC,
                &[period, gid, 0],
            )
            .await?;
        match status {
            STATUS_SUCCESS => Ok(Some(decode_group_totals(period, group_id, &payload)?)),
            STATUS_NO_TOTALS_FOUND => Ok(None),
            code => Err(DriverError::Protocol {
                code: Some(i32::from(code)),
                message: format!("GROUP_TOTALS failed with status {code}"),
            }),
        }
    }

    async fn read_operator_totals(
        &mut self,
        period: u8,
        operator_id: u32,
    ) -> Result<Option<OperatorTotals>, DriverError> {
        let opid = u8::try_from(operator_id).unwrap_or(0);
        let (status, payload) = self
            .send_command(
                CMD_OPERATOR_TOTALS,
                ACTION_REQUEST,
                CVN_TOTALS_BASIC,
                &[period, opid, 0],
            )
            .await?;
        match status {
            STATUS_SUCCESS => Ok(Some(decode_operator_totals(period, operator_id, &payload)?)),
            STATUS_NO_TOTALS_FOUND => Ok(None),
            code => Err(DriverError::Protocol {
                code: Some(i32::from(code)),
                message: format!("OPERATOR_TOTALS failed with status {code}"),
            }),
        }
    }

    async fn read_mach_totals(&mut self, period: u8) -> Result<Option<MachTotals>, DriverError> {
        let (status, payload) = self
            .send_command(CMD_MACH_TOTALS, ACTION_REQUEST, CVN_TOTALS_BASIC, &[period])
            .await?;
        match status {
            STATUS_SUCCESS => Ok(Some(decode_mach_totals(period, &payload)?)),
            STATUS_NO_TOTALS_FOUND => Ok(None),
            code => Err(DriverError::Protocol {
                code: Some(i32::from(code)),
                message: format!("MACH_TOTALS failed with status {code}"),
            }),
        }
    }

    async fn list_plu_totals(&mut self, period: u8) -> Result<Vec<PluTotals>, DriverError> {
        let from_key = PluKey::new(0, 1)?;
        let to_key = PluKey::new(XS_MAX_DID, XS_MAX_PLU_NUMBER)?;
        let mut results = Vec::new();
        let mut index: u32 = 0;

        loop {
            let search = encode_plu_search_header(from_key, to_key, 0, index, false)?;
            let mut req = Vec::with_capacity(42);
            req.push(period);
            req.extend_from_slice(&search);

            let (status, payload) = self
                .send_command(CMD_PLU_TOTALS, ACTION_REQUEST, CVN_PLU_TOTALS, &req)
                .await?;

            match status {
                STATUS_SUCCESS => {
                    results.push(decode_plu_totals(period, &payload)?);
                    index = index.saturating_add(1);
                }
                STATUS_NO_TOTALS_FOUND | STATUS_PLU_LAST_PLU | STATUS_PLU_NOT_FOUND => break,
                code => {
                    return Err(DriverError::Protocol {
                        code: Some(i32::from(code)),
                        message: format!("PLU_TOTALS failed at index {index} with status {code}"),
                    });
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{KIND_RESPONSE, build_atomic_blocks};
    use openscale_domain::types::new_id;
    use openscale_domain::{ProductType, SyncMetadata};
    use std::collections::BTreeMap;
    use tokio::net::TcpListener;

    async fn spawn_mock_xs_server() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let store_id = new_id();
            let mut plus: BTreeMap<PluKey, Vec<u8>> = BTreeMap::new();

            // Pre-populate 5 PLUs (PLU 5 has >500 B payload to exercise lockstep multi-block READ)
            for n in 1..=5u64 {
                let key = PluKey::new(1, n).unwrap();
                let text_areas = if n == 5 {
                    vec!["INGREDIENTES: ".to_string() + &"CARNE DE RES, SAL, ESPECIAS. ".repeat(20)]
                } else {
                    vec![]
                };
                let plu = Plu {
                    id: new_id(),
                    key,
                    description: format!("ITEM {n}"),
                    button_text: None,
                    price_1: MoneyCents((n as i64) * 1000),
                    price_2: MoneyCents(0),
                    price_3: MoneyCents(0),
                    product_type: ProductType::ByWeight,
                    ean: None,
                    sku: None,
                    group_id: 0,
                    barcode_format_id: 1,
                    label_format_id: 1,
                    tare_id: Some(0),
                    tare_grams: None,
                    date_offset_1: 0,
                    date_print_control_1: 0,
                    date_offset_2: 0,
                    date_print_control_2: 0,
                    tax_rate_id_1: 0,
                    tax_rate_id_2: 0,
                    text_areas,
                    delta_hash: String::new(),
                    meta: SyncMetadata::new(store_id),
                };
                plus.insert(key, encode_plu_write(&plu, Some([0xAA, 0xBB])).unwrap());
            }

            let mut hdr_buf = [0u8; HEADER_LEN];
            while socket.read_exact(&mut hdr_buf).await.is_ok() {
                let req = AbCmdHeader::decode(&hdr_buf).unwrap();
                let mut payload = vec![0u8; req.payload_len()];
                if !payload.is_empty() {
                    socket.read_exact(&mut payload).await.unwrap();
                }

                // Lockstep multi-block request reception (e.g. PLU:WRITE > 500 B)
                let mut current_req = req;
                while current_req.block_num < current_req.num_blocks {
                    let mut ack_hdr = current_req;
                    ack_hdr.command_or_response = KIND_RESPONSE;
                    ack_hdr.status = STATUS_SUCCESS;
                    let ack = ack_hdr.encode(0).unwrap();
                    socket.write_all(&ack).await.unwrap();

                    socket.read_exact(&mut hdr_buf).await.unwrap();
                    current_req = AbCmdHeader::decode(&hdr_buf).unwrap();
                    let mut chunk = vec![0u8; current_req.payload_len()];
                    if !chunk.is_empty() {
                        socket.read_exact(&mut chunk).await.unwrap();
                        payload.extend_from_slice(&chunk);
                    }
                }

                let (status, resp_payload): (i16, Vec<u8>) = match (req.command_type, req.action) {
                    (CMD_VERSION_DETAILS, ACTION_REQUEST) => (
                        STATUS_SUCCESS,
                        b"M Series Scale\0APP. v4.1.7.0\0Jan 12 2016\0PROTOCOL 8000\0".to_vec(),
                    ),
                    (CMD_ASSETINFO, ACTION_REQUEST) => {
                        let mut data = vec![0u8; 316];
                        data[0x33..0x3B].copy_from_slice(b"22320310");
                        data[0x69..0x6C].copy_from_slice(b"200");
                        data[0xBC..0xC9].copy_from_slice(b"APP. v4.1.7.0");
                        data[0x125..0x132].copy_from_slice(b"PROTOCOL 8000");
                        (STATUS_SUCCESS, data)
                    }
                    (CMD_MACHNETCONFIG, ACTION_REQUEST) => {
                        let mut data = vec![0u8; 48];
                        data[0..4].copy_from_slice(&[127, 0, 0, 1]);
                        data[0x20..0x26].copy_from_slice(&[0x00, 0xE0, 0x8A, 0x0D, 0x72, 0xAF]);
                        (STATUS_SUCCESS, data)
                    }
                    (CMD_DATE_AND_TIME, ACTION_REQUEST) => {
                        let dt = ScaleDateTime {
                            year: 2026,
                            month: 11,
                            day: 22,
                            hour: 18,
                            minute: 30,
                            second: 0,
                        };
                        (STATUS_SUCCESS, encode_datetime(dt).to_vec())
                    }
                    (CMD_DATE_AND_TIME, ACTION_UPDATE) => (STATUS_SUCCESS, vec![]),
                    (CMD_PLU_RECORD, ACTION_REQUEST) => {
                        let mut from_raw = [0u8; 8];
                        let mut to_raw = [0u8; 8];
                        from_raw.copy_from_slice(&payload[0..8]);
                        to_raw.copy_from_slice(&payload[8..16]);
                        let from_key = PluKey::from_xs_bytes(from_raw).unwrap();
                        let to_key = PluKey::from_xs_bytes(to_raw).unwrap();
                        let idx = u32::from_be_bytes([
                            payload[32],
                            payload[33],
                            payload[34],
                            payload[35],
                        ]) as usize;

                        let matching: Vec<&Vec<u8>> =
                            plus.range(from_key..=to_key).map(|(_, v)| v).collect();
                        if let Some(found) = matching.get(idx) {
                            (STATUS_SUCCESS, (*found).clone())
                        } else {
                            (STATUS_PLU_LAST_PLU, vec![])
                        }
                    }
                    (CMD_PLU_RECORD, ACTION_UPDATE) => {
                        let mut key_raw = [0u8; 8];
                        key_raw.copy_from_slice(&payload[0..8]);
                        let key = PluKey::from_xs_bytes(key_raw).unwrap();
                        plus.insert(key, payload);
                        (STATUS_SUCCESS, vec![])
                    }
                    (CMD_PLU_RECORD, ACTION_CLEAR) => {
                        let mut key_raw = [0u8; 8];
                        key_raw.copy_from_slice(&payload[0..8]);
                        let key = PluKey::from_xs_bytes(key_raw).unwrap();
                        plus.remove(&key);
                        (STATUS_SUCCESS, vec![])
                    }
                    (CMD_CHANGE_PRICE, ACTION_UPDATE) => {
                        let mut key_raw = [0u8; 8];
                        key_raw.copy_from_slice(&payload[0..8]);
                        let key = PluKey::from_xs_bytes(key_raw).unwrap();
                        if let Some(existing) = plus.get_mut(&key) {
                            existing[41 + 0x0D..41 + 0x11].copy_from_slice(&payload[16..20]);
                            (STATUS_SUCCESS, vec![])
                        } else {
                            (STATUS_PLU_NOT_FOUND, vec![])
                        }
                    }
                    _ => (-1117, vec![]),
                };

                let mut resp_hdr = current_req;
                resp_hdr.command_or_response = KIND_RESPONSE;
                resp_hdr.status = status;
                let frames = build_atomic_blocks(resp_hdr, &resp_payload).unwrap();
                for (i, frame) in frames.iter().enumerate() {
                    if i > 0 {
                        // Lockstep: wait for client continuation request before sending next block
                        socket.read_exact(&mut hdr_buf).await.unwrap();
                        let cont_req = AbCmdHeader::decode(&hdr_buf).unwrap();
                        assert_eq!(cont_req.block_num, (i + 1) as u8);
                        if cont_req.payload_len() > 0 {
                            let mut discard = vec![0u8; cont_req.payload_len()];
                            socket.read_exact(&mut discard).await.unwrap();
                        }
                    }
                    socket.write_all(frame).await.unwrap();
                }
            }
        });

        port
    }

    #[tokio::test]
    async fn xs_driver_full_lifecycle_against_mock_tcp() {
        let port = spawn_mock_xs_server().await;
        let store_id = new_id();
        let mut cfg = XsDriverConfig::new("127.0.0.1", store_id);
        cfg.port = port;

        let mut driver = XsDriver::new(cfg);
        let info = driver.connect().await.unwrap();
        assert_eq!(info.model, "XS200");
        assert_eq!(info.serial_number.as_deref(), Some("22320310"));
        assert_eq!(info.mac_address.as_deref(), Some("00:E0:8A:0D:72:AF"));

        driver.ping().await.unwrap();
        assert_eq!(driver.get_datetime().await.unwrap().year, 2026);
        driver
            .set_datetime(ScaleDateTime {
                year: 2026,
                month: 11,
                day: 22,
                hour: 19,
                minute: 0,
                second: 0,
            })
            .await
            .unwrap();

        assert_eq!(driver.count_plus().await.unwrap(), 5);
        let listed = driver.list_plus(None, None).await.unwrap();
        assert_eq!(listed.len(), 5);
        assert!(listed[4].text_areas[0].len() > 500);

        // Exercise lockstep multi-block WRITE (>500 B) and read-back
        let mut long_plu = listed[4].clone();
        long_plu.key = PluKey::new(1, 6).unwrap();
        long_plu.description = "MULTI BLOCK WRITE".to_string();
        driver.upsert_plu(&long_plu).await.unwrap();
        let read_back = driver.read_plu(long_plu.key).await.unwrap().unwrap();
        assert_eq!(read_back.description, "MULTI BLOCK WRITE");
        assert_eq!(read_back.text_areas, long_plu.text_areas);
        driver.delete_plu(long_plu.key).await.unwrap();

        let key = PluKey::new(1, 3).unwrap();
        driver
            .change_price(key, MoneyCents(9999), None, None)
            .await
            .unwrap();
        let updated = driver.read_plu(key).await.unwrap().unwrap();
        assert_eq!(updated.price_1, MoneyCents(9999));

        driver.delete_plu(key).await.unwrap();
        assert_eq!(driver.count_plus().await.unwrap(), 4);
        driver.disconnect().await.unwrap();
    }
}
