use anyhow::Result;
use chrono::{DateTime, TimeDelta, Utc};
use sea_orm::{
    entity::prelude::*, ActiveValue::NotSet, FromQueryResult, QueryOrder, QuerySelect, Set,
};
use serde::Serialize;

#[derive(Clone, Debug, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "audit_stats")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub timestamp: DateTime<Utc>,
    pub num_audits: i32,
    pub success_rate_all: f32,
    pub success_rate_latest: f32,
    pub success_rate_random: f32,
    pub success_rate_oldest: f32,
    pub success_rate_four_fours: f32,
    pub success_rate_all_headers: f32,
    pub success_rate_all_bodies: f32,
    pub success_rate_all_receipts: f32,
    pub success_rate_latest_headers: f32,
    pub success_rate_latest_bodies: f32,
    pub success_rate_latest_receipts: f32,
    pub success_rate_random_headers: f32,
    pub success_rate_random_bodies: f32,
    pub success_rate_random_receipts: f32,
    pub success_rate_four_fours_headers: f32,
    pub success_rate_four_fours_bodies: f32,
    pub success_rate_four_fours_receipts: f32,
    pub success_rate_state_all: f32,
    pub success_rate_state_latest: f32,
    pub success_rate_state_state_roots: f32,
    pub success_rate_beacon_all: f32,
    pub success_rate_beacon_latest: f32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    timestamp: DateTime<Utc>,
    num_audits: i32,
    success_rate_all: f32,
    success_rate_latest: f32,
    success_rate_random: f32,
    success_rate_oldest: f32,
    success_rate_four_fours: f32,
    success_rate_all_headers: f32,
    success_rate_all_bodies: f32,
    success_rate_all_receipts: f32,
    success_rate_latest_headers: f32,
    success_rate_latest_bodies: f32,
    success_rate_latest_receipts: f32,
    success_rate_random_headers: f32,
    success_rate_random_bodies: f32,
    success_rate_random_receipts: f32,
    success_rate_four_fours_headers: f32,
    success_rate_four_fours_bodies: f32,
    success_rate_four_fours_receipts: f32,
    success_rate_state_all: f32,
    success_rate_state_latest: f32,
    success_rate_state_state_roots: f32,
    success_rate_beacon_all: f32,
    success_rate_beacon_latest: f32,
    conn: &DatabaseConnection,
) -> Result<Model> {
    let audit_stats = ActiveModel {
        id: NotSet,
        timestamp: Set(timestamp),
        num_audits: Set(num_audits),
        success_rate_all: Set(success_rate_all),
        success_rate_latest: Set(success_rate_latest),
        success_rate_random: Set(success_rate_random),
        success_rate_oldest: Set(success_rate_oldest),
        success_rate_four_fours: Set(success_rate_four_fours),
        success_rate_all_headers: Set(success_rate_all_headers),
        success_rate_all_bodies: Set(success_rate_all_bodies),
        success_rate_all_receipts: Set(success_rate_all_receipts),
        success_rate_latest_headers: Set(success_rate_latest_headers),
        success_rate_latest_bodies: Set(success_rate_latest_bodies),
        success_rate_latest_receipts: Set(success_rate_latest_receipts),
        success_rate_random_headers: Set(success_rate_random_headers),
        success_rate_random_bodies: Set(success_rate_random_bodies),
        success_rate_random_receipts: Set(success_rate_random_receipts),
        success_rate_four_fours_headers: Set(success_rate_four_fours_headers),
        success_rate_four_fours_bodies: Set(success_rate_four_fours_bodies),
        success_rate_four_fours_receipts: Set(success_rate_four_fours_receipts),
        success_rate_state_all: Set(success_rate_state_all),
        success_rate_state_latest: Set(success_rate_state_latest),
        success_rate_state_state_roots: Set(success_rate_state_state_roots),
        success_rate_beacon_all: Set(success_rate_beacon_all),
        success_rate_beacon_latest: Set(success_rate_beacon_latest),
    };
    Ok(audit_stats.insert(conn).await?)
}

