use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use openscale_domain::ScaleFamily;
use openscale_driver::{DiscoveredScale, DriverError};
use tokio::net::UdpSocket;

use crate::protocol::{XS_DATA_PORT, XS_DISCOVERY_PORT, XS_SERVICE_PORT};

pub const XS_ENQ_PAYLOAD: &[u8] = b":AVERYBERKELENQ:";

/// Parses the 112-byte ASCII discovery reply from an XS / M-Series scale:
/// `Hello from X LPC1833 [ Xs Series ABR30-000403 APP. v4.1.7.0 ]  Scale Port = 25000  (192.168.9.113)  {ASSET_ID}`
pub fn parse_discovery_response(text: &str, fallback_ip: IpAddr) -> Option<DiscoveredScale> {
    if !text.contains("Hello from") {
        return None;
    }

    let bracket_content = text
        .split_once('[')
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(inner, _)| inner.trim());

    let (model, firmware_version) = match bracket_content {
        Some(inner) => {
            if let Some((left, right)) = inner.split_once("APP.") {
                (
                    Some(left.trim().to_string()),
                    Some(format!("APP.{}", right.trim_end())),
                )
            } else {
                (Some(inner.to_string()), None)
            }
        }
        None => (None, None),
    };

    let ip_address = text
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(ip, _)| ip.trim().to_string())
        .filter(|ip| ip.parse::<IpAddr>().is_ok())
        .unwrap_or_else(|| fallback_ip.to_string());

    Some(DiscoveredScale {
        family: ScaleFamily::AveryBerkelXs,
        ip_address,
        data_port: XS_DATA_PORT,
        service_port: Some(XS_SERVICE_PORT),
        model,
        serial_number: None,
        mac_address: None,
        firmware_version,
    })
}

/// Discovers XS / M-Series scales on the LAN via UDP port 23452 (broadcast + optional unicast targets).
pub async fn discover_xs_scales(
    timeout: Duration,
    extra_targets: &[SocketAddr],
) -> Result<Vec<DiscoveredScale>, DriverError> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    socket.set_broadcast(true)?;

    let broadcast_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), XS_DISCOVERY_PORT);
    let _ = socket.send_to(XS_ENQ_PAYLOAD, broadcast_addr).await;
    for &target in extra_targets {
        let _ = socket.send_to(XS_ENQ_PAYLOAD, target).await;
    }

    let mut found: HashMap<String, DiscoveredScale> = HashMap::new();
    let deadline = tokio::time::Instant::now() + timeout;
    let mut buf = [0u8; 512];

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await {
            Ok(Ok((n, peer))) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                if let Some(scale) = parse_discovery_response(&text, peer.ip()) {
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
    fn parses_real_xs200_udp_response() {
        let raw = "Hello from X LPC1833 [ Xs Series ABR30-000403 APP. v4.1.7.0 ]  Scale Port = 25000  (192.168.9.113)  {ASSET_ID}";
        let parsed = parse_discovery_response(raw, IpAddr::V4(Ipv4Addr::new(192, 168, 9, 113)))
            .expect("should parse real XS response");

        assert_eq!(parsed.family, ScaleFamily::AveryBerkelXs);
        assert_eq!(parsed.ip_address, "192.168.9.113");
        assert_eq!(parsed.data_port, 3001);
        assert_eq!(parsed.service_port, Some(28822));
        assert_eq!(parsed.model.as_deref(), Some("Xs Series ABR30-000403"));
        assert_eq!(parsed.firmware_version.as_deref(), Some("APP. v4.1.7.0"));
    }

    #[test]
    fn ignores_non_xs_packets() {
        assert!(
            parse_discovery_response("random packet", IpAddr::V4(Ipv4Addr::LOCALHOST)).is_none()
        );
    }
}
