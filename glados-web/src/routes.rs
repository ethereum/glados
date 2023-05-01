use std::sync::Arc;
use std::{fmt::Display, io};

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Duration, Utc};
use entity::{
    content,
    content_audit::{self, AuditResult},
    execution_metadata, key_value, node, record,
};
use ethportal_api::types::content_key::{HistoryContentKey, OverlayContentKey};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use tracing::error;
use tracing::info;
use trin_utils::bytes::{hex_decode, hex_encode};

use crate::state::State;
use crate::templates::{
    ContentAuditDetailTemplate, ContentDashboardTemplate, ContentIdDetailTemplate,
    ContentIdListTemplate, ContentKeyDetailTemplate, ContentKeyListTemplate, EnrDetailTemplate,
    HtmlTemplate, IndexTemplate, NetworkDashboardTemplate, NodeDetailTemplate,
};

//
// Routes
//
pub async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}

pub async fn root(Extension(_state): Extension<Arc<State>>) -> impl IntoResponse {
    let template = IndexTemplate {};
    HtmlTemplate(template)
}

pub async fn network_dashboard(
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<NetworkDashboardTemplate>, StatusCode> {
    const KEY_COUNT: u64 = 20;

    let recent_node_list = node::Entity::find()
        .order_by_desc(node::Column::Id)
        .limit(KEY_COUNT)
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(key.count=KEY_COUNT, err=?e, "Could not look up recent nodes");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let total_node_count = node::Entity::find()
        .count(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Error looking up total Node count");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let recent_enr_list: Vec<(record::Model, node::Model)> = record::Entity::find()
        .order_by_desc(record::Column::Id)
        .find_also_related(node::Entity)
        .limit(KEY_COUNT)
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(key.count=KEY_COUNT, err=?e, "Could not look up recent ENR records");
            StatusCode::INTERNAL_SERVER_ERROR
        })
        .unwrap()
        .iter()
        .filter_map(|(r, n)| {
            n.as_ref()
                .map(|enr_node| (r.to_owned(), enr_node.to_owned()))
        })
        .collect();

    let total_enr_count = record::Entity::find()
        .count(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Error looking up total ENR count");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let template = NetworkDashboardTemplate {
        total_node_count,
        total_enr_count,
        recent_node_list,
        recent_enr_list,
    };
    Ok(HtmlTemplate(template))
}

