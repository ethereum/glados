use alloy_primitives::{hex, B256, U256};
use axum::{
    extract::{Extension, Path, Query as HttpQuery},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, TimeZone, Utc};
use enr::NodeId;
use entity::{audit_stats, census, census_node, client_info, content::SubProtocol};
use entity::{
    content,
    content_audit::{self, AuditResult},
    execution_metadata, key_value, node, record,
};
use ethportal_api::types::{
    distance::{Distance, Metric, XorMetric},
    query_trace::QueryTrace,
};
use ethportal_api::utils::bytes::{hex_decode, hex_encode};
use ethportal_api::{jsonrpsee::core::__reexports::serde_json, BeaconContentKey, StateContentKey};
use ethportal_api::{HistoryContentKey, OverlayContentKey};
use glados_core::stats::{
    filter_audits, get_audit_stats, AuditFilters, ContentTypeFilter, Period, StrategyFilter,
    SuccessFilter,
};
use migration::{Alias, Order};
use sea_orm::sea_query::{Expr, Query};
use sea_orm::{sea_query::SimpleExpr, Statement};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult,
    LoaderTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Serialize;
use std::fmt::Formatter;
use std::sync::Arc;
use std::{
    collections::{HashMap, HashSet},
    time::UNIX_EPOCH,
};
use std::{fmt::Display, io};
use tracing::{error, info, warn};

use crate::templates::{
    AuditDashboardTemplate, AuditTableTemplate, CensusExplorerTemplate, ContentAuditDetailTemplate,
    ContentIdDetailTemplate, ContentIdListTemplate, ContentKeyDetailTemplate,
    ContentKeyListTemplate, EnrDetailTemplate, HtmlTemplate, IndexTemplate, NodeDetailTemplate,
    PaginatedCensusListTemplate, SingleCensusViewTemplate,
};
use crate::{state::State, templates::AuditTuple};

//
// Routes
//
pub async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}

// Get the subprotocol from the query parameters, defaulting to History
pub fn get_subprotocol_from_params(params: &HashMap<String, String>) -> SubProtocol {
    match params.get("network") {
        None => SubProtocol::History,
        Some(subprotocol) => match subprotocol.try_into().ok() {
            Some(subprotocol) => subprotocol,
            None => SubProtocol::History,
        },
    }
}

pub async fn network_overview(
    params: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    let subprotocol = get_subprotocol_from_params(&params);

    let client_diversity_data = match get_max_census_id(&state, subprotocol).await {
        None => vec![],
        Some(max_census_id) => generate_client_diversity_data(&state, max_census_id.id)
            .await
            .unwrap(),
    };

    let radius_percentages = generate_radius_graph_data(&state, subprotocol).await;
    // Run queries for content dashboard data concurrently
    let (hour_stats, day_stats, week_stats) = tokio::join!(
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::FourFours,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: subprotocol,
            },),
            Period::Hour,
            &state.database_connection,
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::FourFours,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: subprotocol,
            },),
            Period::Day,
            &state.database_connection,
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::FourFours,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: subprotocol,
            },),
            Period::Week,
            &state.database_connection,
        ),
    );
    // Get results from queries
    let hour_stats = hour_stats.unwrap();
    let day_stats = day_stats.unwrap();
    let week_stats = week_stats.unwrap();

    let template = IndexTemplate {
        client_diversity_data,
        average_radius_chart: radius_percentages,
        stats: [hour_stats, day_stats, week_stats],
    };
    HtmlTemplate(template)
}

