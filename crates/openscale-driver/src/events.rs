use chrono::{DateTime, Utc};
use openscale_domain::ScaleFamily;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredScale {
    pub family: ScaleFamily,
    pub ip_address: String,
    pub data_port: u16,
    pub service_port: Option<u16>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub mac_address: Option<String>,
    pub firmware_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleInfo {
    pub family: ScaleFamily,
    pub model: String,
    pub serial_number: Option<String>,
    pub firmware_version: String,
    pub protocol_version: Option<String>,
    pub mac_address: Option<String>,
    pub requires_auth: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleDateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveWeight {
    pub gross_grams: i64,
    pub tare_grams: i64,
    pub net_grams: i64,
    pub is_stable: bool,
    pub is_zero: bool,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcdFrame {
    pub width: u16,
    pub height: u16,
    /// Raw 1-bpp packed bitmap bytes (e.g. 320x128 -> 5120 bytes).
    pub bitmap: Vec<u8>,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScaleEvent {
    ConnectionChanged {
        scale_id: Uuid,
        state: ConnectionState,
        message: Option<String>,
    },
    LiveWeightUpdated {
        scale_id: Uuid,
        weight: LiveWeight,
    },
    LcdFrameUpdated {
        scale_id: Uuid,
        frame: LcdFrame,
    },
    IdentityConflict {
        scale_id: Uuid,
        ip_address: String,
        expected_serial: String,
        actual_serial: String,
    },
}
