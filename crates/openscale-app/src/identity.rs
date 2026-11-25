use openscale_domain::Scale;
use openscale_driver::DiscoveredScale;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScaleIdentityStatus {
    /// Scale is not registered yet and its IP is free.
    NewScale,
    /// Scale matches an existing registered scale (same IP and identity).
    Matched { scale_id: Uuid, scale_name: String },
    /// A registered scale (matched by serial number or MAC) is now responding on a different IP.
    IpChanged {
        scale_id: Uuid,
        scale_name: String,
        previous_ip: String,
        new_ip: String,
        serial_number: Option<String>,
        mac_address: Option<String>,
    },
    /// An IP registered to scale A is now responding with a different serial/MAC (either another
    /// registered scale B or an unregistered scale).
    IpTakenByOtherScale {
        existing_scale_id: Uuid,
        existing_scale_name: String,
        expected_serial: Option<String>,
        actual_serial: Option<String>,
        ip_address: String,
        other_registered_scale_id: Option<Uuid>,
        other_registered_scale_name: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredScaleMatch {
    pub discovered: DiscoveredScale,
    pub identity: ScaleIdentityStatus,
}

fn norm_opt(val: Option<&str>) -> Option<String> {
    val.map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty())
}

fn identity_matches(discovered: &DiscoveredScale, scale: &Scale) -> bool {
    let d_sn = norm_opt(discovered.serial_number.as_deref());
    let s_sn = norm_opt(scale.serial_number.as_deref());
    if let (Some(a), Some(b)) = (&d_sn, &s_sn) {
        return a == b;
    }

    let d_mac = norm_opt(discovered.mac_address.as_deref());
    let s_mac = norm_opt(scale.mac_address.as_deref());
    if let (Some(a), Some(b)) = (&d_mac, &s_mac) {
        return a == b;
    }

    false
}

fn has_conflicting_identity(discovered: &DiscoveredScale, scale: &Scale) -> bool {
    let d_sn = norm_opt(discovered.serial_number.as_deref());
    let s_sn = norm_opt(scale.serial_number.as_deref());
    if let (Some(a), Some(b)) = (&d_sn, &s_sn) {
        return a != b;
    }

    let d_mac = norm_opt(discovered.mac_address.as_deref());
    let s_mac = norm_opt(scale.mac_address.as_deref());
    if let (Some(a), Some(b)) = (&d_mac, &s_mac) {
        return a != b;
    }

    false
}

/// Classifies each discovered LAN scale against the registered inventory to detect:
/// - New scales
/// - Exact matches
/// - Registered scales whose IP address changed (matched by serial number or MAC)
/// - IP collisions where a different physical scale is now answering on a registered IP
pub fn classify_discovered_scales(
    discovered: &[DiscoveredScale],
    registered: &[Scale],
) -> Vec<DiscoveredScaleMatch> {
    discovered
        .iter()
        .map(|cand| {
            let by_identity = registered.iter().find(|s| identity_matches(cand, s));
            let by_ip = registered
                .iter()
                .find(|s| s.ip_address == cand.ip_address && s.data_port == cand.data_port);

            let identity = match (by_ip, by_identity) {
                // Same IP and either same serial/MAC or no conflicting serial/MAC
                (Some(ip_scale), Some(id_scale)) if ip_scale.id == id_scale.id => {
                    ScaleIdentityStatus::Matched {
                        scale_id: ip_scale.id,
                        scale_name: ip_scale.name.clone(),
                    }
                }
                // IP belongs to scale A, but the physical scale answering is registered scale B!
                (Some(ip_scale), Some(id_scale)) => ScaleIdentityStatus::IpTakenByOtherScale {
                    existing_scale_id: ip_scale.id,
                    existing_scale_name: ip_scale.name.clone(),
                    expected_serial: ip_scale.serial_number.clone(),
                    actual_serial: cand.serial_number.clone(),
                    ip_address: cand.ip_address.clone(),
                    other_registered_scale_id: Some(id_scale.id),
                    other_registered_scale_name: Some(id_scale.name.clone()),
                },
                // IP belongs to scale A, and answering scale is NOT matched in DB:
                // check whether serial/MAC explicitly conflicts with scale A.
                (Some(ip_scale), None) => {
                    if has_conflicting_identity(cand, ip_scale) {
                        ScaleIdentityStatus::IpTakenByOtherScale {
                            existing_scale_id: ip_scale.id,
                            existing_scale_name: ip_scale.name.clone(),
                            expected_serial: ip_scale.serial_number.clone(),
                            actual_serial: cand.serial_number.clone(),
                            ip_address: cand.ip_address.clone(),
                            other_registered_scale_id: None,
                            other_registered_scale_name: None,
                        }
                    } else {
                        ScaleIdentityStatus::Matched {
                            scale_id: ip_scale.id,
                            scale_name: ip_scale.name.clone(),
                        }
                    }
                }
                // IP is not registered, but serial/MAC matches an existing scale -> IP changed!
                (None, Some(id_scale)) => ScaleIdentityStatus::IpChanged {
                    scale_id: id_scale.id,
                    scale_name: id_scale.name.clone(),
                    previous_ip: id_scale.ip_address.clone(),
                    new_ip: cand.ip_address.clone(),
                    serial_number: cand
                        .serial_number
                        .clone()
                        .or_else(|| id_scale.serial_number.clone()),
                    mac_address: cand
                        .mac_address
                        .clone()
                        .or_else(|| id_scale.mac_address.clone()),
                },
                // Neither IP nor serial/MAC is registered -> brand new scale
                (None, None) => ScaleIdentityStatus::NewScale,
            };

            DiscoveredScaleMatch {
                discovered: cand.clone(),
                identity,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openscale_domain::types::new_id;
    use openscale_domain::{ScaleFamily, ScaleStatus, SyncMetadata};

    fn make_scale(name: &str, ip: &str, serial: Option<&str>) -> Scale {
        Scale {
            id: new_id(),
            branch_id: None,
            group_id: None,
            name: name.to_string(),
            family: ScaleFamily::AveryBerkelXs,
            ip_address: ip.to_string(),
            data_port: 3001,
            service_port: Some(28822),
            serial_number: serial.map(str::to_string),
            model: Some("XS200".to_string()),
            firmware_version: Some("APP. v4.1.7.0".to_string()),
            mac_address: None,
            status: ScaleStatus::Online,
            last_seen_at: None,
            meta: SyncMetadata::new(new_id()),
        }
    }

    fn make_discovered(ip: &str, serial: Option<&str>) -> DiscoveredScale {
        DiscoveredScale {
            family: ScaleFamily::AveryBerkelXs,
            ip_address: ip.to_string(),
            data_port: 3001,
            service_port: Some(28822),
            model: Some("XS200".to_string()),
            serial_number: serial.map(str::to_string),
            mac_address: None,
            firmware_version: Some("APP. v4.1.7.0".to_string()),
        }
    }

    #[test]
    fn detects_ip_change_ip_takeover_and_new_scale() {
        let s1 = make_scale("Carnicería 1", "192.168.9.111", Some("22320310"));
        let s2 = make_scale("Carnicería 2", "192.168.9.113", Some("24310206"));
        let registered = vec![s1.clone(), s2.clone()];

        let discovered = vec![
            // s1 moved to a new IP (.150)
            make_discovered("192.168.9.150", Some("22320310")),
            // s2 is now answering on s1's old IP (.111) -> IpTakenByOtherScale referencing s2
            make_discovered("192.168.9.111", Some("24310206")),
            // unknown serial answering on s2's IP (.113) -> IpTakenByOtherScale (unregistered)
            make_discovered("192.168.9.113", Some("99999999")),
            // completely new scale
            make_discovered("192.168.9.199", Some("77777777")),
        ];

        let results = classify_discovered_scales(&discovered, &registered);
        assert_eq!(results.len(), 4);

        assert!(matches!(
            &results[0].identity,
            ScaleIdentityStatus::IpChanged {
                scale_id,
                previous_ip,
                new_ip,
                ..
            } if *scale_id == s1.id && previous_ip == "192.168.9.111" && new_ip == "192.168.9.150"
        ));

        assert!(matches!(
            &results[1].identity,
            ScaleIdentityStatus::IpTakenByOtherScale {
                existing_scale_id,
                other_registered_scale_id,
                ..
            } if *existing_scale_id == s1.id && *other_registered_scale_id == Some(s2.id)
        ));

        assert!(matches!(
            &results[2].identity,
            ScaleIdentityStatus::IpTakenByOtherScale {
                existing_scale_id,
                other_registered_scale_id: None,
                actual_serial: Some(actual),
                ..
            } if *existing_scale_id == s2.id && actual == "99999999"
        ));

        assert!(matches!(results[3].identity, ScaleIdentityStatus::NewScale));
    }
}
