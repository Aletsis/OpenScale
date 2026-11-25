use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use openscale_app::{
    ImportCategory, ImportSelection, OpenScaleApp, ScaleImportApplyRequest, SyncMode,
};
use openscale_domain::types::new_id;
use openscale_domain::{
    BarcodeFormat, Department, DeptTotals, GrandTotals, LabelFormat, MachTotals, MoneyCents, Plu,
    PluKey, PluTotals, ProductType, Scale, ScaleFamily, ScaleStatus, StoredText, SyncMetadata,
    Tare, TaxRate,
};
use openscale_driver::{
    ConnectionState, DriverCapabilities, DriverError, ScaleDateTime, ScaleDriver, ScaleEvent,
    ScaleInfo,
};
use openscale_storage::{DEFAULT_LOCAL_STORE_ID, Database};

#[derive(Default)]
struct InMemoryDriverState {
    plus: BTreeMap<PluKey, Plu>,
    departments: Vec<Department>,
    tares: Vec<Tare>,
    tax_rates: Vec<TaxRate>,
    stored_texts: Vec<StoredText>,
    barcode_formats: Vec<BarcodeFormat>,
    label_formats: Vec<LabelFormat>,
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

    async fn upsert_department(&mut self, _dept: &Department) -> Result<(), DriverError> {
        Ok(())
    }

    async fn upsert_tare(&mut self, _tare: &Tare) -> Result<(), DriverError> {
        Ok(())
    }

    async fn upsert_tax_rate(&mut self, _tax: &TaxRate) -> Result<(), DriverError> {
        Ok(())
    }

    async fn upsert_stored_text(
        &mut self,
        _text: &StoredText,
        _table: u8,
    ) -> Result<(), DriverError> {
        Ok(())
    }

    async fn upsert_barcode_format(&mut self, _fmt: &BarcodeFormat) -> Result<(), DriverError> {
        Ok(())
    }

    async fn upsert_label_format(&mut self, fmt: &LabelFormat) -> Result<(), DriverError> {
        let mut st = self.state.lock().unwrap();
        if let Some(existing) = st.label_formats.iter_mut().find(|f| f.pfid == fmt.pfid) {
            *existing = fmt.clone();
        } else {
            st.label_formats.push(fmt.clone());
        }
        Ok(())
    }

    async fn list_departments(&mut self) -> Result<Vec<Department>, DriverError> {
        Ok(self.state.lock().unwrap().departments.clone())
    }

    async fn list_tares(&mut self) -> Result<Vec<Tare>, DriverError> {
        Ok(self.state.lock().unwrap().tares.clone())
    }

    async fn list_tax_rates(&mut self) -> Result<Vec<TaxRate>, DriverError> {
        Ok(self.state.lock().unwrap().tax_rates.clone())
    }

    async fn list_stored_texts(&mut self) -> Result<Vec<StoredText>, DriverError> {
        Ok(self.state.lock().unwrap().stored_texts.clone())
    }

    async fn list_barcode_formats(&mut self) -> Result<Vec<BarcodeFormat>, DriverError> {
        Ok(self.state.lock().unwrap().barcode_formats.clone())
    }

    async fn list_label_formats(&mut self) -> Result<Vec<LabelFormat>, DriverError> {
        Ok(self.state.lock().unwrap().label_formats.clone())
    }

    async fn read_grand_totals(&mut self, period: u8) -> Result<Option<GrandTotals>, DriverError> {
        Ok(Some(GrandTotals {
            period,
            counter_sales: MoneyCents(50_000),
            prepack_sales: MoneyCents(10_000),
            transactions: 12,
            gross_total: MoneyCents(60_000),
            last_cleared_at: None,
        }))
    }

    async fn read_mach_totals(&mut self, period: u8) -> Result<Option<MachTotals>, DriverError> {
        Ok(Some(MachTotals {
            period,
            counter_sales: MoneyCents(50_000),
            prepack_sales: MoneyCents(10_000),
            transactions: 12,
            gross_total: MoneyCents(60_000),
        }))
    }

    async fn list_dept_totals(&mut self, period: u8) -> Result<Vec<DeptTotals>, DriverError> {
        Ok(vec![DeptTotals {
            period,
            did: 1,
            counter_sales: MoneyCents(50_000),
            prepack_sales: MoneyCents(0),
            transactions: 12,
            last_cleared_at: None,
        }])
    }

