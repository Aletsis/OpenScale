use chrono::Utc;
use openscale_domain::types::new_id;
use openscale_domain::{
    AuditLog, MoneyCents, Plu, PluKey, ProductType, Scale, ScaleFamily, ScaleStatus, SyncMetadata,
};
use openscale_storage::repos::{AuditRepository, PluRepository, ScaleRepository};
use openscale_storage::{DEFAULT_LOCAL_STORE_ID, Database, StorageConfig};

#[tokio::test]
async fn sqlite_wal_persistence_lifecycle_e2e() {
    let db_path = std::env::temp_dir().join(format!("openscale-e2e-{}.db", new_id()));
    let config = StorageConfig::new(&db_path);

    let key = PluKey::new(1, 200).unwrap();
    let scale_id = new_id();

    {
        let db = Database::connect(&config).await.unwrap();

        let journal_mode: (String,) = sqlx::query_as("PRAGMA journal_mode;")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(journal_mode.0.to_lowercase(), "wal");

        let scale_repo = ScaleRepository::new(db.pool().clone());
        let plu_repo = PluRepository::new(db.pool().clone());
        let audit_repo = AuditRepository::new(db.pool().clone());

        let scale = Scale {
            id: scale_id,
            group_id: None,
            name: "Salchichonería 01".to_string(),
            family: ScaleFamily::AveryBerkelXti,
            ip_address: "192.168.9.120".to_string(),
            data_port: 3002,
            service_port: None,
            serial_number: Some("99887766".to_string()),
            model: Some("XTi400".to_string()),
            firmware_version: Some("5.3.0.0".to_string()),
            mac_address: Some("00:11:22:33:44:55".to_string()),
            status: ScaleStatus::Online,
            last_seen_at: Some(Utc::now()),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        };
        scale_repo.upsert(&scale).await.unwrap();

        let mut plu = Plu {
            id: new_id(),
            key,
            description: "JAMON DE PAVO".to_string(),
            button_text: Some("Jamón".to_string()),
            price_1: MoneyCents(12500),
            price_2: MoneyCents(11500),
            price_3: MoneyCents(0),
            product_type: ProductType::ByWeight,
            ean: Some("2000200000000".to_string()),
            sku: Some("SKU-200".to_string()),
            group_id: 2,
            barcode_format_id: 1,
            label_format_id: 1,
            tare_id: None,
            tare_grams: Some(10),
            date_offset_1: 10,
            date_print_control_1: 1,
            date_offset_2: 0,
            date_print_control_2: 0,
            tax_rate_id_1: 0,
            tax_rate_id_2: 0,
            text_areas: vec!["Conservar en frío".to_string()],
            delta_hash: String::new(),
            meta: SyncMetadata::new(DEFAULT_LOCAL_STORE_ID),
        };
        plu_repo.upsert(&plu).await.unwrap();

        // Update price and verify sync_version increments
        plu.price_1 = MoneyCents(13000);
        plu.delta_hash.clear();
        plu_repo.upsert(&plu).await.unwrap();

        audit_repo
            .record(&AuditLog {
                id: new_id(),
                store_id: DEFAULT_LOCAL_STORE_ID,
                actor_id: None,
                actor_name: "local-operator".to_string(),
                action: "plu.price_updated".to_string(),
                target_type: "plu".to_string(),
                target_id: Some("1:200".to_string()),
                details_json: Some(r#"{"price_1":13000}"#.to_string()),
                created_at: Utc::now(),
            })
            .await
            .unwrap();
    }

    // Reopen database from disk and verify full durability
    {
        let db = Database::connect(&config).await.unwrap();
        let scale_repo = ScaleRepository::new(db.pool().clone());
        let plu_repo = PluRepository::new(db.pool().clone());

        let scales = scale_repo
            .list_active(DEFAULT_LOCAL_STORE_ID)
            .await
            .unwrap();
        assert_eq!(scales.len(), 1);
        assert_eq!(scales[0].id, scale_id);

        let plus = plu_repo.list_active(DEFAULT_LOCAL_STORE_ID).await.unwrap();
        assert_eq!(plus.len(), 1);
        assert_eq!(plus[0].price_1, MoneyCents(13000));
        assert_eq!(plus[0].meta.sync_version, 2);

        assert!(
            scale_repo
                .update_status(DEFAULT_LOCAL_STORE_ID, scale_id, ScaleStatus::Offline, None)
                .await
                .unwrap()
        );
        assert!(
            !scale_repo
                .update_status(DEFAULT_LOCAL_STORE_ID, new_id(), ScaleStatus::Offline, None)
                .await
                .unwrap()
        );

        assert!(
            plu_repo
                .soft_delete(DEFAULT_LOCAL_STORE_ID, key)
                .await
                .unwrap()
        );
        assert!(
            !plu_repo
                .soft_delete(DEFAULT_LOCAL_STORE_ID, key)
                .await
                .unwrap()
        );
        assert!(
            plu_repo
                .list_active(DEFAULT_LOCAL_STORE_ID)
                .await
                .unwrap()
                .is_empty()
        );
    }

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}
