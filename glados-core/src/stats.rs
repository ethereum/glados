use std::fmt::Display;

use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::{Expr, IntoCondition},
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, JoinType, PaginatorTrait, QueryFilter,
    QuerySelect, RelationTrait, Select,
};
use serde::Deserialize;

use entity::{
    content::{self, SubProtocol},
    content_audit::{self, AuditResult, HistorySelectionStrategy, SelectionStrategy},
};

/// Generates a SeaORM select query for audits based on the provided filters.
/// User can decide whether to retrieve or only count results.
/// TODO: add support for filtering by portal client
pub fn filter_audits(filters: AuditFilters) -> Select<content_audit::Entity> {
    // This base query will have filters added to it
    let audits = content_audit::Entity::find();
    let audits = audits.join(
        JoinType::Join,
        content_audit::Relation::Content
            .def()
            .on_condition(move |_left, _right| {
                content::Column::ProtocolId
                    .eq(filters.network)
                    .into_condition()
            }),
    );
    // Strategy filters
    let audits = match filters.strategy {
        StrategyFilter::All => audits,
        StrategyFilter::Sync => audits.filter(
            content_audit::Column::StrategyUsed
                .eq(SelectionStrategy::History(HistorySelectionStrategy::Sync)),
        ),
        StrategyFilter::Random => audits.filter(
            content_audit::Column::StrategyUsed
                .eq(SelectionStrategy::History(HistorySelectionStrategy::Random)),
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
    // TODO(milos): Update to new content keys
    match filters.content_type {
        ContentTypeFilter::All => audits,
        ContentTypeFilter::HeadersByNumber => {
            audits.filter(Expr::cust("get_byte(content.content_key, 0) = 0x03").into_condition())
        }
        ContentTypeFilter::Bodies => {
            audits.filter(Expr::cust("get_byte(content.content_key, 0) = 0x03").into_condition())
        }
        ContentTypeFilter::Receipts => {
            audits.filter(Expr::cust("get_byte(content.content_key, 0) = 0x01").into_condition())
        }
    }
}

/// Calculates stats for the given set of audits over the given period.
pub async fn get_audit_stats(
    filtered: Select<content_audit::Entity>,
    period: Period,
    conn: &DatabaseConnection,
) -> Result<AuditStats, DbErr> {
    let cutoff = period.cutoff_time();

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

    // In case the numbers change in between queries, make sure passes don't exceed total audits
    let total_passes = std::cmp::min(total_passes, total_audits);

    let total_failures = total_audits - total_passes;

    let audits_per_minute = total_audits
        .checked_div(period.as_time_delta().num_minutes() as u32)
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
    fn as_time_delta(&self) -> chrono::TimeDelta {
        match self {
            Period::Hour => chrono::TimeDelta::hours(1),
            Period::Day => chrono::TimeDelta::days(1),
            Period::Week => chrono::TimeDelta::weeks(1),
        }
    }

    fn cutoff_time(&self) -> DateTime<Utc> {
        Utc::now() - self.as_time_delta()
    }
}

#[derive(Deserialize, Copy, Clone)]
pub struct AuditFilters {
    pub strategy: StrategyFilter,
    pub content_type: ContentTypeFilter,
    pub success: SuccessFilter,
    pub network: SubProtocol,
}

#[derive(Deserialize, Copy, Clone)]
pub enum StrategyFilter {
    All,
    Sync,
    Random,
}

impl Display for StrategyFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match &self {
            StrategyFilter::All => "All",
            StrategyFilter::Sync => "Sync",
            StrategyFilter::Random => "Random",
        };
        write!(f, "{}", name)
    }
}

#[derive(Deserialize, Copy, Clone)]
pub enum SuccessFilter {
    All,
    Success,
    Failure,
}

#[derive(Deserialize, Copy, Clone)]
pub enum ContentTypeFilter {
    All,
    HeadersByNumber,
    Bodies,
    Receipts,
}
