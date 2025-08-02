use std::time::Duration;

use chrono::{TimeDelta, Utc};
use entity::census;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{error, info};

const PERIOD: Duration = Duration::from_secs(60 * 60); // 1 hour

/// Loops indefinitely, periodically (once per hour) deletes old census data
pub async fn periodically_delete_old_census(
    retention_period: Option<TimeDelta>,
    conn: DatabaseConnection,
) {
    let Some(retention_period) = retention_period else {
        return;
    };
    info!(
        "Initializing task for deleting censuses older than {} days",
        retention_period.num_days(),
    );

    let mut interval = interval(PERIOD);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        interval.tick().await;

        match census::Entity::delete_many()
            .filter(census::Column::StartedAt.lt(Utc::now() - retention_period))
            .exec(&conn)
            .await
        {
            Ok(delete_result) => {
                info!(
                    "Deleted {} censuses older than {} days",
                    delete_result.rows_affected,
                    retention_period.num_days(),
                );
            }
            Err(err) => {
                error!(%err, "Failed to delete censuses");
            }
        }
    }
}
