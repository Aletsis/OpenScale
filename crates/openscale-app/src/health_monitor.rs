use std::time::Duration;

use chrono::{DateTime, Utc};
use openscale_domain::ScaleStatus;
use openscale_driver::ConnectionState;
use serde::{Deserialize, Serialize};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::MissedTickBehavior;
use uuid::Uuid;

use crate::error::AppError;
use crate::service::OpenScaleApp;

/// Default periodic interval between background scale connectivity checks.
pub const DEFAULT_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(15);

/// Result of a single scale network connectivity health check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleHealthCheckResult {
    pub scale_id: Uuid,
    pub previous_status: ScaleStatus,
    pub status: ScaleStatus,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub checked_at: DateTime<Utc>,
    pub error: Option<String>,
}

/// Checks a single scale's live network reachability, persists its status in SQLite,
/// and emits a `ScaleEvent::ConnectionChanged` event when its status transitions.
pub async fn check_scale_health(
    app: &OpenScaleApp,
    scale_id: Uuid,
) -> Result<ScaleHealthCheckResult, AppError> {
    let scale = app
        .scale_repo()
        .get_by_id(app.store_id, scale_id)
        .await?
        .ok_or(AppError::ScaleNotRegistered(scale_id))?;

    let handle = match app.scales.get(scale_id).await {
        Ok(h) => h,
        Err(AppError::ScaleNotRegistered(_)) => app.scales.register_scale(&scale).await,
        Err(err) => return Err(err),
    };

    let previous_status = scale.status;
    let checked_at = Utc::now();

    match handle.health_check().await {
        Ok(()) => {
            app.scale_repo()
                .update_status(
                    app.store_id,
                    scale_id,
                    ScaleStatus::Online,
                    Some(checked_at),
                )
                .await?;

            if previous_status != ScaleStatus::Online {
                app.scales
                    .emit_connection_changed(scale_id, ConnectionState::Connected, None);
            }

            Ok(ScaleHealthCheckResult {
                scale_id,
                previous_status,
                status: ScaleStatus::Online,
                last_seen_at: Some(checked_at),
                checked_at,
                error: None,
            })
        }
        Err(err) => {
            let error_msg = err.to_string();
            app.scale_repo()
                .update_status(app.store_id, scale_id, ScaleStatus::Offline, None)
                .await?;

            if previous_status != ScaleStatus::Offline {
                app.scales.emit_connection_changed(
                    scale_id,
                    ConnectionState::Disconnected,
                    Some(error_msg.clone()),
                );
            }

            Ok(ScaleHealthCheckResult {
                scale_id,
                previous_status,
                status: ScaleStatus::Offline,
                last_seen_at: scale.last_seen_at,
                checked_at,
                error: Some(error_msg),
            })
        }
    }
}

/// Checks all active registered scales concurrently so a timed-out offline scale
/// does not delay health checks for the remaining scales.
pub async fn check_all_scales_health(
    app: &OpenScaleApp,
) -> Result<Vec<ScaleHealthCheckResult>, AppError> {
    let scales = app.list_scales().await?;
    let mut join_set = JoinSet::new();

    for scale in scales {
        let app_clone = app.clone();
        join_set.spawn(async move { check_scale_health(&app_clone, scale.id).await });
    }

    let mut results = Vec::new();
    while let Some(joined) = join_set.join_next().await {
        if let Ok(Ok(res)) = joined {
            results.push(res);
        }
    }

    Ok(results)
}

/// Spawns a background Tokio task that periodically monitors all registered scales
/// for network disconnection and recovery.
pub fn start_health_monitor(app: OpenScaleApp, interval: Duration) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;
            let _ = check_all_scales_health(&app).await;
        }
    })
}
