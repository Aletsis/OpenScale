use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use openscale_domain::{MoneyCents, Plu, PluKey, Scale, ScaleFamily};
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
        }
    }
}
