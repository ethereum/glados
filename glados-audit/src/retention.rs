use std::time::Duration;

use chrono::{TimeDelta, Utc};
use entity::audit;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{error, info};

use crate::config::AuditConfig;

const PERIOD: Duration = Duration::from_secs(10); // 10 seconds

/// Loops indefinitely, periodically (once 10 seconds) deletes old audits
pub async fn periodically_delete_old_audits(config: AuditConfig) {
    let Some(retention_period) = config.retention_period else {
        return;
    };
    info!(
        "initializing task for deleting audits older than {} days",
        retention_period.num_days()
    );

    let mut interval = interval(PERIOD);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        interval.tick().await;

        delete_old_audits(retention_period, &config.database_connection).await;
    }
}

pub async fn delete_old_audits(retention_period: TimeDelta, conn: &DatabaseConnection) {
    match audit::Entity::delete_many()
        .filter(audit::Column::CreatedAt.lt(Utc::now() - retention_period))
        .exec(conn)
        .await
    {
        Ok(delete_result) => {
            info!(
                "Deleted {} audits older than {} days",
                delete_result.rows_affected,
                retention_period.num_days(),
            );
        }
        Err(err) => {
            error!(%err, "Failed to delete too old audits");
        }
    }
}