#[derive(Clone, Debug, Serialize, FromQueryResult)]
pub struct HistoryStats {
    id: i32,
    timestamp: DateTime<Utc>,
    num_audits: i32,
    success_rate_all: f32,
    success_rate_latest: f32,
    success_rate_random: f32,
    success_rate_oldest: f32,
    success_rate_four_fours: f32,
    success_rate_all_headers: f32,
    success_rate_all_bodies: f32,
    success_rate_all_receipts: f32,
    success_rate_latest_headers: f32,
    success_rate_latest_bodies: f32,
    success_rate_latest_receipts: f32,
    success_rate_random_headers: f32,
    success_rate_random_bodies: f32,
    success_rate_random_receipts: f32,
    success_rate_four_fours_headers: f32,
    success_rate_four_fours_bodies: f32,
    success_rate_four_fours_receipts: f32,
}

#[derive(Clone, Debug, Serialize, FromQueryResult)]
pub struct StateStats {
    id: i32,
    timestamp: DateTime<Utc>,
    success_rate_state_all: f32,
    success_rate_state_latest: f32,
    success_rate_state_state_roots: f32,
}

#[derive(Clone, Debug, Serialize, FromQueryResult)]
pub struct BeaconStats {
    id: i32,
    timestamp: DateTime<Utc>,
    success_rate_beacon_all: f32,
    success_rate_beacon_latest: f32,
}

fn compute_week_period(weeks_ago: i32) -> (DateTime<Utc>, DateTime<Utc>) {
    let beginning_days_ago =
        TimeDelta::try_days(7 * (weeks_ago + 1) as i64).expect("Couldn't calculate days ago.");
    let seven_days = TimeDelta::try_days(7).expect("Couldn't calculate 7 day delta.");

    let beginning = Utc::now() - beginning_days_ago;
    let end = beginning + seven_days;

    (beginning, end)
}

// Get 7 days of history audit stat series.
pub async fn get_weekly_history_stats(
    conn: &DatabaseConnection,
    weeks_ago: i32,
) -> Result<Vec<HistoryStats>, DbErr> {
    let (beginning, end) = compute_week_period(weeks_ago);

    Entity::find()
        .select_only()
        .columns([
            Column::Id,
            Column::Timestamp,
            Column::NumAudits,
            Column::SuccessRateAll,
            Column::SuccessRateLatest,
            Column::SuccessRateRandom,
            Column::SuccessRateOldest,
            Column::SuccessRateFourFours,
            Column::SuccessRateAllHeaders,
            Column::SuccessRateAllBodies,
            Column::SuccessRateAllReceipts,
            Column::SuccessRateLatestHeaders,
            Column::SuccessRateLatestBodies,
            Column::SuccessRateLatestReceipts,
            Column::SuccessRateRandomHeaders,
            Column::SuccessRateRandomBodies,
            Column::SuccessRateRandomReceipts,
            Column::SuccessRateFourFoursHeaders,
            Column::SuccessRateFourFoursBodies,
            Column::SuccessRateFourFoursReceipts,
        ])
        .filter(Column::Timestamp.gt(beginning))
        .filter(Column::Timestamp.lt(end))
        .order_by_asc(Column::Timestamp)
        .into_model::<HistoryStats>()
        .all(conn)
        .await
}
/// Get 7 days of state audit stat series.
pub async fn get_weekly_state_stats(
    conn: &DatabaseConnection,
    weeks_ago: i32,
) -> Result<Vec<StateStats>, DbErr> {
    let (beginning, end) = compute_week_period(weeks_ago);

    Entity::find()
        .select_only()
        .columns([
            Column::Id,
            Column::Timestamp,
            Column::SuccessRateStateAll,
            Column::SuccessRateStateLatest,
            Column::SuccessRateStateStateRoots,
        ])
        .filter(Column::Timestamp.gt(beginning))
        .filter(Column::Timestamp.lt(end))
        .order_by_asc(Column::Timestamp)
        .into_model::<StateStats>()
        .all(conn)
        .await
}
/// Get 7 days of beacon audit stat series.
pub async fn get_weekly_beacon_stats(
    conn: &DatabaseConnection,
    weeks_ago: i32,
) -> Result<Vec<BeaconStats>, DbErr> {
    let (beginning, end) = compute_week_period(weeks_ago);

    Entity::find()
        .select_only()
        .columns([
            Column::Id,
            Column::Timestamp,
            Column::SuccessRateBeaconAll,
            Column::SuccessRateBeaconLatest,
        ])
        .filter(Column::Timestamp.gt(beginning))
        .filter(Column::Timestamp.lt(end))
        .order_by_asc(Column::Timestamp)
        .into_model::<BeaconStats>()
        .all(conn)
        .await
}
