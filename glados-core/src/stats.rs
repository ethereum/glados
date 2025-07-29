use std::{collections::HashMap, fmt::Display};

use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::IntoCondition, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, JoinType,
    QueryFilter, QuerySelect, RelationTrait, Select,
};
use serde::Deserialize;

use entity::{
    audit, content, AuditResult, ContentType, HistorySelectionStrategy, SelectionStrategy,
    SubProtocol,
};

/// Generates a SeaORM select query for audits based on the provided filters.
///
/// User can decide whether to retrieve or only count results.
/// TODO: add support for filtering by portal client
pub fn filter_audits(filters: AuditFilters) -> Select<audit::Entity> {
    // This base query will have filters added to it
    let audits = audit::Entity::find();
    let audits = audits.join(
        JoinType::LeftJoin,
        audit::Relation::Content
            .def()
            .on_condition(move |_left, _right| {
                content::Column::SubProtocol
                    .eq(filters.sub_protocol)
                    .into_condition()
            }),
    );
    // Strategy filters
    let audits = match filters.strategy {
        StrategyFilter::All => audits,
        StrategyFilter::Sync => audits.filter(
            audit::Column::Strategy.eq(SelectionStrategy::History(HistorySelectionStrategy::Sync)),
        ),
        StrategyFilter::Random => audits.filter(
            audit::Column::Strategy
                .eq(SelectionStrategy::History(HistorySelectionStrategy::Random)),
        ),
    };
    // Success filters
    let audits = match filters.success {
        SuccessFilter::All => audits,
        SuccessFilter::Success => audits.filter(audit::Column::Result.eq(AuditResult::Success)),
        SuccessFilter::Failure => audits.filter(audit::Column::Result.eq(AuditResult::Failure)),
    };
    // Content type filters
    match filters.content_type {
        ContentTypeFilter::All => audits,
        ContentTypeFilter::Bodies => {
            audits.filter(content::Column::ContentType.eq(ContentType::BlockBodies))
        }
        ContentTypeFilter::Receipts => {
            audits.filter(content::Column::ContentType.eq(ContentType::BlockReceipts))
        }
    }
}

/// Calculates stats for the given set of audits over the given period.
pub async fn get_audit_stats(
    filtered: Select<audit::Entity>,
    period: Period,
    conn: &DatabaseConnection,
) -> Result<AuditStats, DbErr> {
    let cutoff = period.cutoff_time();

    let query = filtered
        .filter(audit::Column::CreatedAt.gt(cutoff))
        .select_only()
        .column(audit::Column::Result)
        .column_as(audit::Column::Result.count(), "count")
        .group_by(audit::Column::Result);

    let audit_result_count: HashMap<AuditResult, i64> = query
        .clone()
        .into_tuple::<(AuditResult, i64)>()
        .all(conn)
        .await?
        .into_iter()
        .collect();

    let total_passes = *audit_result_count.get(&AuditResult::Success).unwrap_or(&0) as u64;
    let total_failures = *audit_result_count.get(&AuditResult::Failure).unwrap_or(&0) as u64;
    let total_audits = total_passes + total_failures;

    let audits_per_minute = total_audits / (period.as_time_delta().num_minutes() as u64);

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
    pub total_audits: u64,
    pub total_passes: u64,
    pub pass_percent: f32,
    pub total_failures: u64,
    pub fail_percent: f32,
    pub audits_per_minute: u64,
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
    pub sub_protocol: SubProtocol,
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
    Bodies,
    Receipts,
}
