pub mod codec;
pub mod discovery;
pub mod driver;
pub mod protocol;

pub use discovery::{XTI_ENQ_PAYLOAD, discover_xti_scales, parse_xti_discovery_xml};
pub use driver::{XtiDriver, XtiDriverConfig};
pub use protocol::{
    ResponseBatch, XTI_DATA_PORT, XTI_DISCOVERY_PORT, XtiBanner, encode_instruction,
    parse_xml_fields,
};
