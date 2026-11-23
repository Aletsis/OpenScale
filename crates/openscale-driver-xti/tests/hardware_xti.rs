//! Opt-in non-destructive integration tests against a physical Avery Berkel XTi scale.
//! Run with: `OPENSCALE_TEST_XTI_IP=192.168.9.120 cargo test -p openscale-driver-xti --test hardware_xti -- --nocapture`

use openscale_domain::types::new_id;
use openscale_driver::ScaleDriver;
use openscale_driver_xti::{XtiDriver, XtiDriverConfig};

#[tokio::test]
async fn live_xti_read_only_smoke_test() {
    let Ok(ip) = std::env::var("OPENSCALE_TEST_XTI_IP") else {
        return; // Skipped when no physical scale IP is configured
    };

    let mut driver = XtiDriver::new(XtiDriverConfig::new(ip, new_id()));
    let info = driver
        .connect()
        .await
        .expect("connect to physical XTi scale");
    println!("Raw Banner XML:\n{}", driver.banner().unwrap().raw_xml);
    println!("Connected XTi: {info:?}");

    driver.ping().await.expect("ping XTi");
    let plus = driver.list_plus(None, None).await.expect("list XTi PLUs");
    println!("XTi PLU count: {}", plus.len());
    if let Some(first) = plus.first() {
        println!(
            "First PLU: DID={} PLU={} desc={:?} P1={} ean={:?}",
            first.key.did, first.key.plu, first.description, first.price_1.0, first.ean
        );
    }

    driver.disconnect().await.expect("disconnect XTi");
}
