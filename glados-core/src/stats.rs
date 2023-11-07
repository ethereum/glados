use std::fmt::Display;

use chrono::{DateTime, Duration, Utc};

use entity::{
    content::{self, SubProtocol},
    content_audit::{self, AuditResult, SelectionStrategy},
};
use sea_orm::{
    sea_query::{Expr, IntoCondition},
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, JoinType, PaginatorTrait, QueryFilter,
    QuerySelect, RelationTrait, Select,
};
use serde::Deserialize;

/// Generates a SeaORM select query for audits based on the provided filters.
/// User can decide whether to retrieve or only count results.
/// TODO: add support for filtering by portal client
pub fn filter_audits(filters: AuditFilters) -> Select<content_audit::Entity> {
    // This base query will have filters added to it
    let audits = content_audit::Entity::find();
    // Strategy filters
    let audits = match filters.strategy {
        StrategyFilter::All => audits,
        StrategyFilter::Random => {
            audits.filter(content_audit::Column::StrategyUsed.eq(SelectionStrategy::Random))
        }
        StrategyFilter::Latest => {
            audits.filter(content_audit::Column::StrategyUsed.eq(SelectionStrategy::Latest))
        }
        StrategyFilter::Oldest => audits.filter(
            content_audit::Column::StrategyUsed.eq(SelectionStrategy::SelectOldestUnaudited),
        ),
    };
    // Success filters
    let audits = match filters.success {
        SuccessFilter::All => audits,
        SuccessFilter::Success => {
            audits.filter(content_audit::Column::Result.eq(AuditResult::Success))
        }
        SuccessFilter::Failure => {
            audits.filter(content_audit::Column::Result.eq(AuditResult::Failure))
        }
    };
    // Content type filters
    match filters.content_type {
        ContentTypeFilter::All => audits,
        ContentTypeFilter::Headers => audits.join(
            JoinType::InnerJoin,
            content_audit::Relation::Content
                .def()
                .on_condition(|_left, right| {
                    Expr::cust("get_byte(content.content_key, 0) = 0x00")
                        .and(
                            Expr::col((right, content::Column::ProtocolId))
                                .eq(SubProtocol::History),
                        )
                        .into_condition()
                }),
        ),
        ContentTypeFilter::Bodies => audits.join(
            JoinType::InnerJoin,
            content_audit::Relation::Content
                .def()
                .on_condition(|_left, right| {
                    Expr::cust("get_byte(content.content_key, 0) = 0x01")
                        .and(
                            Expr::col((right, content::Column::ProtocolId))
                                .eq(SubProtocol::History),
                        )
                        .into_condition()
                }),
        ),
        ContentTypeFilter::Receipts => audits.join(
            JoinType::InnerJoin,
            content_audit::Relation::Content
                .def()
                .on_condition(|_left, right| {
                    Expr::cust("get_byte(content.content_key, 0) = 0x02")
                        .and(
                            Expr::col((right, content::Column::ProtocolId))
                                .eq(SubProtocol::History),
                        )
                        .into_condition()
                }),
        ),
    }
}

/// Calculates stats for the given set of audits over the given period.
pub async fn get_audit_stats(
    filtered: Select<content_audit::Entity>,
    period: Period,
    conn: &DatabaseConnection,
) -> Result<AuditStats, DbErr> {
    let cutoff = period.cutoff_time();

    let new_content = content::Entity::find()
        .filter(content::Column::FirstAvailableAt.gt(cutoff))
        .count(conn)
        .await? as u32;

    let total_audits = filtered
        .clone()
        .filter(content_audit::Column::CreatedAt.gt(cutoff))
        .count(conn)
        .await? as u32;

    let total_passes = filtered
        .filter(content_audit::Column::CreatedAt.gt(cutoff))
        .filter(content_audit::Column::Result.eq(AuditResult::Success))
        .count(conn)
        .await? as u32;

    let total_failures = total_audits - total_passes;

    let audits_per_minute = (60 * total_audits)
        .checked_div(period.total_seconds())
        .unwrap_or(0);

    let (pass_percent, fail_percent) = if total_audits == 0 {
        (0.0, 0.0)
    } else {
        let total_audits = total_audits as f32;
        (
            (total_passes as f32) * 100.0 / total_audits,
            (total_failures as f32) * 100.0 / total_audits,
        )
    };

    Ok(AuditStats {
        period,
        new_content,
        total_audits,
        total_passes,
        pass_percent,
        total_failures,
        fail_percent,
        audits_per_minute,
    })
}

pub struct AuditStats {
    pub period: Period,
    pub new_content: u32,
    pub total_audits: u32,
    pub total_passes: u32,
    pub pass_percent: f32,
    pub total_failures: u32,
    pub fail_percent: f32,
    pub audits_per_minute: u32,
}

pub enum Period {
    Hour,
    Day,
    Week,
}

impl Display for Period {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time_period = match self {
            Period::Hour => "hour",
            Period::Day => "day",
            Period::Week => "week",
        };
        write!(f, "Last {time_period}")
    }
}

impl Period {
    fn cutoff_time(&self) -> DateTime<Utc> {
        let duration = match self {
            Period::Hour => Duration::hours(1),
            Period::Day => Duration::days(1),
            Period::Week => Duration::weeks(1),
        };
        Utc::now() - duration
    }

    fn total_seconds(&self) -> u32 {
        match self {
            Period::Hour => 3600,
            Period::Day => 86400,
            Period::Week => 604800,
        }
    }
}

#[derive(Deserialize)]
pub struct AuditFilters {
    pub strategy: StrategyFilter,
    pub content_type: ContentTypeFilter,
    pub success: SuccessFilter,
}

#[derive(Deserialize)]
pub enum StrategyFilter {
    All,
    Random,
    Latest,
    Oldest,
}

#[derive(Deserialize)]
pub enum SuccessFilter {
    All,
    Success,
    Failure,
}

#[derive(Deserialize)]
pub enum ContentTypeFilter {
    All,
    Headers,
    Bodies,
    Receipts,
}