pub async fn node_detail(
    Path(node_id_hex): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<NodeDetailTemplate>, StatusCode> {
    const KEY_COUNT: u64 = 50;
    let node_id = hex_decode(&node_id_hex).map_err(|e| {
        error!(node_id=node_id_hex, err=?e, "Could not decode proved node_id");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let node_model = node::Entity::find()
        .filter(node::Column::NodeId.eq(node_id))
        .one(&state.database_connection)
        .await
        .map_err(|e| {
            error!(node_id=node_id_hex, err=?e, "No record found for node_id");
            StatusCode::NOT_FOUND
        })
        .unwrap()
        .unwrap();
    let enr_list = record::Entity::find()
        .filter(record::Column::NodeId.eq(node_model.id))
        .order_by_desc(record::Column::SequenceNumber)
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(node.node_id=node_id_hex, node.db_id=node_model.id, err=?e, "Error looking up ENRs");
            StatusCode::NOT_FOUND
        })?;
    let closest_node_list = node::closest_xor(node_model.get_node_id(), &state.database_connection)
        .await
        .unwrap();

    let latest_enr = enr_list.get(0).cloned();

    let latest_enr_key_value_list = match &latest_enr {
        Some(enr) => Some(
            key_value::Entity::find()
                .filter(key_value::Column::RecordId.eq(enr.id))
                .order_by_asc(key_value::Column::Key)
                .all(&state.database_connection)
                .await
                .map_err(|e| {
                    error!(enr.id=enr.id, err=?e, "Error looking up key_value pairs");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?,
        ),
        None => None,
    };
    let template = NodeDetailTemplate {
        node: node_model,
        latest_enr,
        latest_enr_key_value_list,
        enr_list,
        closest_node_list,
    };
    Ok(HtmlTemplate(template))
}

pub async fn enr_detail(
    Path((node_id_hex, enr_seq)): Path<(String, u64)>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<EnrDetailTemplate>, StatusCode> {
    const KEY_COUNT: u64 = 50;
    let node_id = hex_decode(&node_id_hex).map_err(|e| {
        error!(node_id=node_id_hex, err=?e, "Could not decode proved node_id");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let node_model = node::Entity::find()
        .filter(node::Column::NodeId.eq(node_id))
        .one(&state.database_connection)
        .await
        .map_err(|e| {
            error!(node_id=node_id_hex, err=?e, "No record found for node_id");
            StatusCode::NOT_FOUND
        })
        .unwrap()
        .unwrap();
    let enr = record::Entity::find()
        .filter(record::Column::NodeId.eq(node_model.id.to_owned()))
        .filter(record::Column::SequenceNumber.eq(enr_seq))
        .one(&state.database_connection)
        .await
        .map_err(|e| {
            error!(enr.node_id=node_id_hex, enr.seq=enr_seq, err=?e, "No record found for node_id and sequence_number");
            StatusCode::NOT_FOUND
        })
        .unwrap()
        .unwrap();
    let key_value_list = key_value::Entity::find()
        .filter(key_value::Column::RecordId.eq(enr.id))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(enr.id=enr.id, enr.node_id=node_id_hex, err=?e, "Error looking up key_value pairs");
            StatusCode::NOT_FOUND
        })?;

    let template = EnrDetailTemplate {
        node: node_model,
        enr,
        key_value_list,
    };
    Ok(HtmlTemplate(template))
}

pub async fn content_dashboard(
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<ContentDashboardTemplate>, StatusCode> {
    const KEY_COUNT: u64 = 20;
    let contentid_list = content::Entity::find()
        .order_by_desc(content::Column::FirstAvailableAt)
        .limit(KEY_COUNT)
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(key.count=KEY_COUNT, err=?e, "Could not look up latest keys");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let recent_content_model: Vec<(content::Model, Vec<content_audit::Model>)> =
        content::Entity::find()
            .order_by_desc(content::Column::FirstAvailableAt)
            .find_with_related(content_audit::Entity)
            .filter(content_audit::Column::Result.is_not_null())
            .limit(KEY_COUNT)
            .all(&state.database_connection)
            .await
            .map_err(|e| {
                error!(key.count=KEY_COUNT, err=?e, "Could not look up latest keys with audits");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let recent_audits_model: Vec<(content_audit::Model, Vec<content::Model>)> =
        content_audit::Entity::find()
            .order_by_desc(content_audit::Column::CreatedAt)
            .find_with_related(content::Entity)
            .limit(KEY_COUNT)
            .all(&state.database_connection)
            .await
            .map_err(|e| {
                error!(key.count=KEY_COUNT, err=?e, "Could not look up recent audits");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let recent_audit_success_model: Vec<(content_audit::Model, Vec<content::Model>)> =
        content_audit::Entity::find()
            .order_by_desc(content_audit::Column::CreatedAt)
            .find_with_related(content::Entity)
            .filter(content_audit::Column::Result.eq(AuditResult::Success))
            .limit(KEY_COUNT)
            .all(&state.database_connection)
            .await
            .map_err(|e| {
                error!(key.count=KEY_COUNT, err=?e, "Could not look up recent successful audits");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let recent_audit_failure_model: Vec<(content_audit::Model, Vec<content::Model>)> =
        content_audit::Entity::find()
            .order_by_desc(content_audit::Column::CreatedAt)
            .find_with_related(content::Entity)
            .filter(content_audit::Column::Result.eq(AuditResult::Failure))
            .limit(KEY_COUNT)
            .all(&state.database_connection)
            .await
            .map_err(|e| {
                error!(key.count=KEY_COUNT, err=?e, "Could not look up recent failed audits");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let template = ContentDashboardTemplate {
        stats: [
            get_audit_stats(Period::Hour, &state.database_connection).await?,
            get_audit_stats(Period::Day, &state.database_connection).await?,
            get_audit_stats(Period::Week, &state.database_connection).await?,
        ],
        contentid_list,
        recent_content: content_model_to_display(recent_content_model)?,
        recent_audits: audit_model_to_display(recent_audits_model)?,
        recent_audit_successes: audit_model_to_display(recent_audit_success_model)?,
        recent_audit_failures: audit_model_to_display(recent_audit_failure_model)?,
    };
    Ok(HtmlTemplate(template))
}

/// Summary of a model result (content with vector of audits).
fn content_model_to_display(
    content_model: Vec<(content::Model, Vec<content_audit::Model>)>,
) -> Result<Vec<(content::Model, content_audit::Model)>, StatusCode> {
    content_model
        .into_iter()
        .map(|content| {
            let content_data = content.0;
            let mut audits = content.1;
            audits.sort_by(|a, b| a.id.cmp(&b.id));
            let latest_audit: Result<entity::content_audit::Model, StatusCode> = audits
                .into_iter()
                // Choose the latest audit.
                .rev()
                .next()
                // We know there will beat least one audit because we filtered nulls out after joining.
                .ok_or_else(|| {
                    error!("Expected content to have at least one associated audit.");
                    StatusCode::INTERNAL_SERVER_ERROR
                });
            match latest_audit {
                Ok(a) => Ok((content_data, a)),
                Err(e) => Err(e),
            }
        })
        .collect()
}

/// Summary of a model result (audit with vector of content).
fn audit_model_to_display(
    content_model: Vec<(content_audit::Model, Vec<content::Model>)>,
) -> Result<Vec<(content::Model, content_audit::Model)>, StatusCode> {
    content_model
        .into_iter()
        .map(|content| {
            let audit = content.0;
            let content_data = content.1;
            let content_data: Result<entity::content::Model, StatusCode> = content_data
                .into_iter()
                .next()
                // We know there will be one content because audits only have one one content foreign key.
                .ok_or_else(|| {
                    error!("Expected audit to have at least one associated content.");
                    StatusCode::INTERNAL_SERVER_ERROR
                });
            match content_data {
                Ok(c) => Ok((c, audit)),
                Err(e) => Err(e),
            }
        })
        .collect()
}

pub async fn contentid_list(
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<ContentIdListTemplate>, StatusCode> {
    const KEY_COUNT: u64 = 50;
    let contentid_list: Vec<content::Model> = content::Entity::find()
        .order_by_asc(content::Column::ContentId)
        .limit(KEY_COUNT)
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(key.count=KEY_COUNT, err=?e, "Could not look up ids");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let template = ContentIdListTemplate { contentid_list };
    Ok(HtmlTemplate(template))
}

pub async fn contentid_detail(
    Path(content_id_hex): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<ContentIdDetailTemplate>, StatusCode> {
    let content_id_raw = hex_decode(&content_id_hex).map_err(|e| {
        error!(content.id=content_id_hex, err=?e, "Could not decode up id bytes");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let content_id = content::Entity::find()
        .filter(content::Column::ContentId.eq(content_id_raw.clone()))
        .one(&state.database_connection)
        .await
        .map_err(|e| {
            error!(content.id=content_id_hex, err=?e, "Could not look up id");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            error!(content.id = content_id_hex, "No data for id");
            StatusCode::NOT_FOUND
        })?;

    let contentkey_list = content::Entity::find()
        .filter(content::Column::ContentId.eq(content_id_raw))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(content.id=content_id_hex, err=?e, "Could not content keys for id");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let template = ContentIdDetailTemplate {
        content_id,
        contentkey_list,
    };
    Ok(HtmlTemplate(template))
}

pub async fn contentkey_list(
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<ContentKeyListTemplate>, StatusCode> {
    const KEY_COUNT: u64 = 50;
    let contentkey_list: Vec<content::Model> = content::Entity::find()
        .order_by_desc(content::Column::Id)
        .limit(KEY_COUNT)
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(key.count=KEY_COUNT, err=?e, "Could not look up keys");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let template = ContentKeyListTemplate { contentkey_list };
    Ok(HtmlTemplate(template))
}

/// Retrieves key details to display.
///
/// At present this assumes it is a HistoryContentKey.
pub async fn contentkey_detail(
    Path(content_key_hex): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<ContentKeyDetailTemplate>, StatusCode> {
    let content_key_raw = hex_decode(&content_key_hex).map_err(|e| {
        error!(content.key=content_key_hex, err=?e, "Could not decode up key bytes");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let content_key_model = content::Entity::find()
        .filter(content::Column::ContentKey.eq(content_key_raw.clone()))
        .one(&state.database_connection)
        .await
        .map_err(|e| {
            error!(content.key=content_key_hex, err=?e, "Could not look up key");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            error!(content.key = content_key_hex, "No data for key");
            StatusCode::NOT_FOUND
        })?;

    let contentaudit_list = content_key_model
        .find_related(content_audit::Entity)
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(content.key=content_key_hex, err=?e, "Could not look up audits for key");
            StatusCode::NOT_FOUND
        })?;

    let content_key: HistoryContentKey = HistoryContentKey::try_from(content_key_raw.clone())
        .map_err(|e| {
            error!(content.key=content_key_hex, err=?e, "Could not create key from bytes.");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let metadata_model = execution_metadata::Entity::find()
        .filter(execution_metadata::Column::Content.eq(content_key_model.id))
        .one(&state.database_connection)
        .await
        .map_err(|e| {
            error!(content.key=content_key_hex, err=?e, "No content metadata found");
            StatusCode::NOT_FOUND
        })?;
    let block_number = metadata_model.map(|m| m.block_number);

    let content_id = hex_encode(content_key.content_id());
    let content_kind = content_key.to_string();
    let template = ContentKeyDetailTemplate {
        content_key: content_key_hex,
        content_key_model,
        contentaudit_list,
        content_id,
        content_kind,
        block_number,
    };
    Ok(HtmlTemplate(template))
}

pub async fn contentaudit_detail(
    Path(audit_id): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    let audit_id = audit_id.parse::<i32>().unwrap();
    info!("Audit ID: {}", audit_id);
    let audit = content_audit::Entity::find_by_id(audit_id)
        .one(&state.database_connection)
        .await
        .unwrap()
        .expect("No audit found");

    let content = audit
        .find_related(content::Entity)
        .one(&state.database_connection)
        .await
        .unwrap()
        .expect("Failed to get audit content key");

    let template = ContentAuditDetailTemplate { audit, content };
    HtmlTemplate(template)
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

pub struct Stats {
    pub period: Period,
    pub new_content: u32,
    pub total_audits: u32,
    pub total_passes: u32,
    pub passes_per_100: u32,
    pub total_failures: u32,
    pub failures_per_100: u32,
    pub audits_per_minute: u32,
}

async fn get_audit_stats(period: Period, conn: &DatabaseConnection) -> Result<Stats, StatusCode> {
    let cutoff = period.cutoff_time();
    let new_content = content::Entity::find()
        .filter(content::Column::FirstAvailableAt.gt(cutoff))
        .all(conn)
        .await
        .map_err(|e| {
            error!(err=?e, "Could not look up audit stats");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .len() as u32;

    let total_audits = content_audit::Entity::find()
        .filter(content_audit::Column::CreatedAt.gt(cutoff))
        .all(conn)
        .await
        .map_err(|e| {
            error!(err=?e, "Could not look up audit stats");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .len() as u32;

    let total_passes = content_audit::Entity::find()
        .filter(content_audit::Column::CreatedAt.gt(cutoff))
        .filter(content_audit::Column::Result.eq(AuditResult::Success))
        .all(conn)
        .await
        .map_err(|e| {
            error!(err=?e, "Could not look up audit stats");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .len() as u32;
    let total_failures = total_audits - total_passes;
    let audits_per_minute = (60 * total_audits)
        .checked_div(period.total_seconds())
        .unwrap_or(0);
    let passes_per_100 = (100 * total_passes).checked_div(total_audits).unwrap_or(0);
    let failures_per_100 = (100 * total_failures)
        .checked_div(total_audits)
        .unwrap_or(0);
    Ok(Stats {
        period,
        new_content,
        total_audits,
        total_passes,
        passes_per_100,
        total_failures,
        failures_per_100,
        audits_per_minute,
    })
}
