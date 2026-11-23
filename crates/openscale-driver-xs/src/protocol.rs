use bytes::{BufMut, BytesMut};
use encoding_rs::WINDOWS_1252;
use openscale_driver::DriverError;

pub const XS_DATA_PORT: u16 = 3001;
pub const XS_SERVICE_PORT: u16 = 28822;
pub const XS_DISCOVERY_PORT: u16 = 23452;

pub const HEADER_LEN: usize = 51;
pub const MAX_DATA_BLOCK_SIZE: usize = 500;

pub const SOURCE_HOST: u8 = 99; // 0x63
pub const DEST_SELF: u8 = 253; // 0xFD
pub const FAMILY_M_SERIES: u8 = 32; // 0x20
pub const KIND_REQUEST: u8 = b'I'; // 73
pub const KIND_RESPONSE: u8 = b'R'; // 82
pub const CHANNEL_MAIN: u8 = 0;
pub const ENDIAN_NETWORK: u8 = b'N'; // 78

pub const ACTION_REQUEST: u8 = b'Q'; // 81
pub const ACTION_UPDATE: u8 = b'U'; // 85
pub const ACTION_CLEAR: u8 = b'C'; // 67

pub const CMD_CHANGE_PRICE: u16 = 5;
pub const CMD_PLU_RECORD: u16 = 13;
pub const CMD_DATE_AND_TIME: u16 = 16;
pub const CMD_VERSION_DETAILS: u16 = 59;
pub const CMD_ASSETINFO: u16 = 121;
pub const CMD_MACHNETCONFIG: u16 = 128;

pub const CVN_VERSION_DETAILS: u16 = 0;
pub const CVN_DATE_AND_TIME: u16 = 11;
pub const CVN_CHANGE_PRICE: u16 = 3400;
pub const CVN_PLU_RECORD: u16 = 7600;
pub const CVN_ASSETINFO: u16 = 8000;
pub const CVN_MACHNETCONFIG: u16 = 8000;

pub const STATUS_SUCCESS: i16 = 0;
pub const STATUS_BUSY: i16 = -1119;
pub const STATUS_PLU_NOT_FOUND: i16 = -1203;
pub const STATUS_PLU_LAST_PLU: i16 = -1205;

/// Fixed 51-byte Big-Endian `ABcmdheader` used by Avery Berkel XS / M-Series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbCmdHeader {
    pub cvn: u16,
    pub length: u16,
    pub source_id: u8,
    pub dest_id: u8,
    pub family: u8,
    pub command_or_response: u8,
    pub channel: u8,
    pub command_type: u16,
    pub action: u8,
    pub status: i16,
    pub block_num: u8,
    pub num_blocks: u8,
    pub endian: u8,
    pub send_sequence: u32,
}

impl AbCmdHeader {
    pub fn new_request(command_type: u16, action: u8, cvn: u16, send_sequence: u32) -> Self {
        Self {
            cvn,
            length: 19,
            source_id: SOURCE_HOST,
            dest_id: DEST_SELF,
            family: FAMILY_M_SERIES,
            command_or_response: KIND_REQUEST,
            channel: CHANNEL_MAIN,
            command_type,
            action,
            status: STATUS_SUCCESS,
            block_num: 1,
            num_blocks: 1,
            endian: ENDIAN_NETWORK,
            send_sequence,
        }
    }

