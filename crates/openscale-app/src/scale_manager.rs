use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use openscale_domain::{
    BarcodeFormat, Department, LabelFormat, MoneyCents, Plu, PluKey, Scale, ScaleFamily,
    ScaleSalesReport, StoredText, Tare, TaxRate,
};
use openscale_driver::{
    ConnectionState, DriverCapabilities, DriverError, ScaleDateTime, ScaleDriver, ScaleEvent,
    ScaleInfo,
};
use openscale_driver_xs::{XsDriver, XsDriverConfig};
use openscale_driver_xti::{XtiDriver, XtiDriverConfig};
use tokio::sync::{RwLock, broadcast, mpsc, oneshot};
use uuid::Uuid;

use crate::error::AppError;

enum ScaleCommand {
    Connect(oneshot::Sender<Result<ScaleInfo, DriverError>>),
    Disconnect(oneshot::Sender<Result<(), DriverError>>),
    Ping(oneshot::Sender<Result<(), DriverError>>),
    GetInfo(oneshot::Sender<Result<ScaleInfo, DriverError>>),
    HealthCheck(oneshot::Sender<Result<(), DriverError>>),
    GetDateTime(oneshot::Sender<Result<ScaleDateTime, DriverError>>),
    SetDateTime(ScaleDateTime, oneshot::Sender<Result<(), DriverError>>),
    ReadPlu(PluKey, oneshot::Sender<Result<Option<Plu>, DriverError>>),
    ListPlus(
        Option<PluKey>,
        Option<PluKey>,
        oneshot::Sender<Result<Vec<Plu>, DriverError>>,
    ),
    CountPlus(oneshot::Sender<Result<u64, DriverError>>),
    UpsertPlu(Box<Plu>, oneshot::Sender<Result<(), DriverError>>),
    ChangePrice {
        key: PluKey,
        p1: MoneyCents,
        p2: Option<MoneyCents>,
        p3: Option<MoneyCents>,
        reply: oneshot::Sender<Result<(), DriverError>>,
    },
    DeletePlu(PluKey, oneshot::Sender<Result<(), DriverError>>),
    UpsertDepartment(Department, oneshot::Sender<Result<(), DriverError>>),
    UpsertTare(Tare, oneshot::Sender<Result<(), DriverError>>),
    UpsertTaxRate(TaxRate, oneshot::Sender<Result<(), DriverError>>),
    UpsertStoredText(StoredText, u8, oneshot::Sender<Result<(), DriverError>>),
    UpsertBarcodeFormat(BarcodeFormat, oneshot::Sender<Result<(), DriverError>>),
    UpsertLabelFormat(LabelFormat, oneshot::Sender<Result<(), DriverError>>),
    ListDepartments(oneshot::Sender<Result<Vec<Department>, DriverError>>),
    ListTares(oneshot::Sender<Result<Vec<Tare>, DriverError>>),
    ListTaxRates(oneshot::Sender<Result<Vec<TaxRate>, DriverError>>),
    ListStoredTexts(oneshot::Sender<Result<Vec<StoredText>, DriverError>>),
    ListBarcodeFormats(oneshot::Sender<Result<Vec<BarcodeFormat>, DriverError>>),
    ListLabelFormats(oneshot::Sender<Result<Vec<LabelFormat>, DriverError>>),
    CaptureSalesReport(u8, oneshot::Sender<Result<ScaleSalesReport, DriverError>>),
}

/// Handle for sending serialized commands to a single scale's dedicated Tokio Actor.
#[derive(Clone)]
pub struct ScaleHandle {
    scale_id: Uuid,
    capabilities: DriverCapabilities,
    tx: mpsc::Sender<ScaleCommand>,
}

impl ScaleHandle {
    pub fn scale_id(&self) -> Uuid {
        self.scale_id
    }

    pub fn capabilities(&self) -> &DriverCapabilities {
        &self.capabilities
    }

