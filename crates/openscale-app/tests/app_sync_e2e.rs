use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

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
        departments: [preview.new_departments.clone(), preview.conflict_departments.clone()]
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
    let plu2 = app.get_plu(PluKey::new(1, 2).unwrap()).await.unwrap().unwrap();
    assert_eq!(plu2.description, "PECHUGA LOCAL");
    assert_eq!(plu2.price_1, MoneyCents(11000));

    let plu3 = app.get_plu(PluKey::new(1, 3).unwrap()).await.unwrap().unwrap();
    assert_eq!(plu3.description, "PIERNA NUEVA");

    let depts = app.list_departments().await.unwrap();
    assert_eq!(depts.len(), 2);
    assert_eq!(depts.iter().find(|d| d.did == 1).unwrap().name, "CARNICERIA BASCULA");

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
