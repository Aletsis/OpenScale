use std::net::SocketAddr;
use std::time::Duration;

use openscale_driver::DiscoveredScale;
use openscale_driver_xs::discover_xs_scales;
use openscale_driver_xti::discover_xti_scales;

use crate::error::AppError;

/// Runs Avery Berkel XS (UDP 23452) and XTi (UDP 23453) LAN discovery concurrently.
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

    found.sort_by(|a, b| a.ip_address.cmp(&b.ip_address));
    Ok(found)
}
