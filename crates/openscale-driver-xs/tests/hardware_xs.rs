//! Opt-in non-destructive integration tests against a physical Avery Berkel XS scale.
//! Run with: `OPENSCALE_TEST_XS_IP=192.168.9.113 cargo test -p openscale-driver-xs --test hardware_xs -- --nocapture`

use openscale_domain::types::new_id;
use openscale_driver::ScaleDriver;
use openscale_driver_xs::{XsDriver, XsDriverConfig};

#[tokio::test]
async fn live_xs_read_only_smoke_test() {
    let Ok(ip) = std::env::var("OPENSCALE_TEST_XS_IP") else {
        return; // Skipped when no physical scale IP is configured
    };

    let mut driver = XsDriver::new(XsDriverConfig::new(ip, new_id()));
    let info = driver
        .connect()
        .await
        .expect("connect to physical XS scale");
    println!("Connected XS: {info:?}");

    driver.ping().await.expect("ping XS");
    let dt = driver.get_datetime().await.expect("read XS datetime");
    println!("XS DateTime: {dt:?}");

    let count = driver.count_plus().await.expect("O(log N) count_plus");
    println!("XS PLU count: {count}");

    driver.disconnect().await.expect("disconnect XS");
}

#[tokio::test]
async fn live_xs_service_port_weight_and_lcd_test() {
    let Ok(ip) = std::env::var("OPENSCALE_TEST_XS_IP") else {
        return;
    };

    let mut client =
        openscale_driver_xs::XsServiceClient::connect(&ip, None, std::time::Duration::from_secs(5))
            .await
            .expect("connect XsServicePort 28822");

    let weight = client.read_weight().await.expect("read_weight CMD 14");
    println!("XS Live Weight: {weight:?}");

    let lcd = client
        .read_display_bitmap()
        .await
        .expect("read_display_bitmap CMD 56");
    println!(
        "XS LCD Frame: {}x{}, {} bytes (first 16: {:02X?})",
        lcd.width,
        lcd.height,
        lcd.bitmap.len(),
        &lcd.bitmap[..lcd.bitmap.len().min(16)]
    );
}
