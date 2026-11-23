use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use openscale_app::{OpenScaleApp, SyncMode};
use openscale_domain::types::new_id;
use openscale_domain::{
    MoneyCents, Plu, PluKey, ProductType, Scale, ScaleFamily, ScaleStatus, SyncMetadata,
};
use openscale_driver::{
    ConnectionState, DriverCapabilities, DriverError, ScaleDateTime, ScaleDriver, ScaleEvent,
    ScaleInfo,
};
use openscale_storage::{DEFAULT_LOCAL_STORE_ID, Database};

#[derive(Default)]
struct InMemoryDriverState {
    plus: BTreeMap<PluKey, Plu>,
    upserts: usize,
}

struct MockScaleDriver {
    state: Arc<Mutex<InMemoryDriverState>>,
}

#[async_trait]
impl ScaleDriver for MockScaleDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities::xs_default()
    }

    async fn connect(&mut self) -> Result<ScaleInfo, DriverError> {
        Ok(ScaleInfo {
            family: ScaleFamily::AveryBerkelXs,
            model: "XS200".to_string(),
            serial_number: Some("MOCK-01".to_string()),
            firmware_version: "4.1.7.0".to_string(),
            protocol_version: Some("8000".to_string()),
            mac_address: None,
            requires_auth: false,
        })
    }

    async fn disconnect(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    async fn get_info(&mut self) -> Result<ScaleInfo, DriverError> {
        self.connect().await
    }

    async fn get_datetime(&mut self) -> Result<ScaleDateTime, DriverError> {
        Ok(ScaleDateTime {
            year: 2026,
            month: 11,
            day: 22,
            hour: 18,
            minute: 0,
            second: 0,
        })
    }

    async fn set_datetime(&mut self, _dt: ScaleDateTime) -> Result<(), DriverError> {
        Ok(())
    }

    async fn read_plu(&mut self, key: PluKey) -> Result<Option<Plu>, DriverError> {
        Ok(self.state.lock().unwrap().plus.get(&key).cloned())
    }

    async fn list_plus(
        &mut self,
        _from: Option<PluKey>,
        _to: Option<PluKey>,
    ) -> Result<Vec<Plu>, DriverError> {
        Ok(self.state.lock().unwrap().plus.values().cloned().collect())
    }

    async fn count_plus(&mut self) -> Result<u64, DriverError> {
        Ok(self.state.lock().unwrap().plus.len() as u64)
    }

    async fn upsert_plu(&mut self, plu: &Plu) -> Result<(), DriverError> {
        let mut st = self.state.lock().unwrap();
        st.upserts += 1;
        st.plus.insert(plu.key, plu.clone());
        Ok(())
    }

    async fn change_price(
        &mut self,
        key: PluKey,
        price_1: MoneyCents,
        _price_2: Option<MoneyCents>,
        _price_3: Option<MoneyCents>,
    ) -> Result<(), DriverError> {
        if let Some(plu) = self.state.lock().unwrap().plus.get_mut(&key) {
            plu.price_1 = price_1;
        }
        Ok(())
    }

    async fn delete_plu(&mut self, key: PluKey) -> Result<(), DriverError> {
        self.state.lock().unwrap().plus.remove(&key);
        Ok(())
    }
}

fn sample_plu(plu_num: u64, desc: &str, cents: i64) -> Plu {
    let mut plu = Plu {
        id: new_id(),
        key: PluKey::new(1, plu_num).unwrap(),
        description: desc.to_string(),
        button_text: None,
        price_1: MoneyCents(cents),
        price_2: MoneyCents(0),
        price_3: MoneyCents(0),
        product_type: ProductType::ByWeight,
        ean: None,
        sku: None,
        group_id: 0,
        barcode_format_id: 1,
        label_format_id: 1,
        tare_id: Some(0),
        tare_grams: None,
        date_offset_1: 5,
        date_print_control_1: 1,
        date_offset_2: 0,
        date_print_control_2: 0,
        tax_rate_id_1: 0,
        tax_rate_id_2: 0,
        text_areas: vec![],
        delta_hash: String::new(),
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };
    plu.delta_hash = plu.compute_delta_hash();
    plu
}

