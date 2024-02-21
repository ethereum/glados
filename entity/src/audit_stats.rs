use chrono::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, QueryOrder, Set};
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
    };
    Ok(audit_stats.insert(conn).await?)
}

/// Get the most recent audit stat series of the last 7 days.
pub async fn get_recent_stats(conn: &DatabaseConnection) -> Result<Vec<Model>, DbErr> {
    let one_week_ago = Utc::now() - Duration::days(7);

    Entity::find()
        .filter(Column::Timestamp.gt(one_week_ago))
        .order_by_asc(Column::Timestamp)
        .all(conn)
        .await
}
