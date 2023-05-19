use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Duration, Utc};
use entity::client_info;
use entity::{
    content,
    content_audit::{self, AuditResult},
    execution_metadata, key_value, node, record,
};
use ethportal_api::types::content_key::{HistoryContentKey, OverlayContentKey};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, LoaderTrait, ModelTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
use std::sync::Arc;
use std::{fmt::Display, io};
use tracing::error;
use tracing::info;
use trin_utils::bytes::{hex_decode, hex_encode};

use crate::templates::{
    ContentAuditDetailTemplate, ContentDashboardTemplate, ContentIdDetailTemplate,
    ContentIdListTemplate, ContentKeyDetailTemplate, ContentKeyListTemplate, EnrDetailTemplate,
    HtmlTemplate, IndexTemplate, NetworkDashboardTemplate, NodeDetailTemplate,
};
use crate::{state::State, templates::AuditTuple};

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

    let audits_of_recent_content: Vec<AuditTuple> =
        get_audits_for_recent_content(KEY_COUNT, &state.database_connection).await?;

    let recent_audits: Vec<AuditTuple> =
        get_recent_audits(KEY_COUNT, &state.database_connection).await?;
    let recent_audit_successes: Vec<AuditTuple> =
        get_recent_audit_successes(KEY_COUNT, &state.database_connection).await?;
    let recent_audit_failures: Vec<AuditTuple> =
        get_recent_audit_failures(KEY_COUNT, &state.database_connection).await?;

    let template = ContentDashboardTemplate {
        stats: [
            get_audit_stats(Period::Hour, &state.database_connection).await?,
            get_audit_stats(Period::Day, &state.database_connection).await?,
            get_audit_stats(Period::Week, &state.database_connection).await?,
        ],
        contentid_list,
        audits_of_recent_content,
        recent_audits,
        recent_audit_successes,
        recent_audit_failures,
    };
    Ok(HtmlTemplate(template))
}

pub async fn get_recent_audits(
    num_audits: u64,
    conn: &DatabaseConnection,
) -> Result<Vec<AuditTuple>, StatusCode> {
    let recent_audits: Vec<content_audit::Model> = content_audit::Entity::find()
        .order_by_desc(content_audit::Column::CreatedAt)
        .limit(num_audits)
        .all(conn)
        .await
        .map_err(|e| {
            error!(key.count=num_audits, err=?e, "Could not look up recent audits");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    get_audit_tuples_from_audit_models(recent_audits, conn).await
}

pub async fn get_audits_for_recent_content(
    num_content: u64,
    conn: &DatabaseConnection,
) -> Result<Vec<AuditTuple>, StatusCode> {
    // Get recent content that has been audited along with the audit.
    // Done in a single query and then split using unzip.
    let (recent_content, audits): (Vec<content::Model>, Vec<content_audit::Model>) =
        content::Entity::find()
            .order_by_desc(content::Column::FirstAvailableAt)
            .find_with_related(content_audit::Entity)
            .filter(content_audit::Column::Result.is_not_null())
            .limit(num_content)
            .all(conn)
            .await
            .map_err(|e| {
                error!(key.count=num_content, err=?e, "Could not look up latest keys with audits");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .into_iter()
            .filter_map(|(content, audits)| audits.into_iter().next().map(|audit| (content, audit)))
            .unzip();

    let client_info = audits
            .load_one(client_info::Entity, conn)
            .await
            .map_err(|e| {
                error!(key.count=audits.len(), err=?e, "Could not look up client info for recent audits");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let audit_tuples: Vec<AuditTuple> = itertools::izip!(audits, recent_content, client_info)
        .filter_map(|(audit, con, info)| {
            if let (c, Some(i)) = (con, info) {
                Some((audit, c, i))
            } else {
                None
            }
        })
        .collect();

    Ok(audit_tuples)
}

pub async fn get_recent_audit_successes(
    num_audits: u64,
    conn: &DatabaseConnection,
) -> Result<Vec<AuditTuple>, StatusCode> {
    let recent_audits: Vec<content_audit::Model> = content_audit::Entity::find()
        .order_by_desc(content_audit::Column::CreatedAt)
        .filter(content_audit::Column::Result.eq(AuditResult::Success))
        .limit(num_audits)
        .all(conn)
        .await
        .map_err(|e| {
            error!(key.count=num_audits, err=?e, "Could not look up recent audit successes");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    get_audit_tuples_from_audit_models(recent_audits, conn).await
}

pub async fn get_recent_audit_failures(
    num_audits: u64,
    conn: &DatabaseConnection,
) -> Result<Vec<AuditTuple>, StatusCode> {
    let recent_audits: Vec<content_audit::Model> = content_audit::Entity::find()
        .order_by_desc(content_audit::Column::CreatedAt)
        .filter(content_audit::Column::Result.eq(AuditResult::Failure))
        .limit(num_audits)
        .all(conn)
        .await
        .map_err(|e| {
            error!(key.count=num_audits, err=?e, "Could not look up recent audit failures");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    get_audit_tuples_from_audit_models(recent_audits, conn).await
}

pub async fn get_audit_tuples_from_audit_models(
    audits: Vec<content_audit::Model>,
    conn: &DatabaseConnection,
) -> Result<Vec<AuditTuple>, StatusCode> {
    // Get the corresponding content for each audit.
    let content: Vec<Option<content::Model>> =
        audits.load_one(content::Entity, conn).await.map_err(|e| {
            error!(key.count=audits.len(), err=?e, "Could not look up content for recent audits");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Get the corresponding client_info for each audit.
    let client_info: Vec<Option<client_info::Model>> = audits
        .load_one(client_info::Entity, conn)
        .await
        .map_err(|e| {
            error!(key.count=audits.len(), err=?e, "Could not look up client info for recent audits");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Zip up the audits with their corresponding content and client info.
    // Filter out the (ideally zero) audits that do not have content or client info.
    let audit_tuples: Vec<AuditTuple> = itertools::izip!(audits, content, client_info)
        .filter_map(|(audit, content, info)| content.map(|c| (audit, c, info.unwrap())))
        .collect();

    Ok(audit_tuples)
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
