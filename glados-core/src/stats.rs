use std::fmt;

use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::{Expr, IntoCondition},
    strum::EnumMessage,
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, JoinType, PaginatorTrait, QueryFilter,
    QuerySelect, QueryTrait, RelationTrait, Select,
};
use serde::{de, Deserialize, Serialize};

use entity::{
    content::{self, ContentType, SubProtocol},
    content_audit::{self, serialize_selection_strategy, AuditResult, SelectionStrategy},
};

/// Generates a SeaORM select query for audits based on the provided filters.
/// User can decide whether to retrieve or only count results.
/// TODO: add support for filtering by portal client
pub fn filter_audits(filters: AuditFilters) -> Select<content_audit::Entity> {
    // This base query will have filters added to it
    content_audit::Entity::find()
        .join(
            JoinType::Join,
            content_audit::Relation::Content
                .def()
                .on_condition(move |_left, _right| {
                    content::Column::ProtocolId
                        .eq(filters.network)
                        .into_condition()
                }),
        )
        // Strategy filters
        .apply_if(filters.strategy, |query, audit_strategy| {
            query.filter(content_audit::Column::StrategyUsed.eq(audit_strategy))
        })
        // Success filters
        .apply_if(filters.audit_result, |query, audit_result| {
            query.filter(content_audit::Column::Result.eq(audit_result))
        })
        // Content type filters
        .apply_if(filters.content_type, |query, content_type| {
            query.filter(
                Expr::cust(
                    &("get_byte(content.content_key, 0) = ".to_string()
                        + &content_type.to_string()),
                )
                .into_condition(),
            )
        })
}

/// Counts new content items for the given subprotocol and period
pub async fn get_new_content_count(
    subprotocol: SubProtocol,
    period: Period,
    conn: &DatabaseConnection,
) -> Result<u32, DbErr> {
    let cutoff = period.cutoff_time();

    let new_content = content::Entity::find()
        .filter(content::Column::ProtocolId.eq(subprotocol))
        .filter(content::Column::FirstAvailableAt.gt(cutoff))
        .count(conn)
        .await? as u32;
    Ok(new_content)
}

/// Calculates stats for the given set of audits over the given period.
pub async fn get_audit_stats(
    filtered: Select<content_audit::Entity>,
    period: Period,
    conn: &DatabaseConnection,
) -> Result<AuditStats, DbErr> {
    let cutoff = period.cutoff_time();

    let (total_audits, total_passes): (i64, i64) = (filtered
        .clone()
        .filter(content_audit::Column::CreatedAt.gt(cutoff))
        .select_only()
        .column_as(Expr::cust("COUNT(1)"), "total_audits")
        .column_as(
            Expr::cust("COALESCE(SUM(CASE WHEN result = 1 THEN 1 ELSE 0 END),0)"),
            "total_passes",
        )
        .into_tuple()
        .all(conn)
        .await?)[0];

    let total_failures = total_audits - total_passes;

    let audits_per_minute = (60 * total_audits as u32)
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
        total_audits: total_audits as u32,
        total_passes: total_passes as u32,
        pass_percent,
        total_failures: total_failures as u32,
        fail_percent,
        audits_per_minute,
    })
}

#[derive(Serialize)]
pub struct AuditStats {
    pub period: Period,
    pub total_audits: u32,
    pub total_passes: u32,
    pub pass_percent: f32,
    pub total_failures: u32,
    pub fail_percent: f32,
    pub audits_per_minute: u32,
}

#[derive(Serialize)]
pub enum Period {
    Hour,
    Day,
    Week,
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
            Period::Hour => chrono::TimeDelta::try_hours(1).unwrap(),
            Period::Day => chrono::TimeDelta::try_days(1).unwrap(),
            Period::Week => chrono::TimeDelta::try_weeks(1).unwrap(),
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

#[derive(Clone, Copy)]
pub struct AuditFilters {
    pub network: SubProtocol,
    pub strategy: Option<SelectionStrategy>,
    pub content_type: Option<ContentType>,
    pub audit_result: Option<AuditResult>,
}

impl<'de> de::Deserialize<'de> for AuditFilters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct IntermediateAuditFilters {
            network: SubProtocol,
            content_type: Option<ContentType>,
            audit_result: Option<AuditResult>,
            strategy: Option<String>,
        }

        let intermediate_filter = IntermediateAuditFilters::deserialize(deserializer)?;

        let strategy = match intermediate_filter.strategy {
            Some(strat) => Some(
                serialize_selection_strategy(intermediate_filter.network, &strat).map_err(
                    |_| {
                        de::Error::custom(format!(
                            "unknown variant for {}: {}",
                            intermediate_filter
                                .network
                                .get_message()
                                .expect("Subprotocol missing message"),
                            strat
                        ))
                    },
                )?,
            ),
            None => None,
        };

        Ok(AuditFilters {
            network: intermediate_filter.network,
            content_type: intermediate_filter.content_type,
            audit_result: intermediate_filter.audit_result,
            strategy,
        })
    }
}