pub async fn node_detail(
    Path(node_id_hex): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<NodeDetailTemplate>, StatusCode> {
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

    let latest_enr = enr_list.first().cloned();

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

pub async fn contentaudit_dashboard(
    params: HttpQuery<HashMap<String, String>>,
) -> Result<HtmlTemplate<AuditDashboardTemplate>, StatusCode> {
    let subprotocol = get_subprotocol_from_params(&params);
    let template = AuditDashboardTemplate { subprotocol };
    Ok(HtmlTemplate(template))
}

pub async fn census_explorer() -> Result<HtmlTemplate<CensusExplorerTemplate>, StatusCode> {
    let template = CensusExplorerTemplate {};
    Ok(HtmlTemplate(template))
}

/// Returns the success rate for the last hour as a percentage.
pub async fn hourly_success_rate(
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<f32>, StatusCode> {
    let open_filter = content_audit::Entity::find();
    let stats = get_audit_stats(open_filter, Period::Hour, &state.database_connection)
        .await
        .map_err(|e| {
            error!("Could not look up hourly stats: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(stats.pass_percent))
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

    let (content_id, content_kind) =
        if let Ok(content_key) = HistoryContentKey::try_from(content_key_raw.clone()) {
            let content_id = hex_encode(content_key.content_id());
            let content_kind = content_key.to_string();
            (content_id, content_kind)
        } else if let Ok(content_key) = StateContentKey::try_from(content_key_raw.clone()) {
            let content_id = hex_encode(content_key.content_id());
            let content_kind = content_key.to_string();
            (content_id, content_kind)
        } else if let Ok(content_key) = BeaconContentKey::try_from(content_key_raw.clone()) {
            let content_id = hex_encode(content_key.content_id());
            let content_kind = content_key.to_string();
            (content_id, content_kind)
        } else {
            error!(
                content.key = content_key_hex,
                "Could not create key from bytes"
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };
    let metadata_model = execution_metadata::Entity::find()
        .filter(execution_metadata::Column::Content.eq(content_key_model.id))
        .one(&state.database_connection)
        .await
        .map_err(|e| {
            error!(content.key=content_key_hex, err=?e, "No content metadata found");
            StatusCode::NOT_FOUND
        })?;
    let block_number = metadata_model.map(|m| m.block_number);

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
) -> Result<HtmlTemplate<ContentAuditDetailTemplate>, StatusCode> {
    let audit_id = audit_id.parse::<i32>().unwrap();
    info!("Audit ID: {}", audit_id);
    let mut audit = match content_audit::Entity::find_by_id(audit_id)
        .one(&state.database_connection)
        .await
    {
        Ok(Some(audit)) => audit,
        Ok(None) => return Err(StatusCode::from_u16(404).unwrap()),
        Err(err) => {
            error!(err=?err, "Failed to lookup audit");
            return Err(StatusCode::from_u16(404).unwrap());
        }
    };

    let trace_string = &audit.trace;
    let mut trace: Option<QueryTrace> = match serde_json::from_str(trace_string) {
        Ok(trace) => Some(trace),
        Err(err) => {
            error!(trace=?trace_string, err=?err, "Failed to deserialize query trace.");
            None
        }
    };

    // If we were able to deserialize the trace, we can look up & interpolate the radius for the nodes in the trace.
    if let Some(trace) = &mut trace {
        // Get the timestamp of the query
        let query_timestamp = trace.started_at_ms;
        let timestamp: DateTime<Utc> = {
            let duration = query_timestamp.duration_since(UNIX_EPOCH).unwrap();
            Utc.timestamp_opt(duration.as_secs() as i64, duration.subsec_nanos())
                .single()
                .expect("Failed to convert timestamp to DateTime")
        };

        // Do a query to get, for each node, the radius recorded closest to the time at which the trace took place.
        let node_ids: Vec<Vec<u8>> = trace
            .metadata
            .keys()
            .cloned()
            .map(|x| x.raw().to_vec())
            .collect();
        let node_ids_str = format!(
            "{{{}}}",
            node_ids
                .iter()
                .map(|id| format!("\\\\x{}", hex::encode(id)))
                .collect::<Vec<String>>()
                .join(",")
        );
        let nodes_with_radius: HashMap<NodeId, B256> =
            match NodeWithRadius::find_by_statement(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "
                SELECT DISTINCT ON (n.node_id)
                    n.node_id,
                    cn.data_radius
                FROM
                    node n
                    JOIN record r ON r.node_id = n.id
                    JOIN census_node cn ON cn.record_id = r.id
                WHERE
                    n.node_id = ANY($1::bytea[])
                ORDER BY
                    n.node_id,
                    ABS(EXTRACT(EPOCH FROM (cn.surveyed_at - $2::timestamp)))
                ",
                vec![node_ids_str.into(), timestamp.into()],
            ))
            .all(&state.database_connection)
            .await
            {
                Ok(data) => data
                    .into_iter()
                    // Transform SQL result into a hashmap.
                    .map(|node_result| {
                        let mut node_id = [0u8; 32];
                        node_id.copy_from_slice(&node_result.node_id);
                        let node_id = NodeId::new(&node_id);
                        let mut radius = [0u8; 32];
                        radius.copy_from_slice(&node_result.data_radius);
                        let radius = B256::new(radius);
                        (node_id, radius)
                    })
                    .collect(),
                Err(err) => {
                    error!(err=?err, "Failed to lookup radius for traced nodes");
                    HashMap::new()
                }
            };

        // Add radius info to node metadata.
        trace.metadata.iter_mut().for_each(|(node_id, node_info)| {
            if let Some(radius) = nodes_with_radius.get(node_id) {
                node_info.radius = Some(*radius);
            }
        });
        // Update the trace with radius metadata.
        audit.trace =
            serde_json::to_string(&trace).expect("Failed to serialize updated query trace.");
    }

    let content = audit
        .find_related(content::Entity)
        .one(&state.database_connection)
        .await
        .unwrap()
        .expect("Failed to get audit content key");

    let execution_metadata = content
        .find_related(execution_metadata::Entity)
        .one(&state.database_connection)
        .await
        .unwrap()
        .expect("Failed to get audit metadata");

    let template = ContentAuditDetailTemplate {
        audit,
        content,
        execution_metadata,
    };
    Ok(HtmlTemplate(template))
}

#[derive(FromQueryResult, Debug)]
pub struct NodeWithRadius {
    pub node_id: Vec<u8>,
    pub data_radius: Vec<u8>,
}

/// Takes an AuditFilter object generated from http query params
/// Conditionally creates a query based on the filters
pub async fn contentaudit_filter(
    Extension(state): Extension<Arc<State>>,
    filters: HttpQuery<AuditFilters>,
) -> Result<HtmlTemplate<AuditTableTemplate>, StatusCode> {
    let audits = filter_audits(filters.0);
    let (hour_stats, day_stats, week_stats, filtered_audits) = tokio::join!(
        get_audit_stats(audits.clone(), Period::Hour, &state.database_connection),
        get_audit_stats(audits.clone(), Period::Day, &state.database_connection),
        get_audit_stats(audits.clone(), Period::Week, &state.database_connection),
        audits
            .order_by_desc(content_audit::Column::CreatedAt)
            .limit(30)
            .all(&state.database_connection),
    );

    let filtered_audits = filtered_audits.map_err(|e| {
        error!(err=?e, "Could not look up audits");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let hour_stats = hour_stats.map_err(|e| {
        error!(err=?e, "Could not look up audit hourly stats");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let day_stats = day_stats.map_err(|e| {
        error!(err=?e, "Could not look up audit daily stats");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let week_stats = week_stats.map_err(|e| {
        error!(err=?e, "Could not look up audit weekly stats");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let filtered_audits: Vec<AuditTuple> =
        get_audit_tuples_from_audit_models(filtered_audits, &state.database_connection).await?;

    let template = AuditTableTemplate {
        stats: [hour_stats, day_stats, week_stats],
        audits: filtered_audits,
    };

    Ok(HtmlTemplate(template))
}

#[derive(FromQueryResult, Serialize, Debug)]
pub struct DeadZoneData {
    pub data_radius: Vec<u8>,
    pub raw: String,
    pub node_id: Vec<u8>,
}

pub async fn is_content_in_deadzone(
    Path(content_key): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let builder = state.database_connection.get_database_backend();
    let mut select_dead_zone_data = Query::select();
    select_dead_zone_data
        .expr(Expr::col((census_node::Entity, census_node::Column::DataRadius)))
        .expr(Expr::col((node::Entity, node::Column::NodeId)))
        .expr(Expr::col((record::Entity, record::Column::Raw)))
        .from(census_node::Entity)
        .from(node::Entity)
        .from(record::Entity)
        .from_subquery(
            Query::select()
                .from(census::Entity)
                .expr_as(Expr::col(census::Column::StartedAt), Alias::new("started_at"))
                .expr_as(Expr::col(census::Column::Duration), Alias::new("duration"))
                .order_by(census::Column::StartedAt, Order::Desc)
                .limit(1)
                .take(),
            Alias::new("latest_census"),
        )
        .and_where(
            Expr::col((census_node::Entity, census_node::Column::SurveyedAt))
                .gte(Expr::col((Alias::new("latest_census"), Alias::new("started_at")))),
        )
        .and_where(
            Expr::col((census_node::Entity, census_node::Column::SurveyedAt))
                .lt(Expr::cust(if builder == DbBackend::Sqlite {
                    "STRFTIME('%Y-%m-%dT%H:%M:%S.%f', DATETIME(latest_census.started_at, '+' || latest_census.duration || ' seconds'))"
                } else {
                    "latest_census.started_at + latest_census.duration * interval '1 second'"
                })),
        )
        .and_where(
            Expr::col((census_node::Entity, census_node::Column::RecordId))
                .eq(Expr::col((record::Entity, record::Column::Id))),
        )
        .and_where(
            Expr::col((record::Entity, record::Column::NodeId))
                .eq(Expr::col((node::Entity, node::Column::Id))),
        );

    let dead_zone_data_vec = DeadZoneData::find_by_statement(builder.build(&select_dead_zone_data))
        .all(&state.database_connection)
        .await
        .unwrap();

    let content_key: ethportal_api::HistoryContentKey =
        serde_json::from_value(serde_json::json!(content_key)).unwrap();
    let content_id = content_key.content_id();

    let mut enrs: Vec<String> = vec![];
    for dead_zone_data in dead_zone_data_vec {
        let radius = Distance::from(U256::from_be_slice(&dead_zone_data.data_radius));
        let node_id = Distance::from(U256::from_be_slice(&dead_zone_data.node_id));
        if XorMetric::distance(&content_id, &node_id.big_endian()) <= radius {
            enrs.push(dead_zone_data.raw);
        }
    }

    Ok(Json(enrs))
}

pub async fn get_audit_stats_handler(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<audit_stats::Model>>, StatusCode> {
    let weeks_ago: i32 = match http_args.get("weeks-ago") {
        None => 0,
        Some(days_ago) => days_ago.parse::<i32>().unwrap_or(0),
    };
    let stats = audit_stats::get_recent_stats(&state.database_connection, weeks_ago)
        .await
        .map_err(|e| {
            error!(err=?e, "Could not look up audit stat history");
            StatusCode::INTERNAL_SERVER_ERROR
        })
        .unwrap();

    Ok(Json(stats))
}

pub async fn census_explorer_list(
    params: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<PaginatedCensusListTemplate>, StatusCode> {
    let subprotocol = get_subprotocol_from_params(&params);
    let max_census_id = match get_max_census_id(&state, subprotocol).await {
        None => return Err(StatusCode::from_u16(404).unwrap()),
        Some(max_census_id) => max_census_id,
    };

    let mut list_census_page_id: i32 = match params.get("page") {
        None => return Err(StatusCode::from_u16(404).unwrap()),
        Some(list_census_page_id) => match list_census_page_id.parse::<i32>() {
            Ok(list_census_page_id) => list_census_page_id,
            Err(_) => return Err(StatusCode::from_u16(404).unwrap()),
        },
    };

    if list_census_page_id > max_census_id.id / 50 + 1 {
        list_census_page_id = max_census_id.id / 50 + 1;
    }
    if list_census_page_id < 1 {
        list_census_page_id = 1;
    }

    let builder = state.database_connection.get_database_backend();
    let mut paginated_census_list = Query::select();
    paginated_census_list
        .expr(Expr::col((
            census_node::Entity,
            census_node::Column::CensusId,
        )))
        .expr_as(
            Expr::count(Expr::col(census_node::Column::CensusId)),
            Alias::new("node_count"),
        )
        .expr_as(
            Expr::col((census::Entity, census::Column::StartedAt)),
            Alias::new("created_at"),
        )
        .from(census::Entity)
        .from(census_node::Entity)
        .and_where(
            Expr::col((census::Entity, census_node::Column::Id)).eq(Expr::col((
                census_node::Entity,
                census_node::Column::CensusId,
            ))),
        )
        .add_group_by([
            SimpleExpr::from(Expr::col((
                census_node::Entity,
                census_node::Column::CensusId,
            ))),
            SimpleExpr::from(Expr::col((census::Entity, census::Column::StartedAt))),
        ])
        .order_by(census::Column::StartedAt, Order::Desc)
        .limit(50)
        .offset(((list_census_page_id - 1) * 50) as u64);

    let paginated_census_list =
        PaginatedCensusListResult::find_by_statement(builder.build(&paginated_census_list))
            .all(&state.database_connection)
            .await
            .unwrap();

    let template = PaginatedCensusListTemplate {
        census_data: paginated_census_list,
        list_census_page_id,
        max_census_id: max_census_id.id,
    };

    Ok(HtmlTemplate(template))
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct NodeStatus {
    enr_id: i32,
    census_time: DateTime<Utc>,
    census_id: i32,
    node_id: Vec<u8>,
    present: bool,
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct RecordInfo {
    id: i32,
    raw: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CensusTimeSeriesData {
    node_ids_with_nicknames: Vec<(String, Option<String>)>,
    censuses: Vec<CensusStatuses>,
    enrs: HashMap<i32, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CensusStatuses {
    census_id: i32,
    time: DateTime<Utc>,
    enr_statuses: Vec<Option<i32>>,
}

pub async fn census_timeseries(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<CensusTimeSeriesData>, StatusCode> {
    let days_ago: i32 = match http_args.get("days-ago") {
        None => 0,
        Some(days_ago) => days_ago.parse::<i32>().unwrap_or(0),
    };

    let subprotocol = get_subprotocol_from_params(&http_args);

    // Load all censuses in the given 24 hour window with each node's presence status & ENR
    let node_statuses: Vec<NodeStatus> =
        NodeStatus::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT 
                c.started_at AS census_time, 
                c.id AS census_id,
                n.node_id,
                r.id as enr_id,
                CASE 
                    WHEN r.id IS NOT NULL THEN true 
                    ELSE false 
                END AS present
            FROM 
                (
                    SELECT * FROM census
                    WHERE sub_network = $2
                    AND started_at >= NOW() - INTERVAL '1 day' * ($1 + 1)
                    AND started_at < NOW() - INTERVAL '1 day' * $1
                ) AS c
            LEFT JOIN 
                census_node AS cn ON c.id = cn.census_id
            LEFT JOIN
                record AS r ON r.id = cn.record_id
            LEFT JOIN 
                node AS n ON n.id = r.node_id
            ORDER BY 
                c.started_at, n.node_id;",
            vec![days_ago.into(), subprotocol.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup census node timeseries data");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Load all ENRs found in the census
    let record_ids = node_statuses
        .iter()
        .map(|n| n.enr_id)
        .collect::<HashSet<i32>>() // Collect into a HashSet to remove duplicates
        .into_iter()
        .collect::<Vec<i32>>();
    let record_ids_str = format!(
        "{{{}}}",
        record_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let records: Vec<RecordInfo> = RecordInfo::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, raw
            FROM record
            WHERE id = ANY($1::int[]);",
        vec![record_ids_str.into()],
    ))
    .all(&state.database_connection)
    .await
    .map_err(|e| {
        error!(err=?e, "Failed to lookup census node timeseries data");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let enr_id_map: HashMap<i32, String> = records.into_iter().map(|r| (r.id, r.raw)).collect();

    let (node_ids, censuses) = decouple_nodes_and_censuses(node_statuses);
    let node_ids_with_nicknames: Vec<(String, Option<String>)> = node_ids
        .iter()
        .map(|id| {
            if id.len() != 66 {
                return (id.clone(), None);
            }
            // Node nickname mappings including full node IDs and shortened node IDs
            let short_id = format!("{}..{}", &id[..6], &id[id.len() - 4..]);
            let nickname: Option<String> =
                if let Some(nickname) = node::NODE_NICKNAME_MAP.get(&short_id) {
                    Some(nickname.clone())
                } else {
                    node::NODE_NICKNAME_MAP.get(id).cloned()
                };

            (id.clone(), nickname)
        })
        .collect();

    Ok(Json(CensusTimeSeriesData {
        node_ids_with_nicknames,
        censuses,
        enrs: enr_id_map,
    }))
}

/// Decouples census data from node data, now including ENR strings.
type NodeIdString = String;
fn decouple_nodes_and_censuses(
    node_statuses: Vec<NodeStatus>,
) -> (Vec<NodeIdString>, Vec<CensusStatuses>) {
    let mut node_set: HashSet<String> = HashSet::new();

    type NodeEnrIdStatuses = HashMap<String, Option<i32>>;
    let mut census_map: HashMap<i32, (DateTime<Utc>, NodeEnrIdStatuses)> = HashMap::new();

    for status in node_statuses {
        let hex_id = hex_encode(status.node_id);
        node_set.insert(hex_id.clone());
        let enr_opt = if status.present {
            Some(status.enr_id)
        } else {
            None
        };
        let entry = census_map
            .entry(status.census_id)
            .or_insert((status.census_time, HashMap::new()));
        entry.1.insert(hex_id, enr_opt);
    }

    let node_ids: Vec<String> = node_set.into_iter().collect();
    let mut censuses: Vec<CensusStatuses> = vec![];

    for (census_id, (time, enr_statuses_map)) in census_map {
        let enr_statuses = node_ids
            .iter()
            .map(|node_id| enr_statuses_map.get(node_id).cloned().unwrap_or(None))
            .collect();

        censuses.push(CensusStatuses {
            census_id,
            time,
            enr_statuses,
        });
    }

    censuses.sort_by_key(|c| c.time);

    (node_ids, censuses)
}

pub async fn single_census_view(
    params: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<SingleCensusViewTemplate>, StatusCode> {
    let subprotocol = get_subprotocol_from_params(&params);
    let max_census_id = match get_max_census_id(&state, subprotocol).await {
        None => return Err(StatusCode::from_u16(404).unwrap()),
        Some(max_census_id) => max_census_id,
    };

    let census_id: i32 = match params.get("census-id") {
        None => return Err(StatusCode::from_u16(404).unwrap()),
        Some(census_id) => match census_id.parse::<i32>() {
            Ok(census_id) => census_id,
            Err(_) => max_census_id.id,
        },
    };

    let client_diversity_data = match generate_client_diversity_data(&state, census_id).await {
        None => return Err(StatusCode::from_u16(404).unwrap()),
        Some(client_diversity_data) => client_diversity_data,
    };

    let enr_list =
        match generate_enr_list_from_census_id(&state, Some(census_id), max_census_id).await {
            None => return Err(StatusCode::from_u16(404).unwrap()),
            Some(enr_list) => enr_list,
        };

    let template = SingleCensusViewTemplate {
        client_diversity_data,
        node_count: enr_list.len() as i32,
        enr_list,
        census_id,
        max_census_id: max_census_id.id,
        created_at: get_created_data_from_census_id(&state, census_id)
            .await
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
    };

    Ok(HtmlTemplate(template))
}

async fn generate_enr_list_from_census_id(
    state: &Arc<State>,
    census_id: Option<i32>,
    max_census_id: MaxCensusId,
) -> Option<Vec<RawEnr>> {
    let census_selection_query = match census_id {
        Some(census_id) => {
            if census_id >= 1 && census_id <= max_census_id.id {
                Query::select()
                    .from(census::Entity)
                    .expr_as(Expr::col(census::Column::Id), Alias::new("id"))
                    .and_where(SimpleExpr::from(Expr::col(census::Column::Id)).eq(census_id))
                    .limit(1)
                    .take()
            } else {
                return None;
            }
        }
        None => Query::select()
            .from(census::Entity)
            .expr_as(Expr::col(census::Column::Id), Alias::new("id"))
            .order_by(census::Column::StartedAt, Order::Desc)
            .limit(1)
            .take(),
    };

    let builder = state.database_connection.get_database_backend();
    let mut enrs_from_census = Query::select();
    enrs_from_census
        .expr(Expr::col((record::Entity, record::Column::Raw)))
        .from(census_node::Entity)
        .from(record::Entity)
        .from_subquery(census_selection_query, Alias::new("selected_census_id"))
        .and_where(
            Expr::col((census_node::Entity, census_node::Column::CensusId)).eq(Expr::col((
                Alias::new("selected_census_id"),
                Alias::new("id"),
            ))),
        )
        .and_where(
            Expr::col((census_node::Entity, census_node::Column::RecordId))
                .eq(Expr::col((record::Entity, record::Column::Id))),
        );

    Some(
        RawEnr::find_by_statement(builder.build(&enrs_from_census))
            .all(&state.database_connection)
            .await
            .unwrap(),
    )
}

async fn get_created_data_from_census_id(state: &Arc<State>, census_id: i32) -> DateTime<Utc> {
    let builder = state.database_connection.get_database_backend();
    // we need to bounds check the requested census_id and return None if it doesn't exist
    let created_data = Query::select()
        .from(census::Entity)
        .expr_as(
            Expr::col(census::Column::StartedAt),
            Alias::new("created_at"),
        )
        .and_where(Expr::col(census::Column::Id).eq(census_id))
        .take();
    let created_data = CensusCreatedAt::find_by_statement(builder.build(&created_data))
        .one(&state.database_connection)
        .await
        .unwrap();
    created_data.unwrap().created_at
}

#[derive(FromQueryResult, Debug, Clone, Copy)]
pub struct MaxCensusId {
    pub id: i32,
}

#[derive(FromQueryResult, Serialize, Debug, Clone)]
pub struct PaginatedCensusListResult {
    pub census_id: i32,
    pub node_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(FromQueryResult, Debug, Clone)]
pub struct CensusCreatedAt {
    pub created_at: DateTime<Utc>,
}

#[derive(FromQueryResult, Debug)]
pub struct RadiusChartData {
    pub data_radius: Vec<u8>,
    pub node_id: Vec<u8>,
    pub raw: String,
}

#[derive(Serialize, Debug)]
pub struct CalculatedRadiusChartData {
    pub data_radius: f64,
    /// Top byte of the advertised radius
    pub radius_top: u8,
    /// Percentage coverage, not including the top byte
    pub radius_lower_fraction: f64,
    pub node_id: u64,
    pub node_id_string: String,
    pub raw_enr: String,
}

#[derive(FromQueryResult, Serialize)]
pub struct ClientDiversityResult {
    pub client_name: String,
    pub client_count: i32,
}

#[derive(FromQueryResult, Serialize)]
pub struct RawEnr {
    pub raw: String,
}

impl Display for ClientDiversityResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Client Name {} Client Count {}",
            self.client_name, self.client_count
        )
    }
}

async fn generate_radius_graph_data(
    state: &Arc<State>,
    subprotocol: SubProtocol,
) -> Vec<CalculatedRadiusChartData> {
    let radius_chart_data = RadiusChartData::find_by_statement(Statement::from_sql_and_values( DbBackend::Postgres,
    "
        WITH latest_census AS (
            SELECT started_at, duration
            FROM census
            WHERE sub_network = $1
            ORDER BY started_at DESC
            LIMIT 1
        )
        SELECT 
            census_node.data_radius,
            record.raw,
            node.node_id
        FROM 
            census_node,
            node,
            record,
            latest_census
        WHERE
            census_node.sub_network = $1
            AND census_node.surveyed_at >= latest_census.started_at
            AND census_node.surveyed_at < latest_census.started_at + latest_census.duration * interval '1 second'
            AND census_node.record_id = record.id
            AND record.node_id = node.id
            ",
     vec![subprotocol.into()])).all(&state.database_connection).await.unwrap();

    let mut radius_percentages: Vec<CalculatedRadiusChartData> = vec![];
    for i in radius_chart_data {
        let radius_fraction = xor_distance_to_fraction([
            i.data_radius[0],
            i.data_radius[1],
            i.data_radius[2],
            i.data_radius[3],
        ]);
        let node_id_high_bytes: [u8; 8] = [
            i.node_id[0],
            i.node_id[1],
            i.node_id[2],
            i.node_id[3],
            i.node_id[4],
            i.node_id[5],
            i.node_id[6],
            i.node_id[7],
        ];

        let formatted_percentage = format!("{:.2}", radius_fraction * 100.0);

        let mut node_id_bytes: [u8; 32] = [0; 32];
        if i.node_id.len() == 32 {
            node_id_bytes.copy_from_slice(&i.node_id);
        }

        let radius_lower_fraction = xor_distance_to_fraction([
            i.data_radius[1],
            i.data_radius[2],
            i.data_radius[3],
            i.data_radius[4],
        ]);

        let node_id_string = hex_encode(node_id_bytes);
        radius_percentages.push(CalculatedRadiusChartData {
            data_radius: formatted_percentage.parse().unwrap(),
            radius_top: i.data_radius[0],
            radius_lower_fraction,
            node_id: u64::from_be_bytes(node_id_high_bytes),
            node_id_string,
            raw_enr: i.raw,
        });
    }

    radius_percentages
}

fn xor_distance_to_fraction(radius_high_bytes: [u8; 4]) -> f64 {
    let radius_int = u32::from_be_bytes(radius_high_bytes);
    radius_int as f64 / u32::MAX as f64
}

async fn get_max_census_id(state: &Arc<State>, subprotocol: SubProtocol) -> Option<MaxCensusId> {
    match MaxCensusId::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT MAX(id) as id FROM census
             WHERE sub_network = $1",
        vec![subprotocol.into()],
    ))
    .one(&state.database_connection)
    .await
    {
        Ok(val) => val,
        Err(err) => {
            warn!("Census data unavailable: {err}");
            None
        }
    }
}

async fn generate_client_diversity_data(
    state: &Arc<State>,
    census_id: i32,
) -> Option<Vec<ClientDiversityResult>> {
    Some(
        ClientDiversityResult::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres,
        "
            WITH left_table AS (
                SELECT census_node.record_id
                FROM census_node
                WHERE census_node.census_id = $1
            ),
            right_table AS (
                SELECT record_id, value
                FROM key_value
                WHERE convert_from(key, 'UTF8') = 'c'
            )
            SELECT 
                CAST(COUNT(*) AS INTEGER) AS client_count,
                CAST(COALESCE(substr(substr(right_table.value, 1, 2), length(substr(right_table.value, 1, 2)), 1), 'unknown') AS TEXT) AS client_name
            FROM left_table
            LEFT JOIN right_table ON left_table.record_id = right_table.record_id
            GROUP BY substr(substr(right_table.value, 1, 2), length(substr(right_table.value, 1, 2)), 1)
            ", vec![census_id.into()])
        ).all(&state.database_connection).await.unwrap(),
    )
}
