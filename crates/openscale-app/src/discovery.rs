use std::net::SocketAddr;
use std::time::Duration;

use openscale_domain::ScaleFamily;
use openscale_driver::{DiscoveredScale, ScaleDriver};
use openscale_driver_xs::{XsDriver, XsDriverConfig, discover_xs_scales};
use openscale_driver_xti::discover_xti_scales;
use openscale_storage::DEFAULT_LOCAL_STORE_ID;
use tokio::task::JoinSet;

use crate::error::AppError;

/// Runs Avery Berkel XS (UDP 23452) and XTi (UDP 23453) LAN discovery concurrently,
/// and enriches XS scales over TCP 3001 (`ASSETINFO` + `MACHNETCONFIG`) to populate
/// `serial_number`, `mac_address`, and exact `model` for IP/identity tracking.
pub async fn discover_all_scales(
    timeout: Duration,
    extra_xs_targets: &[SocketAddr],
    extra_xti_targets: &[SocketAddr],
) -> Result<Vec<DiscoveredScale>, AppError> {
    let (xs_res, xti_res) = tokio::join!(
        discover_xs_scales(timeout, extra_xs_targets),
        discover_xti_scales(timeout, extra_xti_targets),
    );

    let mut found = Vec::new();
    if let Ok(xs) = xs_res {
        found.extend(xs);
    }
    if let Ok(xti) = xti_res {
        found.extend(xti);
    }

    // Concurrently enrich XS scales missing serial_number via a short TCP 3001 handshake.
    let mut join_set = JoinSet::new();
    for (idx, item) in found.iter().enumerate() {
        if item.family == ScaleFamily::AveryBerkelXs && item.serial_number.is_none() {
            let ip = item.ip_address.clone();
            let port = item.data_port;
            join_set.spawn(async move {
                let mut cfg = XsDriverConfig::new(ip, DEFAULT_LOCAL_STORE_ID);
                cfg.port = port;
                cfg.io_timeout = Duration::from_millis(1500);
                let mut driver = XsDriver::new(cfg);
                let info = driver.connect().await.ok();
                let _ = driver.disconnect().await;
                (idx, info)
            });
        }
    }

    while let Some(joined) = join_set.join_next().await {
        if let Ok((idx, Some(info))) = joined
            && let Some(item) = found.get_mut(idx)
        {
            if info
                .serial_number
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
            {
                item.serial_number = info.serial_number;
            }
            if info
                .mac_address
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
            {
                item.mac_address = info.mac_address;
            }
            if !info.model.trim().is_empty() {
                item.model = Some(info.model);
            }
            if !info.firmware_version.trim().is_empty() {
                item.firmware_version = Some(info.firmware_version);
            }
        }
    }

    found.sort_by(|a, b| a.ip_address.cmp(&b.ip_address));
    Ok(found)
}