#[tokio::test]
async fn scale_manager_and_delta_sync_lifecycle() {
    let db = Database::connect_in_memory().await.unwrap();
    let app = OpenScaleApp::new(db);

    let scale_id = new_id();
    let scale = Scale {
        id: scale_id,
        group_id: None,
        name: "Carnicería XS".to_string(),
        family: ScaleFamily::AveryBerkelXs,
        ip_address: "192.168.9.113".to_string(),
        data_port: 3001,
        service_port: Some(28822),
        serial_number: Some("22320310".to_string()),
        model: Some("XS200".to_string()),
        firmware_version: Some("4.1.7.0".to_string()),
        mac_address: None,
        status: ScaleStatus::Unknown,
        last_seen_at: None,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };
    app.scale_repo().upsert(&scale).await.unwrap();

    // Seed mock remote scale with 2 PLUs
    let state = Arc::new(Mutex::new(InMemoryDriverState::default()));
    {
        let mut st = state.lock().unwrap();
        let p1 = sample_plu(1, "POLLO ENTERO", 6500);
        let p2 = sample_plu(2, "PECHUGA", 11000);
        st.plus.insert(p1.key, p1);
        st.plus.insert(p2.key, p2);
    }

    let mut events = app.scales.subscribe();
    let handle = app
        .scales
        .register_driver(
            scale_id,
            Box::new(MockScaleDriver {
                state: Arc::clone(&state),
            }),
        )
        .await;

    let info = handle.connect().await.unwrap();
    assert_eq!(info.model, "XS200");
    assert!(matches!(
        events.recv().await.unwrap(),
        ScaleEvent::ConnectionChanged {
            state: ConnectionState::Connecting,
            ..
        }
    ));
    assert!(matches!(
        events.recv().await.unwrap(),
        ScaleEvent::ConnectionChanged {
            state: ConnectionState::Connected,
            ..
        }
    ));

    // 1. Import from scale -> SQLite catalog now has 2 PLUs and matching checksums
    let imported = app.sync.import_from_scale(scale_id).await.unwrap();
    assert_eq!(imported, 2);
    assert_eq!(app.list_plus().await.unwrap().len(), 2);

    // 2. Delta sync immediately -> 0 sent, 2 skipped
    let report1 = app.sync_scale(scale_id, SyncMode::Delta).await.unwrap();
    assert_eq!(report1.sent_plus, 0);
    assert_eq!(report1.skipped_unchanged, 2);
    assert_eq!(state.lock().unwrap().upserts, 0);

    // 3. Edit 1 PLU price locally + add 1 new PLU -> Delta sync sends ONLY those 2
    let mut edited = app
        .get_plu(PluKey::new(1, 1).unwrap())
        .await
        .unwrap()
        .unwrap();
    edited.price_1 = MoneyCents(6800);
    app.upsert_plu(edited).await.unwrap();
    app.upsert_plu(sample_plu(3, "ALITA ADOBADA", 7000))
        .await
        .unwrap();

    let report2 = app.sync_scale(scale_id, SyncMode::Delta).await.unwrap();
    assert_eq!(report2.total_catalog_plus, 3);
    assert_eq!(report2.sent_plus, 2);
    assert_eq!(report2.skipped_unchanged, 1);
    assert_eq!(state.lock().unwrap().upserts, 2);

    // 4. Full sync forces sending all 3 PLUs
    let report3 = app.sync_scale(scale_id, SyncMode::Full).await.unwrap();
    assert_eq!(report3.sent_plus, 3);
    assert_eq!(report3.skipped_unchanged, 0);
    assert_eq!(state.lock().unwrap().upserts, 5);

    app.scales.unregister(scale_id).await;
}
