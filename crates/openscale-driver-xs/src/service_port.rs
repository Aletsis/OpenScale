use std::time::Duration;

use chrono::Utc;
use openscale_driver::{DriverError, LcdFrame};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::protocol::XS_SERVICE_PORT;

const STX: u8 = 0x02;
const ETX: u8 = 0x03;

/// Fixed 10-byte initial state for `XsServicePortCommand$Crypt` (signed `i8` arithmetic).
const INITIAL_KEY: [i8; 10] = [-52, -74, -89, -9, -53, -56, 105, -51, -50, -49];

/// Symmetric stream cipher used by Avery Berkel `XsServicePort` (TCP `28822`).
/// State resets for every outgoing request and every incoming response.
#[derive(Debug, Clone)]
pub struct XsServiceCrypt {
    state: [i8; 10],
    ptr: usize,
}

impl Default for XsServiceCrypt {
    fn default() -> Self {
        Self::new()
    }
}

impl XsServiceCrypt {
    pub fn new() -> Self {
        Self {
            state: INITIAL_KEY,
            ptr: 0,
        }
    }

    pub fn transform_byte(&mut self, byte: u8) -> u8 {
        let mask = (self.state[self.ptr] ^ self.state[0].wrapping_mul(self.ptr as i8)) as u8;
        let out = byte ^ mask;
        let next = if self.ptr < 9 { self.ptr + 1 } else { 0 };
        let mut updated = self.state[self.ptr].wrapping_add(self.state[next]);
        if updated == 0 {
            updated = 1;
        }
        self.state[self.ptr] = updated;
        self.ptr = (self.ptr + 1) % 10;
        out
    }

    pub fn transform_slice(&mut self, input: &[u8]) -> Vec<u8> {
        input.iter().map(|&b| self.transform_byte(b)).collect()
    }
}

/// Encrypts a plaintext `XsServicePort` command into `[STX]SelectCommand:<cmd>:<payload>[ETX]`.
pub fn encode_service_command(cmd: u8, payload: &str) -> Vec<u8> {
    let plain = format!("\x02SelectCommand:{cmd}:{payload}\x03");
    let mut crypt = XsServiceCrypt::new();
    crypt.transform_slice(plain.as_bytes())
}