    async fn list_plu_totals(&mut self, period: u8) -> Result<Vec<PluTotals>, DriverError> {
        Ok(vec![PluTotals {
            period,
            key: PluKey::new(1, 1).unwrap(),
            transactions: 12,
            counter_sales: MoneyCents(50_000),
            counter_quantity: 5_000,
            counter_transactions: 12,
            prepack_sales: MoneyCents(0),
            prepack_quantity: 0,
            last_cleared_at: None,
        }])
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
        branch_id: None,
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

#[tokio::test]
async fn master_tables_sync_and_sales_snapshots_e2e() {
    let db = Database::connect_in_memory().await.unwrap();
    let app = OpenScaleApp::new(db);

    let scale_id = new_id();
    let scale = Scale {
        id: scale_id,
        branch_id: None,
        group_id: None,
        name: "Báscula Ventas".to_string(),
        family: ScaleFamily::AveryBerkelXs,
        ip_address: "192.168.9.114".to_string(),
        data_port: 3001,
        service_port: None,
        serial_number: None,
        model: Some("XS200".to_string()),
        firmware_version: None,
        mac_address: None,
        status: ScaleStatus::Online,
        last_seen_at: None,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };
    app.upsert_scale(scale).await.unwrap();
    assert_eq!(app.list_scales().await.unwrap().len(), 1);

    let state = Arc::new(Mutex::new(InMemoryDriverState::default()));
    app.scales
        .register_driver(
            scale_id,
            Box::new(MockScaleDriver {
                state: Arc::clone(&state),
            }),
        )
        .await;

    // Master tables CRUD
    app.upsert_department(Department {
        id: new_id(),
        did: 1,
        name: "CARNICERIA".to_string(),
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    })
    .await
    .unwrap();
    assert_eq!(app.list_departments().await.unwrap().len(), 1);

    app.upsert_tare(Tare {
        id: new_id(),
        cid: 1,
        name: "Charola".to_string(),
        weight_grams: 20,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    })
    .await
    .unwrap();
    assert_eq!(app.list_tares().await.unwrap().len(), 1);

    app.upsert_tax_rate(TaxRate {
        id: new_id(),
        tax_id: 1,
        name: "IVA 16%".to_string(),
        rate_basis_points: 1600,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    })
    .await
    .unwrap();
    assert_eq!(app.list_tax_rates().await.unwrap().len(), 1);

    app.upsert_stored_text(StoredText {
        id: new_id(),
        text_id: 1,
        title: "Aviso".to_string(),
        content: "MANTENGA REFRIGERADO".to_string(),
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    })
    .await
    .unwrap();
    assert_eq!(app.list_stored_texts().await.unwrap().len(), 1);

    app.upsert_barcode_format(BarcodeFormat {
        id: new_id(),
        bid: 1,
        name: "EAN-13".to_string(),
        definition_json: r#"{"bmc":0,"bt":3,"bpd":1,"subtype":3,"template":"2NNAAAAWWWWWC"}"#
            .to_string(),
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    })
    .await
    .unwrap();
    assert_eq!(app.list_barcode_formats().await.unwrap().len(), 1);

    // Sync master tables exercises ScaleHandle + actor commands
    let sent = app.sync_master_tables(scale_id).await.unwrap();
    assert_eq!(sent, 5);

    // Capture sales snapshot & list snapshots (filtered + unfiltered)
    let report = app.capture_sales_snapshot(scale_id, 0).await.unwrap();
    assert_eq!(report.scale_id, scale_id);
    assert_eq!(report.period, 0);

    let by_scale = app.list_sales_snapshots(Some(scale_id), 10).await.unwrap();
    assert_eq!(by_scale.len(), 1);
    let all = app.list_sales_snapshots(None, 10).await.unwrap();
    assert_eq!(all.len(), 1);

    app.scales.unregister(scale_id).await;
}

#[tokio::test]
async fn selective_import_with_conflict_resolution_e2e() {
    let db = Database::connect_in_memory().await.unwrap();
    let app = OpenScaleApp::new(db);

    let scale_id = new_id();
    let scale = Scale {
        id: scale_id,
        branch_id: None,
        group_id: None,
        name: "XS Import Test".to_string(),
        family: ScaleFamily::AveryBerkelXs,
        ip_address: "192.168.9.200".to_string(),
        data_port: 3001,
        service_port: Some(28822),
        serial_number: None,
        model: Some("XS200".to_string()),
        firmware_version: Some("4.1.7.0".to_string()),
        mac_address: None,
        status: ScaleStatus::Online,
        last_seen_at: None,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };
    app.scale_repo().upsert(&scale).await.unwrap();

    // Seed local DB with 1 matching PLU, 1 conflicting PLU, 1 conflicting Department, and 1 conflicting Tare
    app.upsert_plu(sample_plu(1, "POLLO ENTERO", 6500))
        .await
        .unwrap();
    app.upsert_plu(sample_plu(2, "PECHUGA LOCAL", 11000))
        .await
        .unwrap();
    app.upsert_department(Department {
        id: new_id(),
        did: 1,
        name: "CARNICERIA LOCAL".to_string(),
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    })
    .await
    .unwrap();
    app.upsert_tare(Tare {
        id: new_id(),
        cid: 5,
        name: "Charola Chica".to_string(),
        weight_grams: 15,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    })
    .await
    .unwrap();

    // Seed remote scale with:
    // - PLU 1:1 (identical)
    // - PLU 1:2 (differs: price & description)
    // - PLU 1:3 (brand new)
    // - Dept 1 (differs) + Dept 2 (new)
    // - Tare 5 (differs: 25g vs 15g) + Tare 6 (new)
    // - Tax 1 (new), StoredText 1 (new), BarcodeFormat 1 (new), LabelFormat 1 (new)
    let state = Arc::new(Mutex::new(InMemoryDriverState::default()));
    {
        let mut st = state.lock().unwrap();
        let p1 = sample_plu(1, "POLLO ENTERO", 6500);
        let p2 = sample_plu(2, "PECHUGA BASCULA", 12500);
        let p3 = sample_plu(3, "PIERNA NUEVA", 5400);
        st.plus.insert(p1.key, p1);
        st.plus.insert(p2.key, p2);
        st.plus.insert(p3.key, p3);

        st.departments.push(Department {
            id: new_id(),
            did: 1,
            name: "CARNICERIA BASCULA".to_string(),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        });
        st.departments.push(Department {
            id: new_id(),
            did: 2,
            name: "CREMERIA".to_string(),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        });

        st.tares.push(Tare {
            id: new_id(),
            cid: 5,
            name: "Container 5".to_string(),
            weight_grams: 25,
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        });
        st.tares.push(Tare {
            id: new_id(),
            cid: 6,
            name: "Container 6".to_string(),
            weight_grams: 40,
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        });

        st.tax_rates.push(TaxRate {
            id: new_id(),
            tax_id: 1,
            name: "Tax 1".to_string(),
            rate_basis_points: 1600,
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        });

        st.stored_texts.push(StoredText {
            id: new_id(),
            text_id: 1,
            title: "Message 1".to_string(),
            content: "MANTENER FRIO".to_string(),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        });

        st.barcode_formats.push(BarcodeFormat {
            id: new_id(),
            bid: 1,
            name: "Barcode 1".to_string(),
            definition_json: r#"{"bmc":0,"bt":3,"bpd":1,"subtype":3,"template":"2NNAAAAWWWWWC"}"#
                .to_string(),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        });

        st.label_formats.push(LabelFormat {
            id: new_id(),
            pfid: 1,
            name: "STD 59x40".to_string(),
            width_tenths_mm: 590,
            height_tenths_mm: 0,
            definition_json: r#"{"active":true,"fields":[]}"#.to_string(),
            raw_binary: Some(vec![0u8; 898]),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        });
    }

    app.scales
        .register_driver(
            scale_id,
            Box::new(MockScaleDriver {
                state: state.clone(),
            }),
        )
        .await;

    let preview = app
        .preview_import_from_scale(scale_id, ImportSelection::default())
        .await
        .unwrap();

    // Verify new, unchanged, and conflict buckets
    assert_eq!(preview.unchanged_counts.plus, 1);
    assert_eq!(preview.new_plus.len(), 1);
    assert_eq!(preview.conflict_plus.len(), 1);
    assert_eq!(preview.new_departments.len(), 1);
    assert_eq!(preview.conflict_departments.len(), 1);
    assert_eq!(preview.new_tares.len(), 1);
    assert_eq!(preview.conflict_tares.len(), 1);
    // Remote auto-generated tare name should preserve local custom name on conflict
    assert_eq!(preview.conflict_tares[0].name, "Charola Chica");
    assert_eq!(preview.new_tax_rates.len(), 1);
    assert_eq!(preview.new_stored_texts.len(), 1);
    assert_eq!(preview.new_barcode_formats.len(), 1);
    assert_eq!(preview.new_label_formats.len(), 1);

    assert_eq!(preview.conflicts.len(), 3);
    assert!(
        preview
            .conflicts
            .iter()
            .any(|c| c.category == ImportCategory::Plu && c.key == "1:2" && c.diffs.len() == 2)
    );
    assert!(
        preview
            .conflicts
            .iter()
            .any(|c| c.category == ImportCategory::Department && c.key == "1")
    );
    assert!(
        preview
            .conflicts
            .iter()
            .any(|c| c.category == ImportCategory::Tare && c.key == "5")
    );

    // Resolve conflicts:
    // - PLU 1:2 -> KEEP LOCAL
    // - Dept 1 -> USE SCALE
    // - Tare 5 -> USE SCALE
    let req = ScaleImportApplyRequest {
        scale_id,
        plus: preview.new_plus.clone(), // omit conflict_plus[0] => keep local
        departments: [
            preview.new_departments.clone(),
            preview.conflict_departments.clone(),
        ]
        .concat(),
        tares: [preview.new_tares.clone(), preview.conflict_tares.clone()].concat(),
        tax_rates: preview.new_tax_rates.clone(),
        stored_texts: preview.new_stored_texts.clone(),
        barcode_formats: preview.new_barcode_formats.clone(),
        label_formats: preview.new_label_formats.clone(),
        kept_local_conflicts: 1,
        skipped_unchanged: preview.unchanged_counts.total(),
    };

    let result = app.apply_scale_import(req).await.unwrap();
    assert_eq!(result.imported_plus, 1);
    assert_eq!(result.imported_departments, 2);
    assert_eq!(result.imported_tares, 2);
    assert_eq!(result.imported_tax_rates, 1);
    assert_eq!(result.imported_stored_texts, 1);
    assert_eq!(result.imported_barcode_formats, 1);
    assert_eq!(result.imported_label_formats, 1);
    assert_eq!(result.kept_local_conflicts, 1);
    assert_eq!(result.skipped_unchanged, 1);

    // Verify local DB reflects exact user choices
    let plu2 = app
        .get_plu(PluKey::new(1, 2).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(plu2.description, "PECHUGA LOCAL");
    assert_eq!(plu2.price_1, MoneyCents(11000));

    let plu3 = app
        .get_plu(PluKey::new(1, 3).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(plu3.description, "PIERNA NUEVA");

    let depts = app.list_departments().await.unwrap();
    assert_eq!(depts.len(), 2);
    assert_eq!(
        depts.iter().find(|d| d.did == 1).unwrap().name,
        "CARNICERIA BASCULA"
    );

    let tares = app.list_tares().await.unwrap();
    assert_eq!(tares.len(), 2);
    let tare5 = tares.iter().find(|t| t.cid == 5).unwrap();
    assert_eq!(tare5.weight_grams, 25);
    assert_eq!(tare5.name, "Charola Chica");

    assert_eq!(app.list_tax_rates().await.unwrap().len(), 1);
    assert_eq!(app.list_stored_texts().await.unwrap().len(), 1);
    assert_eq!(app.list_barcode_formats().await.unwrap().len(), 1);
    assert_eq!(app.list_label_formats().await.unwrap().len(), 1);

    app.scales.unregister(scale_id).await;
}

#[tokio::test]
async fn delete_scale_soft_deletes_and_unregisters_actor() {
    let db = Database::connect_in_memory().await.unwrap();
    let app = OpenScaleApp::new(db);

    let scale_id = new_id();
    let scale = Scale {
        id: scale_id,
        branch_id: None,
        group_id: None,
        name: "Carnicería 02".to_string(),
        family: ScaleFamily::AveryBerkelXs,
        ip_address: "192.168.9.115".to_string(),
        data_port: 3001,
        service_port: Some(28822),
        serial_number: Some("22320311".to_string()),
        model: Some("XS200".to_string()),
        firmware_version: Some("4.1.7.0".to_string()),
        mac_address: None,
        status: ScaleStatus::Online,
        last_seen_at: None,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };

    app.upsert_scale(scale).await.unwrap();
    assert_eq!(app.list_scales().await.unwrap().len(), 1);
    assert!(app.scales.get(scale_id).await.is_ok());

    assert!(app.delete_scale(scale_id).await.unwrap());
    assert!(!app.delete_scale(scale_id).await.unwrap());
    assert!(app.list_scales().await.unwrap().is_empty());
    assert!(app.scales.get(scale_id).await.is_err());
}

struct FlakyScaleDriver {
    reachable: Arc<AtomicBool>,
    connected: bool,
}

#[async_trait]
impl ScaleDriver for FlakyScaleDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities::xs_default()
    }

    async fn connect(&mut self) -> Result<ScaleInfo, DriverError> {
        if self.reachable.load(Ordering::SeqCst) {
            self.connected = true;
            Ok(ScaleInfo {
                family: ScaleFamily::AveryBerkelXs,
                model: "XS200".to_string(),
                serial_number: None,
                firmware_version: "4.1.7.0".to_string(),
                protocol_version: None,
                mac_address: None,
                requires_auth: false,
            })
        } else {
            self.connected = false;
            Err(DriverError::ConnectionFailed(
                "192.168.9.150:3001: host unreachable".to_string(),
            ))
        }
    }

    async fn disconnect(&mut self) -> Result<(), DriverError> {
        self.connected = false;
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DriverError> {
        if !self.connected {
            return Err(DriverError::NotConnected);
        }
        if self.reachable.load(Ordering::SeqCst) {
            Ok(())
        } else {
            self.connected = false;
            Err(DriverError::Timeout)
        }
    }

    async fn get_info(&mut self) -> Result<ScaleInfo, DriverError> {
        self.connect().await
    }

    async fn get_datetime(&mut self) -> Result<ScaleDateTime, DriverError> {
        Err(DriverError::Unsupported("get_datetime"))
    }

    async fn set_datetime(&mut self, _dt: ScaleDateTime) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("set_datetime"))
    }

    async fn read_plu(&mut self, _key: PluKey) -> Result<Option<Plu>, DriverError> {
        Ok(None)
    }

    async fn list_plus(
        &mut self,
        _from: Option<PluKey>,
        _to: Option<PluKey>,
    ) -> Result<Vec<Plu>, DriverError> {
        Ok(vec![])
    }

    async fn count_plus(&mut self) -> Result<u64, DriverError> {
        Ok(0)
    }

    async fn upsert_plu(&mut self, _plu: &Plu) -> Result<(), DriverError> {
        Ok(())
    }

    async fn change_price(
        &mut self,
        _key: PluKey,
        _price_1: MoneyCents,
        _price_2: Option<MoneyCents>,
        _price_3: Option<MoneyCents>,
    ) -> Result<(), DriverError> {
        Ok(())
    }

    async fn delete_plu(&mut self, _key: PluKey) -> Result<(), DriverError> {
        Ok(())
    }
}

#[tokio::test]
async fn health_monitor_detects_disconnect_and_recovery_transitions() {
    let db = Database::connect_in_memory().await.unwrap();
    let app = OpenScaleApp::new(db);

    let scale_id = new_id();
    let scale = Scale {
        id: scale_id,
        branch_id: None,
        group_id: None,
        name: "Báscula Monitor".to_string(),
        family: ScaleFamily::AveryBerkelXs,
        ip_address: "192.168.9.150".to_string(),
        data_port: 3001,
        service_port: Some(28822),
        serial_number: None,
        model: Some("XS200".to_string()),
        firmware_version: None,
        mac_address: None,
        status: ScaleStatus::Unknown,
        last_seen_at: None,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };
    app.scale_repo().upsert(&scale).await.unwrap();

    let reachable = Arc::new(AtomicBool::new(true));
    app.scales
        .register_driver(
            scale_id,
            Box::new(FlakyScaleDriver {
                reachable: Arc::clone(&reachable),
                connected: false,
            }),
        )
        .await;

    let mut events = app.scales.subscribe();

    // 1. Initial check: Unknown -> Online
    let res1 = app.check_scale_health(scale_id).await.unwrap();
    assert_eq!(res1.previous_status, ScaleStatus::Unknown);
    assert_eq!(res1.status, ScaleStatus::Online);
    assert!(res1.last_seen_at.is_some());
    assert!(res1.error.is_none());
    assert!(matches!(
        events.try_recv().unwrap(),
        ScaleEvent::ConnectionChanged {
            scale_id: id,
            state: ConnectionState::Connected,
            message: None,
        } if id == scale_id
    ));

    // 2. Repeated healthy check: stays Online, no duplicate event emitted
    let res2 = app.check_scale_health(scale_id).await.unwrap();
    assert_eq!(res2.previous_status, ScaleStatus::Online);
    assert_eq!(res2.status, ScaleStatus::Online);
    assert!(events.try_recv().is_err());

    let seen_before_drop = res2.last_seen_at;

    // 3. Scale loses network connection: Online -> Offline
    reachable.store(false, Ordering::SeqCst);
    let res3 = app.check_scale_health(scale_id).await.unwrap();
    assert_eq!(res3.previous_status, ScaleStatus::Online);
    assert_eq!(res3.status, ScaleStatus::Offline);
    assert_eq!(res3.last_seen_at, seen_before_drop);
    assert!(res3.error.is_some());
    assert!(matches!(
        events.try_recv().unwrap(),
        ScaleEvent::ConnectionChanged {
            scale_id: id,
            state: ConnectionState::Disconnected,
            message: Some(_),
        } if id == scale_id
    ));

    // Persisted in SQLite as Offline while preserving last_seen_at
    let persisted_offline = app
        .scale_repo()
        .get_by_id(DEFAULT_LOCAL_STORE_ID, scale_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted_offline.status, ScaleStatus::Offline);
    assert_eq!(persisted_offline.last_seen_at, seen_before_drop);

    // 4. Repeated offline check: stays Offline, no duplicate event emitted
    let res4 = app.check_scale_health(scale_id).await.unwrap();
    assert_eq!(res4.status, ScaleStatus::Offline);
    assert!(events.try_recv().is_err());

    // 5. Scale recovers network connection via check_all_scales_health: Offline -> Online
    reachable.store(true, Ordering::SeqCst);
    let all_results = app.check_all_scales_health().await.unwrap();
    assert_eq!(all_results.len(), 1);
    assert_eq!(all_results[0].previous_status, ScaleStatus::Offline);
    assert_eq!(all_results[0].status, ScaleStatus::Online);
    assert!(matches!(
        events.try_recv().unwrap(),
        ScaleEvent::ConnectionChanged {
            scale_id: id,
            state: ConnectionState::Connected,
            message: None,
        } if id == scale_id
    ));

    // 6. Background monitor detects a drop automatically on its periodic tick
    reachable.store(false, Ordering::SeqCst);
    let monitor = app.start_health_monitor(Duration::from_millis(10));
    let drop_event = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("monitor should emit disconnect event")
        .expect("broadcast channel should remain open");
    monitor.abort();
    assert!(matches!(
        drop_event,
        ScaleEvent::ConnectionChanged {
            scale_id: id,
            state: ConnectionState::Disconnected,
            message: Some(_),
        } if id == scale_id
    ));

    app.scales.unregister(scale_id).await;
}

#[tokio::test]
async fn list_and_write_scale_label_formats_e2e() {
    let db = Database::connect_in_memory().await.unwrap();
    let app = OpenScaleApp::new(db);

    let scale_id = new_id();
    let fmt1 = LabelFormat {
        id: new_id(),
        pfid: 1,
        name: "ETIQUETA 1".to_string(),
        width_tenths_mm: 590,
        height_tenths_mm: 400,
        definition_json:
            r#"{"orientation":0,"active":true,"width_hundredths_mm":5900,"fields":[]}"#.to_string(),
        raw_binary: Some(vec![0u8; 898]),
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };
    let fmt2 = LabelFormat {
        id: new_id(),
        pfid: 2,
        name: "ETIQUETA 2".to_string(),
        width_tenths_mm: 590,
        height_tenths_mm: 400,
        definition_json:
            r#"{"orientation":0,"active":true,"width_hundredths_mm":5900,"fields":[]}"#.to_string(),
        raw_binary: Some(vec![0u8; 898]),
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };

    let state = Arc::new(Mutex::new(InMemoryDriverState {
        label_formats: vec![fmt1.clone(), fmt2],
        ..Default::default()
    }));

    app.scales
        .register_driver(
            scale_id,
            Box::new(MockScaleDriver {
                state: Arc::clone(&state),
            }),
        )
        .await;

    // 1. Read existing formats directly from the scale actor
    let remote_formats = app.list_scale_label_formats(scale_id).await.unwrap();
    assert_eq!(remote_formats.len(), 2);
    assert_eq!(remote_formats[0].pfid, 1);
    assert_eq!(remote_formats[0].name, "ETIQUETA 1");
    assert_eq!(remote_formats[1].pfid, 2);

    // 2. Edit PFID 1 and write back to scale + local SQLite
    let mut edited = fmt1;
    edited.name = "ETIQUETA 1 EDITADA".to_string();
    let saved = app
        .write_scale_label_format(scale_id, edited.clone())
        .await
        .unwrap();
    assert_eq!(saved.pfid, 1);
    assert_eq!(saved.name, "ETIQUETA 1 EDITADA");

    // Verify scale received the edit
    let after_write = app.list_scale_label_formats(scale_id).await.unwrap();
    assert_eq!(after_write.len(), 2);
    assert_eq!(after_write[0].name, "ETIQUETA 1 EDITADA");

    // Verify local SQLite persisted the edit
    let local_formats = app.list_label_formats().await.unwrap();
    assert_eq!(local_formats.len(), 1);
    assert_eq!(local_formats[0].pfid, 1);
    assert_eq!(local_formats[0].name, "ETIQUETA 1 EDITADA");

    app.scales.unregister(scale_id).await;
}

#[derive(Default)]
struct FaultySyncState {
    connect_outcomes: std::collections::VecDeque<Result<(), &'static str>>,
    upsert_outcomes: std::collections::VecDeque<Result<(), DriverError>>,
    delete_outcomes: std::collections::VecDeque<Result<(), DriverError>>,
    serial_number: Option<String>,
}

struct FaultyScaleDriver {
    state: Arc<Mutex<FaultySyncState>>,
}

#[async_trait]
impl ScaleDriver for FaultyScaleDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities::xs_default()
    }

    async fn connect(&mut self) -> Result<ScaleInfo, DriverError> {
        let mut st = self.state.lock().unwrap();
        if let Some(Err(msg)) = st.connect_outcomes.pop_front() {
            return Err(DriverError::ConnectionFailed(msg.to_string()));
        }
        Ok(ScaleInfo {
            family: ScaleFamily::AveryBerkelXs,
            model: "XS200".to_string(),
            serial_number: st.serial_number.clone(),
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
        Err(DriverError::Unsupported("get_datetime"))
    }

    async fn set_datetime(&mut self, _dt: ScaleDateTime) -> Result<(), DriverError> {
        Ok(())
    }

    async fn read_plu(&mut self, _key: PluKey) -> Result<Option<Plu>, DriverError> {
        Ok(None)
    }

    async fn list_plus(
        &mut self,
        _from: Option<PluKey>,
        _to: Option<PluKey>,
    ) -> Result<Vec<Plu>, DriverError> {
        Ok(vec![])
    }

    async fn count_plus(&mut self) -> Result<u64, DriverError> {
        Ok(0)
    }

    async fn upsert_plu(&mut self, _plu: &Plu) -> Result<(), DriverError> {
        self.state
            .lock()
            .unwrap()
            .upsert_outcomes
            .pop_front()
            .unwrap_or(Ok(()))
    }

    async fn change_price(
        &mut self,
        _key: PluKey,
        _p1: MoneyCents,
        _p2: Option<MoneyCents>,
        _p3: Option<MoneyCents>,
    ) -> Result<(), DriverError> {
        Ok(())
    }

    async fn delete_plu(&mut self, _key: PluKey) -> Result<(), DriverError> {
        self.state
            .lock()
            .unwrap()
            .delete_outcomes
            .pop_front()
            .unwrap_or(Ok(()))
    }
}

#[tokio::test]
async fn sync_handles_transient_retries_and_failures_for_upserts_and_deletes() {
    let db = Database::connect_in_memory().await.unwrap();
    let app = OpenScaleApp::new(db);
    let scale_id = new_id();
    let scale = Scale {
        id: scale_id,
        name: "Báscula Faulty".to_string(),
        family: ScaleFamily::AveryBerkelXs,
        ip_address: "192.168.1.98".to_string(),
        data_port: 3001,
        service_port: Some(28822),
        branch_id: None,
        group_id: None,
        model: None,
        serial_number: Some("SN-OK".to_string()),
        mac_address: None,
        firmware_version: None,
        status: ScaleStatus::Online,
        last_seen_at: None,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };
    app.scale_repo().upsert(&scale).await.unwrap();

    let p1 = sample_plu(1, "PLU RETRY OK", 1000);
    let p2 = sample_plu(2, "PLU RECONNECT FAIL", 2000);
    let p3 = sample_plu(3, "PLU RETRY FAIL", 3000);
    let p4 = sample_plu(4, "PLU PROTOCOL FAIL", 4000);
    for p in [&p1, &p2, &p3, &p4] {
        app.upsert_plu(p.clone()).await.unwrap();
    }

    let state = Arc::new(Mutex::new(FaultySyncState {
        connect_outcomes: std::collections::VecDeque::from([
            Ok(()),                    // p1 reconnect succeeds
            Err("connection refused"), // p2 reconnect fails
            Ok(()),                    // p3 reconnect succeeds
        ]),
        upsert_outcomes: std::collections::VecDeque::from([
            Err(DriverError::Timeout),      // p1 first attempt transient
            Ok(()),                         // p1 retry succeeds
            Err(DriverError::NotConnected), // p2 transient, reconnect fails
            Err(DriverError::Timeout),      // p3 first attempt transient
            Err(DriverError::Protocol {
                code: Some(99),
                message: "retry rejected".to_string(),
            }), // p3 retry fails
            Err(DriverError::Protocol {
                code: Some(42),
                message: "invalid plu".to_string(),
            }), // p4 non-transient failure
        ]),
        delete_outcomes: std::collections::VecDeque::new(),
        serial_number: Some("SN-OK".to_string()),
    }));

    app.scales
        .register_driver(
            scale_id,
            Box::new(FaultyScaleDriver {
                state: Arc::clone(&state),
            }),
        )
        .await;

    let report = app
        .sync_scale_with_options(scale_id, SyncMode::Delta, false)
        .await
        .unwrap();
    assert_eq!(report.total_catalog_plus, 4);
    assert_eq!(report.sent_plus, 1);
    assert_eq!(report.failed_plus, 3);
    assert_eq!(report.deleted_plus, 0);

    // Sync p2 and p3 cleanly so we have 3 synced PLUs (p1, p2, p3) to test delete error paths
    let ok_report = app
        .sync_scale_with_options(scale_id, SyncMode::Delta, false)
        .await
        .unwrap();
    assert_eq!(ok_report.sent_plus, 3);
    assert_eq!(ok_report.failed_plus, 0);

    // Remove p1, p2, p3, p4 locally and exercise delete retry success, reconnect fail, and protocol fail
    for p in [&p1, &p2, &p3, &p4] {
        app.delete_plu(p.key).await.unwrap();
    }
    {
        let mut st = state.lock().unwrap();
        st.connect_outcomes = std::collections::VecDeque::from([
            Ok(()),           // delete 1 reconnect succeeds
            Err("host down"), // delete 2 reconnect fails
        ]);
        st.delete_outcomes = std::collections::VecDeque::from([
            Err(DriverError::Timeout),      // delete 1 transient
            Ok(()),                         // delete 1 retry succeeds
            Err(DriverError::NotConnected), // delete 2 transient, reconnect fails
            Err(DriverError::Protocol {
                code: Some(50),
                message: "delete rejected".to_string(),
            }), // delete 3 non-transient failure
            Ok(()),                         // delete 4 succeeds directly
        ]);
    }

    let del_report = app
        .sync_scale_with_options(scale_id, SyncMode::Delta, true)
        .await
        .unwrap();
    assert_eq!(del_report.deleted_plus, 2);
    assert_eq!(del_report.failed_plus, 2);

    app.scales.unregister(scale_id).await;
}

#[tokio::test]
async fn health_monitor_detects_identity_conflict() {
    let db = Database::connect_in_memory().await.unwrap();
    let app = OpenScaleApp::new(db);
    let scale_id = new_id();

    let scale = Scale {
        id: scale_id,
        name: "Báscula Carnicería".to_string(),
        family: ScaleFamily::AveryBerkelXs,
        ip_address: "192.168.1.99".to_string(),
        data_port: 3001,
        service_port: Some(28822),
        branch_id: None,
        group_id: None,
        model: None,
        serial_number: Some("SN-EXPECTED".to_string()),
        mac_address: None,
        firmware_version: None,
        status: ScaleStatus::Offline,
        last_seen_at: None,
        meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
    };
    app.scale_repo().upsert(&scale).await.unwrap();

    let state = Arc::new(Mutex::new(FaultySyncState {
        serial_number: Some("SN-OTHER".to_string()),
        ..Default::default()
    }));
    app.scales
        .register_driver(scale_id, Box::new(FaultyScaleDriver { state }))
        .await;

    let mut events = app.scales.subscribe();
    let res = app.check_scale_health(scale_id).await.unwrap();
    assert_eq!(res.status, ScaleStatus::Error);
    assert_eq!(res.detected_serial.as_deref(), Some("SN-OTHER"));
    assert!(res.identity_conflict.is_some());

    assert!(matches!(
        events.try_recv().unwrap(),
        ScaleEvent::IdentityConflict {
            scale_id: id,
            expected_serial,
            actual_serial,
            ..
        } if id == scale_id && expected_serial == "SN-EXPECTED" && actual_serial == "SN-OTHER"
    ));

    app.scales.unregister(scale_id).await;
}
