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
    pub success_rate_history_all: f32,
    pub success_rate_history_latest: f32,
    pub success_rate_history_random: f32,
    pub success_rate_history_oldest: f32,
    pub success_rate_history_four_fours: f32,
    pub success_rate_history_all_headers: f32,
    pub success_rate_history_all_headers_by_number: f32,
    pub success_rate_history_all_bodies: f32,
    pub success_rate_history_all_receipts: f32,
    pub success_rate_history_latest_headers: f32,
    pub success_rate_history_latest_headers_by_number: f32,
    pub success_rate_history_latest_bodies: f32,
    pub success_rate_history_latest_receipts: f32,
    pub success_rate_history_random_headers: f32,
    pub success_rate_history_random_headers_by_number: f32,
    pub success_rate_history_random_bodies: f32,
    pub success_rate_history_random_receipts: f32,
    pub success_rate_history_four_fours_headers: f32,
    pub success_rate_history_four_fours_headers_by_number: f32,
    pub success_rate_history_four_fours_bodies: f32,
    pub success_rate_history_four_fours_receipts: f32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    timestamp: DateTime<Utc>,
    num_audits: i32,
    success_rate_history_all: f32,
    success_rate_history_latest: f32,
    success_rate_history_random: f32,
    success_rate_history_oldest: f32,
    success_rate_history_four_fours: f32,
    success_rate_history_all_headers: f32,
    success_rate_history_all_headers_by_number: f32,
    success_rate_history_all_bodies: f32,
    success_rate_history_all_receipts: f32,
    success_rate_history_latest_headers: f32,
    success_rate_history_latest_headers_by_number: f32,
    success_rate_history_latest_bodies: f32,
    success_rate_history_latest_receipts: f32,
    success_rate_history_random_headers: f32,
    success_rate_history_random_headers_by_number: f32,
    success_rate_history_random_bodies: f32,
    success_rate_history_random_receipts: f32,
    success_rate_history_four_fours_headers: f32,
    success_rate_history_four_fours_headers_by_number: f32,
    success_rate_history_four_fours_bodies: f32,
    success_rate_history_four_fours_receipts: f32,
    conn: &DatabaseConnection,
) -> Result<Model> {
    let audit_stats = ActiveModel {
        id: NotSet,
        timestamp: Set(timestamp),
        num_audits: Set(num_audits),
        success_rate_history_all: Set(success_rate_history_all),
        success_rate_history_latest: Set(success_rate_history_latest),
        success_rate_history_random: Set(success_rate_history_random),
        success_rate_history_oldest: Set(success_rate_history_oldest),
        success_rate_history_four_fours: Set(success_rate_history_four_fours),
        success_rate_history_all_headers: Set(success_rate_history_all_headers),
        success_rate_history_all_headers_by_number: Set(success_rate_history_all_headers_by_number),
        success_rate_history_all_bodies: Set(success_rate_history_all_bodies),
        success_rate_history_all_receipts: Set(success_rate_history_all_receipts),
        success_rate_history_latest_headers: Set(success_rate_history_latest_headers),
        success_rate_history_latest_headers_by_number: Set(
            success_rate_history_latest_headers_by_number,
        ),
        success_rate_history_latest_bodies: Set(success_rate_history_latest_bodies),
        success_rate_history_latest_receipts: Set(success_rate_history_latest_receipts),
        success_rate_history_random_headers: Set(success_rate_history_random_headers),
        success_rate_history_random_headers_by_number: Set(
            success_rate_history_random_headers_by_number,
        ),
        success_rate_history_random_bodies: Set(success_rate_history_random_bodies),
        success_rate_history_random_receipts: Set(success_rate_history_random_receipts),
        success_rate_history_four_fours_headers: Set(success_rate_history_four_fours_headers),
        success_rate_history_four_fours_headers_by_number: Set(
            success_rate_history_four_fours_headers_by_number,
        ),
        success_rate_history_four_fours_bodies: Set(success_rate_history_four_fours_bodies),
        success_rate_history_four_fours_receipts: Set(success_rate_history_four_fours_receipts),
    };
    Ok(audit_stats.insert(conn).await?)
}

#[derive(Clone, Debug, Serialize, FromQueryResult)]
pub struct HistoryStats {
    id: i32,
    timestamp: DateTime<Utc>,
    num_audits: i32,
    success_rate_history_all: f32,
    success_rate_history_latest: f32,
    success_rate_history_random: f32,
    success_rate_history_oldest: f32,
    success_rate_history_four_fours: f32,
    success_rate_history_all_headers: f32,
    success_rate_history_all_bodies: f32,
    success_rate_history_all_receipts: f32,
    success_rate_history_latest_headers: f32,
    success_rate_history_latest_bodies: f32,
    success_rate_history_latest_receipts: f32,
    success_rate_history_random_headers: f32,
    success_rate_history_random_bodies: f32,
    success_rate_history_random_receipts: f32,
    success_rate_history_four_fours_headers: f32,
    success_rate_history_four_fours_bodies: f32,
    success_rate_history_four_fours_receipts: f32,
    success_rate_history_all_headers_by_number: f32,
    success_rate_history_latest_headers_by_number: f32,
    success_rate_history_random_headers_by_number: f32,
    success_rate_history_four_fours_headers_by_number: f32,
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
            Column::SuccessRateHistoryAll,
            Column::SuccessRateHistoryLatest,
            Column::SuccessRateHistoryRandom,
            Column::SuccessRateHistoryOldest,
            Column::SuccessRateHistoryFourFours,
            Column::SuccessRateHistoryAllHeaders,
            Column::SuccessRateHistoryAllHeadersByNumber,
            Column::SuccessRateHistoryAllBodies,
            Column::SuccessRateHistoryAllReceipts,
            Column::SuccessRateHistoryLatestHeaders,
            Column::SuccessRateHistoryLatestHeadersByNumber,
            Column::SuccessRateHistoryLatestBodies,
            Column::SuccessRateHistoryLatestReceipts,
            Column::SuccessRateHistoryRandomHeaders,
            Column::SuccessRateHistoryRandomHeadersByNumber,
            Column::SuccessRateHistoryRandomBodies,
            Column::SuccessRateHistoryRandomReceipts,
            Column::SuccessRateHistoryFourFoursHeaders,
            Column::SuccessRateHistoryFourFoursHeadersByNumber,
            Column::SuccessRateHistoryFourFoursBodies,
            Column::SuccessRateHistoryFourFoursReceipts,
        ])
        .filter(Column::Timestamp.gt(beginning))
        .filter(Column::Timestamp.lt(end))
        .order_by_asc(Column::Timestamp)
        .into_model::<HistoryStats>()
        .all(conn)
        .await
}