/// Decrypts a raw `XsServicePort` response frame and returns `(is_error, code, payload)`.
pub fn decode_service_response(raw: &[u8]) -> Result<(bool, String, String), DriverError> {
    let mut crypt = XsServiceCrypt::new();
    let decrypted = crypt.transform_slice(raw);
    let text: String = decrypted
        .into_iter()
        .filter(|&b| b != STX && b != ETX)
        .map(char::from)
        .collect();

    if let Some(rest) = text.strip_prefix("ReplyCommand:") {
        let (code, data) = rest.split_once(':').unwrap_or((rest, ""));
        return Ok((false, code.to_string(), data.to_string()));
    }
    if let Some(rest) = text.strip_prefix("CommandError:") {
        let (code, msg) = rest.split_once(':').unwrap_or((rest, ""));
        return Ok((true, code.to_string(), msg.to_string()));
    }

    Err(DriverError::Protocol {
        code: None,
        message: format!("unexpected XsServicePort reply: {text:?}"),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceWeightReading {
    pub raw: String,
    pub weight_display: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleSettings748 {
    pub version: String,
    pub bank1: String,
    pub bank2: String,
    pub bank3: String,
    pub bank4: String,
}

impl ScaleSettings748 {
    pub fn parse(raw: &str) -> Result<Self, DriverError> {
        let parts: Vec<&str> = raw.trim().split(',').collect();
        if parts.len() < 5 {
            return Err(DriverError::Protocol {
                code: None,
                message: format!("invalid #748 configuration string: {raw:?}"),
            });
        }
        Ok(Self {
            version: parts[0].to_string(),
            bank1: parts[1].to_string(),
            bank2: parts[2].to_string(),
            bank3: parts[3].to_string(),
            bank4: parts[4].to_string(),
        })
    }

    pub fn format(&self) -> String {
        format!(
            "{},{},{},{},{}",
            self.version, self.bank1, self.bank2, self.bank3, self.bank4
        )
    }
}

/// Parses `READ_DISPLAY_BITMAP` (`CMD=56`) reply: `"320 x 128,1,<10240 hex chars>"`.
pub fn parse_lcd_bitmap_reply(raw: &str) -> Result<LcdFrame, DriverError> {
    let parts: Vec<&str> = raw.trim().splitn(3, ',').collect();
    if parts.len() < 3 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("invalid READ_DISPLAY_BITMAP reply: {raw:?}"),
        });
    }
    let dims: Vec<&str> = parts[0].split(['x', 'X']).collect();
    let width: u16 = dims
        .first()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(320);
    let height: u16 = dims
        .get(1)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(128);

    let hex_bytes: Vec<u8> = parts[2].bytes().filter(u8::is_ascii_hexdigit).collect();
    let mut bitmap = Vec::with_capacity(hex_bytes.len() / 2);
    for pair in hex_bytes.chunks_exact(2) {
        let s = std::str::from_utf8(pair).map_err(|_| DriverError::Protocol {
            code: None,
            message: "non-ASCII hex in LCD bitmap".to_string(),
        })?;
        let val = u8::from_str_radix(s, 16).map_err(|_| DriverError::Protocol {
            code: None,
            message: format!("invalid hex byte {s:?} in LCD bitmap"),
        })?;
        bitmap.push(val);
    }

    Ok(LcdFrame {
        width,
        height,
        bitmap,
        captured_at: Utc::now(),
    })
}

/// TCP client for Avery Berkel `XsServicePort` (default port `28822`).
pub struct XsServiceClient {
    stream: TcpStream,
    timeout: Duration,
}

impl XsServiceClient {
    pub async fn connect(
        host: &str,
        port: Option<u16>,
        timeout: Duration,
    ) -> Result<Self, DriverError> {
        let addr = format!("{}:{}", host, port.unwrap_or(XS_SERVICE_PORT));
        let stream = tokio::time::timeout(timeout, TcpStream::connect(&addr))
            .await
            .map_err(|_| DriverError::Timeout)?
            .map_err(|e| DriverError::ConnectionFailed(format!("{addr}: {e}")))?;
        stream.set_nodelay(true)?;
        Ok(Self { stream, timeout })
    }

    /// Reads one encrypted `STX..ETX` frame by decrypting bytes as they arrive.
    async fn read_encrypted_frame(&mut self) -> Result<(bool, String, String), DriverError> {
        let mut crypt = XsServiceCrypt::new();
        let mut decrypted = Vec::with_capacity(256);
        let mut buf = [0u8; 1];

        loop {
            tokio::time::timeout(self.timeout, self.stream.read_exact(&mut buf))
                .await
                .map_err(|_| DriverError::Timeout)??;
            let plain = crypt.transform_byte(buf[0]);
            decrypted.push(plain);
            if plain == ETX {
                break;
            }
        }

        let text: String = decrypted
            .into_iter()
            .filter(|&b| b != STX && b != ETX)
            .map(char::from)
            .collect();

        if let Some(rest) = text.strip_prefix("ReplyCommand:") {
            let (code, data) = rest.split_once(':').unwrap_or((rest, ""));
            return Ok((false, code.to_string(), data.to_string()));
        }
        if let Some(rest) = text.strip_prefix("CommandError:") {
            let (code, msg) = rest.split_once(':').unwrap_or((rest, ""));
            return Ok((true, code.to_string(), msg.to_string()));
        }
        Err(DriverError::Protocol {
            code: None,
            message: format!("unexpected XsServicePort reply: {text:?}"),
        })
    }

    pub async fn send(&mut self, cmd: u8, payload: &str) -> Result<String, DriverError> {
        let frame = encode_service_command(cmd, payload);
        tokio::time::timeout(self.timeout, self.stream.write_all(&frame))
            .await
            .map_err(|_| DriverError::Timeout)??;
        tokio::time::timeout(self.timeout, self.stream.flush())
            .await
            .map_err(|_| DriverError::Timeout)??;

        // Skip any lagging frames from previous commands until we receive the reply for `cmd`
        for _ in 0..150 {
            let (is_err, code, data) = self.read_encrypted_frame().await?;
            let parsed_code = code.trim().parse::<i32>().ok();
            let matches_cmd = parsed_code == Some(i32::from(cmd)) || parsed_code == Some(0);
            if !matches_cmd {
                continue;
            }
            if is_err {
                return Err(DriverError::Protocol {
                    code: parsed_code,
                    message: format!("XsServicePort error {code}: {data}"),
                });
            }
            return Ok(data);
        }

        Err(DriverError::Timeout)
    }

    pub async fn read_weight(&mut self) -> Result<ServiceWeightReading, DriverError> {
        let raw = self.send(14, "").await?;
        let parts: Vec<&str> = raw.split(',').collect();
        Ok(ServiceWeightReading {
            weight_display: parts.first().copied().unwrap_or("0.000").trim().to_string(),
            status: parts.get(3).copied().unwrap_or("").trim().to_string(),
            raw,
        })
    }

    pub async fn balance_scale(&mut self) -> Result<String, DriverError> {
        self.send(15, "").await
    }

    pub async fn write_tare(&mut self, tare_str: &str) -> Result<String, DriverError> {
        self.send(16, tare_str).await
    }

    pub async fn read_pool_size(&mut self, pool: &str) -> Result<String, DriverError> {
        self.send(30, pool).await
    }

    pub async fn read_memory_details(&mut self, section: &str) -> Result<String, DriverError> {
        self.send(42, section).await
    }

    pub async fn activate_cash_drawer(&mut self) -> Result<String, DriverError> {
        self.send(46, "").await
    }

    pub async fn print_test_report(&mut self) -> Result<String, DriverError> {
        self.send(47, "").await
    }

    /// Reads multi-frame diagnostic log (`SYSTEM`, `COMMS`, `KEYBOARD`). `clear = false` (`0`) preserves log.
    pub async fn read_diagnostic_log(
        &mut self,
        log_type: &str,
        clear: bool,
    ) -> Result<Vec<String>, DriverError> {
        let payload = format!("{log_type},{}", if clear { 1 } else { 0 });
        let frame = encode_service_command(52, &payload);
        tokio::time::timeout(self.timeout, self.stream.write_all(&frame))
            .await
            .map_err(|_| DriverError::Timeout)??;
        tokio::time::timeout(self.timeout, self.stream.flush())
            .await
            .map_err(|_| DriverError::Timeout)??;

        let mut entries = Vec::new();
        loop {
            let (is_err, code, data) = self.read_encrypted_frame().await?;
            if is_err {
                return Err(DriverError::Protocol {
                    code: code.parse::<i32>().ok(),
                    message: format!("READ_DIAGNOSTIC_LOG error {code}: {data}"),
                });
            }
            let parts: Vec<&str> = data.splitn(3, ',').collect();
            let cur: u32 = parts
                .first()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1);
            let tot: u32 = parts
                .get(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1);
            entries.push(parts.get(2).copied().unwrap_or(&data).to_string());
            if cur >= tot {
                break;
            }
        }
        Ok(entries)
    }

    pub async fn reboot(&mut self) -> Result<String, DriverError> {
        self.send(53, "").await
    }

    pub async fn read_display_bitmap(&mut self) -> Result<LcdFrame, DriverError> {
        let raw = self.send(56, "").await?;
        parse_lcd_bitmap_reply(&raw)
    }

    pub async fn write_keystroke(
        &mut self,
        scan_code: u8,
        key_state: u8,
    ) -> Result<String, DriverError> {
        self.send(57, &format!("{scan_code},{key_state}")).await
    }

    pub async fn read_scale_settings(&mut self) -> Result<ScaleSettings748, DriverError> {
        let raw = self.send(62, "").await?;
        ScaleSettings748::parse(&raw)
    }

    pub async fn write_scale_settings(
        &mut self,
        settings: &ScaleSettings748,
    ) -> Result<String, DriverError> {
        self.send(63, &settings.format()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn crypt_is_symmetric_and_resets_cleanly() {
        let msg = b"\x02SelectCommand:14:\x03";
        let mut enc = XsServiceCrypt::new();
        let cipher = enc.transform_slice(msg);
        assert_ne!(cipher, msg);

        let mut dec = XsServiceCrypt::new();
        let plain = dec.transform_slice(&cipher);
        assert_eq!(plain, msg);
    }

    #[test]
    fn parses_scale_settings_748_and_lcd_bitmap() {
        let raw = "0001,8610011020214130,0020020110111000,0110110100111000,0100130010003200";
        let parsed = ScaleSettings748::parse(raw).unwrap();
        assert_eq!(parsed.version, "0001");
        assert_eq!(parsed.bank4, "0100130010003200");
        assert_eq!(parsed.format(), raw);

        let lcd = parse_lcd_bitmap_reply("320 x 128,1,FF00AA55").unwrap();
        assert_eq!(lcd.width, 320);
        assert_eq!(lcd.height, 128);
        assert_eq!(lcd.bitmap, vec![0xFF, 0x00, 0xAA, 0x55]);
    }

    #[tokio::test]
    async fn service_client_roundtrip_against_mock_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1];

            loop {
                let mut dec = XsServiceCrypt::new();
                let mut plain = Vec::new();
                while socket.read_exact(&mut buf).await.is_ok() {
                    let b = dec.transform_byte(buf[0]);
                    plain.push(b);
                    if b == ETX {
                        break;
                    }
                }
                if plain.is_empty() {
                    break;
                }
                let text: String = plain
                    .into_iter()
                    .filter(|&b| b != STX && b != ETX)
                    .map(char::from)
                    .collect();

                let reply = if text.starts_with("SelectCommand:14:") {
                    "\x02ReplyCommand:14:0.250,04/08,0.000,Balance,\x03"
                } else if text.starts_with("SelectCommand:62:") {
                    "\x02ReplyCommand:62:0001,8610011020214130,0020020110111000,0110110100111000,0100130010003200\x03"
                } else {
                    "\x02ReplyCommand:00:OK\x03"
                };

                let mut enc = XsServiceCrypt::new();
                let out = enc.transform_slice(reply.as_bytes());
                socket.write_all(&out).await.unwrap();
            }
        });

        let mut client = XsServiceClient::connect("127.0.0.1", Some(port), Duration::from_secs(2))
            .await
            .unwrap();

        let weight = client.read_weight().await.unwrap();
        assert_eq!(weight.weight_display, "0.250");
        assert_eq!(weight.status, "Balance");

        let cfg = client.read_scale_settings().await.unwrap();
        assert_eq!(cfg.version, "0001");
        assert_eq!(client.write_keystroke(12, 1).await.unwrap(), "OK");
    }
}