    /// Encodes the 51-byte header with `length = 19 + payload_len`.
    pub fn encode(&self, payload_len: usize) -> Result<[u8; HEADER_LEN], DriverError> {
        let total_len = u16::try_from(19usize.saturating_add(payload_len)).map_err(|_| {
            DriverError::Protocol {
                code: None,
                message: format!("payload length {payload_len} exceeds u16 header limit"),
            }
        })?;

        let mut buf = BytesMut::with_capacity(HEADER_LEN);
        buf.put_bytes(0u8, 30); // ContBlock[30]
        buf.put_u16(self.cvn);
        buf.put_u16(total_len);
        buf.put_u8(self.source_id);
        buf.put_u8(self.dest_id);
        buf.put_u8(self.family);
        buf.put_u8(self.command_or_response);
        buf.put_u8(self.channel);
        buf.put_u16(self.command_type);
        buf.put_u8(self.action);
        buf.put_i16(self.status);
        buf.put_u8(self.block_num);
        buf.put_u8(self.num_blocks);
        buf.put_u8(self.endian);
        buf.put_u32(self.send_sequence);

        let mut out = [0u8; HEADER_LEN];
        out.copy_from_slice(&buf);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DriverError> {
        if bytes.len() < HEADER_LEN {
            return Err(DriverError::Protocol {
                code: None,
                message: format!(
                    "header too short: {} bytes (expected {HEADER_LEN})",
                    bytes.len()
                ),
            });
        }

        let length = u16::from_be_bytes([bytes[32], bytes[33]]);
        if length < 19 {
            return Err(DriverError::Protocol {
                code: None,
                message: format!("invalid header length field: {length} (< 19)"),
            });
        }

        Ok(Self {
            cvn: u16::from_be_bytes([bytes[30], bytes[31]]),
            length,
            source_id: bytes[34],
            dest_id: bytes[35],
            family: bytes[36],
            command_or_response: bytes[37],
            channel: bytes[38],
            command_type: u16::from_be_bytes([bytes[39], bytes[40]]),
            action: bytes[41],
            status: i16::from_be_bytes([bytes[42], bytes[43]]),
            block_num: bytes[44],
            num_blocks: bytes[45],
            endian: bytes[46],
            send_sequence: u32::from_be_bytes([bytes[47], bytes[48], bytes[49], bytes[50]]),
        })
    }

    pub fn payload_len(&self) -> usize {
        usize::from(self.length.saturating_sub(19))
    }
}

/// Splits a payload into `<= 500 B` chunks and returns one contiguous `[51 B header + chunk]`
/// buffer per block so each block can be sent in a single atomic `write_all()` call.
pub fn build_atomic_blocks(
    mut header: AbCmdHeader,
    payload: &[u8],
) -> Result<Vec<Vec<u8>>, DriverError> {
    if payload.is_empty() {
        header.block_num = 1;
        header.num_blocks = 1;
        let hdr = header.encode(0)?;
        return Ok(vec![hdr.to_vec()]);
    }

    let chunks: Vec<&[u8]> = payload.chunks(MAX_DATA_BLOCK_SIZE).collect();
    let num_blocks = u8::try_from(chunks.len()).map_err(|_| DriverError::Protocol {
        code: None,
        message: format!("payload requires {} blocks (> 255)", chunks.len()),
    })?;

    let mut frames = Vec::with_capacity(chunks.len());
    for (idx, chunk) in chunks.into_iter().enumerate() {
        header.block_num = (idx + 1) as u8;
        header.num_blocks = num_blocks;
        let hdr = header.encode(chunk.len())?;
        let mut frame = Vec::with_capacity(HEADER_LEN + chunk.len());
        frame.extend_from_slice(&hdr);
        frame.extend_from_slice(chunk);
        frames.push(frame);
    }
    Ok(frames)
}

/// Encodes a UTF-8 string into Windows-1252 bytes terminated by `0x00`.
pub fn encode_cp1252_cstr(input: &str) -> Vec<u8> {
    let (cow, _, _) = WINDOWS_1252.encode(input);
    let mut out = Vec::with_capacity(cow.len() + 1);
    for &b in cow.as_ref() {
        if b != 0 {
            out.push(b);
        }
    }
    out.push(0);
    out
}

/// Decodes a Windows-1252 slice up to the first NUL byte (`0x00`), trimming trailing spaces/NULs.
pub fn decode_cp1252_zstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let (cow, _, _) = WINDOWS_1252.decode(&bytes[..end]);
    cow.trim_end().to_string()
}

/// Splits a buffer of NUL-terminated Windows-1252 strings.
pub fn decode_cp1252_cstrs(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|&b| b == 0)
        .map(|slice| {
            let (cow, _, _) = WINDOWS_1252.decode(slice);
            cow.into_owned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_encode_decode_roundtrip() {
        let req = AbCmdHeader::new_request(CMD_PLU_RECORD, ACTION_REQUEST, CVN_PLU_RECORD, 42);
        let encoded = req.encode(41).unwrap();
        assert_eq!(encoded.len(), 51);
        assert_eq!(&encoded[0..30], &[0u8; 30]);
        assert_eq!(u16::from_be_bytes([encoded[30], encoded[31]]), 7600);
        assert_eq!(u16::from_be_bytes([encoded[32], encoded[33]]), 60); // 19 + 41
        assert_eq!(encoded[34], 99);
        assert_eq!(encoded[35], 253);
        assert_eq!(encoded[36], 32);
        assert_eq!(encoded[37], b'I');
        assert_eq!(encoded[41], b'Q');
        assert_eq!(encoded[46], b'N');

        let decoded = AbCmdHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.cvn, 7600);
        assert_eq!(decoded.payload_len(), 41);
        assert_eq!(decoded.command_type, CMD_PLU_RECORD);
        assert_eq!(decoded.send_sequence, 42);
    }

    #[test]
    fn atomic_blocks_split_at_500_bytes() {
        let req = AbCmdHeader::new_request(29, ACTION_UPDATE, 5200, 7);
        let payload = vec![0xAA; 898];
        let frames = build_atomic_blocks(req, &payload).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].len(), HEADER_LEN + 500);
        assert_eq!(frames[1].len(), HEADER_LEN + 398);

        let h1 = AbCmdHeader::decode(&frames[0]).unwrap();
        let h2 = AbCmdHeader::decode(&frames[1]).unwrap();
        assert_eq!((h1.block_num, h1.num_blocks, h1.payload_len()), (1, 2, 500));
        assert_eq!((h2.block_num, h2.num_blocks, h2.payload_len()), (2, 2, 398));
    }

    #[test]
    fn cp1252_handles_spanish_accents_and_nuls() {
        let encoded = encode_cp1252_cstr("JAMÓN ESPAÑOL");
        assert_eq!(encoded.last(), Some(&0));
        assert_eq!(decode_cp1252_zstr(&encoded), "JAMÓN ESPAÑOL");
    }
}
