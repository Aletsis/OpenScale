use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use openscale_domain::ScaleFamily;
use openscale_driver::{DiscoveredScale, DriverError};
use tokio::net::UdpSocket;

use crate::protocol::{XTI_DATA_PORT, XTI_DISCOVERY_PORT, parse_xml_fields};

pub const XTI_ENQ_PAYLOAD: &[u8] = b":AVERYBERKELXTRAENQ:";

/// Parses the XML UDP discovery response from an XTi / XTRA scale.
pub fn parse_xti_discovery_xml(xml: &str, fallback_ip: IpAddr) -> Option<DiscoveredScale> {
    if !xml.contains("<Field") {
        return None;
    }
    let fields = parse_xml_fields(xml);
    if fields.is_empty() {
        return None;
    }

    let ip_address = fields
        .get("IPAddress")
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| fallback_ip.to_string());

    let data_port = fields
        .get("HostPort")
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(XTI_DATA_PORT);

    Some(DiscoveredScale {
        family: ScaleFamily::AveryBerkelXti,
        ip_address,
        data_port,
        service_port: None,
        model: fields.get("ModelNumber").cloned(),
        serial_number: fields.get("SerialNumber").cloned(),
        mac_address: fields.get("MACAddress").cloned(),
        firmware_version: fields.get("ScaleVersion").cloned(),
    })
}

/// Discovers XTi / XTRA scales on the LAN via UDP port 23453 (broadcast + optional unicast targets).
pub async fn discover_xti_scales(
    timeout: Duration,
    extra_targets: &[SocketAddr],
) -> Result<Vec<DiscoveredScale>, DriverError> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    socket.set_broadcast(true)?;

    let broadcast_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), XTI_DISCOVERY_PORT);
    let _ = socket.send_to(XTI_ENQ_PAYLOAD, broadcast_addr).await;
    for &target in extra_targets {
        let _ = socket.send_to(XTI_ENQ_PAYLOAD, target).await;
    }

    let mut found: HashMap<String, DiscoveredScale> = HashMap::new();
    let deadline = tokio::time::Instant::now() + timeout;
    let mut buf = [0u8; 2048];

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await {
            Ok(Ok((n, peer))) => {
                let xml = String::from_utf8_lossy(&buf[..n]);
                if let Some(scale) = parse_xti_discovery_xml(&xml, peer.ip()) {
                    found.entry(scale.ip_address.clone()).or_insert(scale);
                }
            }
            Ok(Err(err)) => return Err(DriverError::Io(err)),
            Err(_) => break,
        }
    }

    let mut list: Vec<DiscoveredScale> = found.into_values().collect();
    list.sort_by(|a, b| a.ip_address.cmp(&b.ip_address));
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_xti_xml_udp_reply() {
        let xml = r#"<Response>
            <Field name="ScaleType">XT</Field>
            <Field name="ModelNumber">XTi400</Field>
            <Field name="SerialNumber">12345678</Field>
            <Field name="IPAddress">192.168.9.120</Field>
            <Field name="MACAddress">00:E0:8A:11:22:33</Field>
            <Field name="HostPort">3002</Field>
            <Field name="ScaleVersion">5.3.0.0</Field>
        </Response>"#;

        let parsed = parse_xti_discovery_xml(xml, IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
        assert_eq!(parsed.family, ScaleFamily::AveryBerkelXti);
        assert_eq!(parsed.ip_address, "192.168.9.120");
        assert_eq!(parsed.data_port, 3002);
        assert_eq!(parsed.model.as_deref(), Some("XTi400"));
        assert_eq!(parsed.serial_number.as_deref(), Some("12345678"));
        assert_eq!(parsed.mac_address.as_deref(), Some("00:E0:8A:11:22:33"));
    }
}