    pub async fn connect(&self) -> Result<ScaleInfo, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::Connect(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn disconnect(&self) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::Disconnect(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn ping(&self) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::Ping(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn get_info(&self) -> Result<ScaleInfo, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::GetInfo(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    /// Verifies live network reachability by pinging an active session or reconnecting if needed.
    pub async fn health_check(&self) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::HealthCheck(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn get_datetime(&self) -> Result<ScaleDateTime, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::GetDateTime(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn set_datetime(&self, dt: ScaleDateTime) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::SetDateTime(dt, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn read_plu(&self, key: PluKey) -> Result<Option<Plu>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::ReadPlu(key, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn list_plus(
        &self,
        from: Option<PluKey>,
        to: Option<PluKey>,
    ) -> Result<Vec<Plu>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::ListPlus(from, to, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn count_plus(&self) -> Result<u64, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::CountPlus(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn upsert_plu(&self, plu: Plu) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::UpsertPlu(Box::new(plu), tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn change_price(
        &self,
        key: PluKey,
        p1: MoneyCents,
        p2: Option<MoneyCents>,
        p3: Option<MoneyCents>,
    ) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::ChangePrice {
                key,
                p1,
                p2,
                p3,
                reply: tx,
            })
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn delete_plu(&self, key: PluKey) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::DeletePlu(key, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn upsert_department(&self, dept: Department) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::UpsertDepartment(dept, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn upsert_tare(&self, tare: Tare) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::UpsertTare(tare, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn upsert_tax_rate(&self, tax: TaxRate) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::UpsertTaxRate(tax, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn upsert_stored_text(&self, text: StoredText, table: u8) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::UpsertStoredText(text, table, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn upsert_barcode_format(&self, fmt: BarcodeFormat) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::UpsertBarcodeFormat(fmt, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn upsert_label_format(&self, fmt: LabelFormat) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::UpsertLabelFormat(fmt, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn list_departments(&self) -> Result<Vec<Department>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::ListDepartments(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn list_tares(&self) -> Result<Vec<Tare>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::ListTares(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn list_tax_rates(&self) -> Result<Vec<TaxRate>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::ListTaxRates(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn list_stored_texts(&self) -> Result<Vec<StoredText>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::ListStoredTexts(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn list_barcode_formats(&self) -> Result<Vec<BarcodeFormat>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::ListBarcodeFormats(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn list_label_formats(&self) -> Result<Vec<LabelFormat>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::ListLabelFormats(tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }

    pub async fn capture_sales_report(&self, period: u8) -> Result<ScaleSalesReport, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ScaleCommand::CaptureSalesReport(period, tx))
            .await
            .map_err(|_| AppError::ActorUnavailable)?;
        Ok(rx.await.map_err(|_| AppError::ActorUnavailable)??)
    }
}

/// Manages a pool of persistent TCP Scale Actors (1 Tokio task per scale).
#[derive(Clone)]
pub struct ScaleManager {
    actors: Arc<RwLock<HashMap<Uuid, ScaleHandle>>>,
    events: broadcast::Sender<ScaleEvent>,
}

impl Default for ScaleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ScaleManager {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ScaleEvent> {
        self.events.subscribe()
    }

    pub fn emit_connection_changed(
        &self,
        scale_id: Uuid,
        state: ConnectionState,
        message: Option<String>,
    ) {
        let _ = self.events.send(ScaleEvent::ConnectionChanged {
            scale_id,
            state,
            message,
        });
    }

    pub fn emit_identity_conflict(
        &self,
        scale_id: Uuid,
        ip_address: String,
        expected_serial: String,
        actual_serial: String,
    ) {
        let _ = self.events.send(ScaleEvent::IdentityConflict {
            scale_id,
            ip_address,
            expected_serial,
            actual_serial,
        });
    }

    /// Registers a scale using its protocol family's real TCP driver (`XS` or `XTi`).
    pub async fn register_scale(&self, scale: &Scale) -> ScaleHandle {
        let driver: Box<dyn ScaleDriver> = match scale.family {
            ScaleFamily::AveryBerkelXs => {
                let mut cfg = XsDriverConfig::new(&scale.ip_address, scale.meta.store_id);
                cfg.port = scale.data_port;
                cfg.io_timeout = Duration::from_secs(5);
                Box::new(XsDriver::new(cfg))
            }
            ScaleFamily::AveryBerkelXti => {
                let mut cfg = XtiDriverConfig::new(&scale.ip_address, scale.meta.store_id);
                cfg.port = scale.data_port;
                cfg.io_timeout = Duration::from_secs(5);
                Box::new(XtiDriver::new(cfg))
            }
        };
        self.register_driver(scale.id, driver).await
    }

    /// Registers a custom `ScaleDriver` instance (also used for deterministic unit tests).
    pub async fn register_driver(
        &self,
        scale_id: Uuid,
        driver: Box<dyn ScaleDriver>,
    ) -> ScaleHandle {
        // Cleanly close any existing actor for this scale before spawning a new one,
        // freeing the single TCP slot on XS scales before the new actor connects.
        if let Some(previous) = self.actors.write().await.remove(&scale_id) {
            let _ = previous.disconnect().await;
        }

        let capabilities = driver.capabilities();
        let (tx, rx) = mpsc::channel(64);
        let events = self.events.clone();

        tokio::spawn(run_scale_actor(scale_id, driver, rx, events));

        let handle = ScaleHandle {
            scale_id,
            capabilities,
            tx,
        };
        self.actors.write().await.insert(scale_id, handle.clone());
        handle
    }

    pub async fn get(&self, scale_id: Uuid) -> Result<ScaleHandle, AppError> {
        self.actors
            .read()
            .await
            .get(&scale_id)
            .cloned()
            .ok_or(AppError::ScaleNotRegistered(scale_id))
    }

    pub async fn unregister(&self, scale_id: Uuid) {
        if let Some(handle) = self.actors.write().await.remove(&scale_id) {
            let _ = handle.disconnect().await;
        }
    }
}

async fn run_scale_actor(
    scale_id: Uuid,
    mut driver: Box<dyn ScaleDriver>,
    mut rx: mpsc::Receiver<ScaleCommand>,
    events: broadcast::Sender<ScaleEvent>,
) {
    let emit = |state: ConnectionState, message: Option<String>| {
        let _ = events.send(ScaleEvent::ConnectionChanged {
            scale_id,
            state,
            message,
        });
    };

    while let Some(cmd) = rx.recv().await {
        match cmd {
            ScaleCommand::Connect(reply) => {
                emit(ConnectionState::Connecting, None);
                let res = driver.connect().await;
                match &res {
                    Ok(_) => emit(ConnectionState::Connected, None),
                    Err(err) => emit(ConnectionState::Error, Some(err.to_string())),
                }
                let _ = reply.send(res);
            }
            ScaleCommand::Disconnect(reply) => {
                let res = driver.disconnect().await;
                emit(ConnectionState::Disconnected, None);
                let _ = reply.send(res);
            }
            ScaleCommand::Ping(reply) => {
                let _ = reply.send(driver.ping().await);
            }
            ScaleCommand::GetInfo(reply) => {
                let _ = reply.send(driver.get_info().await);
            }
            ScaleCommand::HealthCheck(reply) => {
                let res = match driver.ping().await {
                    Ok(()) => Ok(()),
                    Err(
                        DriverError::NotConnected
                        | DriverError::Io(_)
                        | DriverError::Timeout
                        | DriverError::ConnectionFailed(_),
                    ) => driver.connect().await.map(|_| ()),
                    Err(err) => Err(err),
                };
                let _ = reply.send(res);
            }
            ScaleCommand::GetDateTime(reply) => {
                let _ = reply.send(driver.get_datetime().await);
            }
            ScaleCommand::SetDateTime(dt, reply) => {
                let _ = reply.send(driver.set_datetime(dt).await);
            }
            ScaleCommand::ReadPlu(key, reply) => {
                let _ = reply.send(driver.read_plu(key).await);
            }
            ScaleCommand::ListPlus(from, to, reply) => {
                let _ = reply.send(driver.list_plus(from, to).await);
            }
            ScaleCommand::CountPlus(reply) => {
                let _ = reply.send(driver.count_plus().await);
            }
            ScaleCommand::UpsertPlu(plu, reply) => {
                let _ = reply.send(driver.upsert_plu(&plu).await);
            }
            ScaleCommand::ChangePrice {
                key,
                p1,
                p2,
                p3,
                reply,
            } => {
                let _ = reply.send(driver.change_price(key, p1, p2, p3).await);
            }
            ScaleCommand::DeletePlu(key, reply) => {
                let _ = reply.send(driver.delete_plu(key).await);
            }
            ScaleCommand::UpsertDepartment(dept, reply) => {
                let _ = reply.send(driver.upsert_department(&dept).await);
            }
            ScaleCommand::UpsertTare(tare, reply) => {
                let _ = reply.send(driver.upsert_tare(&tare).await);
            }
            ScaleCommand::UpsertTaxRate(tax, reply) => {
                let _ = reply.send(driver.upsert_tax_rate(&tax).await);
            }
            ScaleCommand::UpsertStoredText(text, table, reply) => {
                let _ = reply.send(driver.upsert_stored_text(&text, table).await);
            }
            ScaleCommand::UpsertBarcodeFormat(fmt, reply) => {
                let _ = reply.send(driver.upsert_barcode_format(&fmt).await);
            }
            ScaleCommand::UpsertLabelFormat(fmt, reply) => {
                let _ = reply.send(driver.upsert_label_format(&fmt).await);
            }
            ScaleCommand::ListDepartments(reply) => {
                let _ = reply.send(driver.list_departments().await);
            }
            ScaleCommand::ListTares(reply) => {
                let _ = reply.send(driver.list_tares().await);
            }
            ScaleCommand::ListTaxRates(reply) => {
                let _ = reply.send(driver.list_tax_rates().await);
            }
            ScaleCommand::ListStoredTexts(reply) => {
                let _ = reply.send(driver.list_stored_texts().await);
            }
            ScaleCommand::ListBarcodeFormats(reply) => {
                let _ = reply.send(driver.list_barcode_formats().await);
            }
            ScaleCommand::ListLabelFormats(reply) => {
                let _ = reply.send(driver.list_label_formats().await);
            }
            ScaleCommand::CaptureSalesReport(period, reply) => {
                let res = async {
                    let grand_totals = match driver.read_grand_totals(period).await {
                        Ok(v) => v,
                        Err(DriverError::NotConnected) => {
                            driver.connect().await?;
                            driver.read_grand_totals(period).await?
                        }
                        Err(e) => return Err(e),
                    };
                    let mach_totals = driver.read_mach_totals(period).await.ok().flatten();
                    let dept_totals = driver.list_dept_totals(period).await.unwrap_or_default();
                    let plu_totals = driver.list_plu_totals(period).await.unwrap_or_default();
                    Ok(ScaleSalesReport {
                        scale_id,
                        period,
                        captured_at: Utc::now(),
                        grand_totals,
                        mach_totals,
                        dept_totals,
                        plu_totals,
                    })
                }
                .await;
                let _ = reply.send(res);
            }
        }
    }
}
