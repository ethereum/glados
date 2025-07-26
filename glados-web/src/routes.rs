use std::collections::{HashMap, HashSet};
use std::io;
use std::str::FromStr;
use std::sync::Arc;

use alloy::rlp::Decodable;
use alloy_primitives::{hex, B256, U256};
use axum::{
    extract::{Extension, Path, Query as HttpQuery},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, TimeZone, Utc};
use clap::ValueEnum;
use enr::NodeId;
use entity::{client, client_info, SelectionStrategy};
use ethportal_api::{
    jsonrpsee::core::__reexports::serde_json,
    types::{
        distance::{Distance, Metric, XorMetric},
        protocol_versions::ProtocolVersionList,
        query_trace::QueryTrace,
    },
    utils::bytes::{hex_decode, hex_encode},
    HistoryContentKey, OverlayContentKey,
};
use sea_orm::prelude::DateTimeUtc;
use sea_orm::sea_query::Alias;
use sea_orm::{
    sea_query::{Expr, Query, SimpleExpr},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult,
    Iterable, LoaderTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect, Statement,
};
use sea_orm::{Order, QueryTrait};
use serde::Serialize;
use tracing::{debug, error, info, warn};

use crate::templates::{
    AuditDashboardTemplate, AuditTableTemplate, CensusExplorerTemplate, ClientsTemplate,
    ContentAuditDetailTemplate, ContentIdDetailTemplate, ContentIdListTemplate,
    ContentKeyDetailTemplate, ContentKeyListTemplate, DiagnosticsTemplate, EnrDetailTemplate,
    HtmlTemplate, IndexTemplate, NodeDetailTemplate, PaginatedCensusListTemplate,
    SingleCensusViewTemplate,
};
use crate::{state::State, templates::AuditTuple};
use entity::{
    audit, audit_stats, census, census_node,
    client_info::{Client, OperatingSystem, Version},
    content, node, node_enr, ContentType, SubProtocol, TransferFailureType,
};
use glados_core::stats::{
    filter_audits, get_audit_stats, AuditFilters, ContentTypeFilter, Period, StrategyFilter,
    SuccessFilter,
};

//
// Routes
//
pub async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}

// Get the subprotocol from the query parameters, defaulting to History
pub fn get_subprotocol_from_params(params: &HashMap<String, String>) -> SubProtocol {
    params
        .get("network")
        .and_then(|network| SubProtocol::from_str(network, /* ignore_case= */ true).ok())
        .unwrap_or(SubProtocol::History)
}

pub async fn network_overview(
    params: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    let sub_protocol = get_subprotocol_from_params(&params);

    let client_diversity_data = match get_max_census_id(&state, sub_protocol).await {
        None => vec![],
        Some(max_census_id) => generate_client_diversity_data(&state, max_census_id)
            .await
            .unwrap_or_default(),
    };

    let radius_percentages = generate_radius_graph_data(&state, sub_protocol).await;

    let strategy: StrategyFilter = match sub_protocol {
        SubProtocol::History => StrategyFilter::Sync,
    };

    // Run queries for content dashboard data concurrently
    let (hour_stats, day_stats, week_stats) = tokio::join!(
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                sub_protocol,
            },),
            Period::Hour,
            &state.database_connection,
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                sub_protocol,
            },),
            Period::Day,
            &state.database_connection,
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                sub_protocol,
            },),
            Period::Week,
            &state.database_connection,
        ),
    );

    let template = IndexTemplate {
        subprotocol: sub_protocol,
        strategy,
        client_diversity_data,
        average_radius_chart: radius_percentages,
        stats: [hour_stats.unwrap(), day_stats.unwrap(), week_stats.unwrap()],
        content_types: ContentType::iter().collect(),
    };
    HtmlTemplate(template)
}

pub async fn clients_overview(params: HttpQuery<HashMap<String, String>>) -> impl IntoResponse {
    let subprotocol = get_subprotocol_from_params(&params);

    let template = ClientsTemplate {
        subprotocol,
        clients: Client::iter().collect(),
        operating_systems: OperatingSystem::iter().collect(),
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
    let enr_list = node_enr::Entity::find()
        .filter(node_enr::Column::NodeId.eq(node_model.id))
        .order_by_desc(node_enr::Column::SequenceNumber)
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

    let template = NodeDetailTemplate {
        node: node_model,
        latest_enr,
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
    let enr = node_enr::Entity::find()
        .filter(node_enr::Column::NodeId.eq(node_model.id.to_owned()))
        .filter(node_enr::Column::SequenceNumber.eq(enr_seq))
        .one(&state.database_connection)
        .await
        .map_err(|e| {
            error!(enr.node_id=node_id_hex, enr.seq=enr_seq, err=?e, "No record found for node_id and sequence_number");
            StatusCode::NOT_FOUND
        })
        .unwrap()
        .unwrap();

    let template = EnrDetailTemplate {
        node: node_model,
        enr,
    };
    Ok(HtmlTemplate(template))
}

pub async fn get_audit_tuples_from_audit_models(
    audits: Vec<audit::Model>,
    conn: &DatabaseConnection,
) -> Result<Vec<AuditTuple>, StatusCode> {
    // Get the corresponding content for each audit.
    let content: Vec<Option<content::Model>> =
        audits.load_one(content::Entity, conn).await.map_err(|e| {
            error!(key.count=audits.len(), err=?e, "Could not look up content for recent audits");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Get the corresponding client_info for each audit.
    let client: Vec<Option<client::Model>> = audits
        .load_one(client::Entity, conn)
        .await
        .map_err(|e| {
            error!(key.count=audits.len(), err=?e, "Could not look up client info for recent audits");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Zip up the audits with their corresponding content and client info.
    // Filter out the (ideally zero) audits that do not have content or client info.
    let audit_tuples: Vec<AuditTuple> = itertools::izip!(audits, content, client)
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
    let open_filter = audit::Entity::find();
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

    let content_model = content::Entity::find()
        .filter(content::Column::ContentKey.eq(content_key_raw))
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

    let audit_list = content_model
        .find_related(audit::Entity)
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(content.key=content_key_hex, err=?e, "Could not look up audits for key");
            StatusCode::NOT_FOUND
        })?;

    let Ok(content_key) = HistoryContentKey::try_from_hex(&content_key_hex) else {
        error!(
            content.key = content_key_hex,
            "Could not create key from bytes"
        );
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let template = ContentKeyDetailTemplate {
        content: content_model,
        content_kind: content_key.to_string(),
        audit_list,
    };
    Ok(HtmlTemplate(template))
}

pub async fn contentaudit_detail(
    Path(audit_id): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<ContentAuditDetailTemplate>, StatusCode> {
    let audit_id = audit_id.parse::<i32>().unwrap();
    info!("Audit ID: {}", audit_id);
    let mut audit = match audit::Entity::find_by_id(audit_id)
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

    let mut trace: Option<QueryTrace> = match &audit.trace {
        Some(trace) => match serde_json::from_str::<QueryTrace>(trace) {
            Ok(trace) => Some(trace),
            Err(err) => {
                error!(trace=?audit.trace, err=?err, "Failed to deserialize query trace.");
                None
            }
        },
        None => None,
    };

    // If we were able to deserialize the trace, we can look up & interpolate the radius for the nodes in the trace.
    if let Some(trace) = &mut trace {
        // Get the timestamp of the query
        let timestamp: DateTime<Utc> = Utc
            .timestamp_millis_opt(trace.started_at_ms as i64)
            .single()
            .expect("Failed to convert timestamp to DateTime");

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
                SELECT DISTINCT ON (node.node_id)
                    node.node_id,
                    closest_census_node.data_radius
                FROM
                    node
                    JOIN node_enr ON node_enr.node_id = node.id
                    CROSS JOIN LATERAL (
                        SELECT census_node.data_radius, census_node.surveyed_at
                        FROM census_node
                        WHERE census_node.node_enr_id = node_enr.id AND census_node.surveyed_at <= $2::timestamp + INTERVAL '15 minutes'
                        ORDER BY census_node.surveyed_at DESC
                        LIMIT 1
                    ) closest_census_node
                WHERE
                    node.node_id = ANY($1::bytea[])
                ORDER BY
                    node.node_id,
                    closest_census_node.surveyed_at DESC
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
            Some(serde_json::to_string(&trace).expect("Failed to serialize updated query trace."));
    }

    let content = audit
        .find_related(content::Entity)
        .one(&state.database_connection)
        .await
        .unwrap()
        .expect("Failed to get audit content key");

    let template = ContentAuditDetailTemplate { audit, content };
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
            .order_by_desc(audit::Column::CreatedAt)
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
    let (sub_protocol, content_id) = if let Ok(content_key) =
        serde_json::from_value::<HistoryContentKey>(serde_json::json!(content_key))
    {
        (SubProtocol::History, content_key.content_id())
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let dead_zone_data_vec = DeadZoneData::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "
            SELECT
                census_node.data_radius,
                node_enr.raw,
                node.node_id
            FROM census_node
            JOIN node_enr ON census_node.node_enr_id = node_enr.id
            JOIN node ON node_enr.node_id = node.id
            WHERE census_node.census_id = (
                SELECT MAX(id)
                FROM census
                WHERE sub_protocol = $1
            )
        ",
        vec![sub_protocol.into()],
    ))
    .all(&state.database_connection)
    .await
    .map_err(|e| {
        error!(err=?e, "Could not look up nodes with radius");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

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

pub async fn get_history_audit_stats_handler(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<audit_stats::HistoryStats>>, StatusCode> {
    let weeks_ago: i32 = match http_args.get("weeks-ago") {
        None => 0,
        Some(days_ago) => days_ago.parse::<i32>().unwrap_or(0),
    };
    let stats = audit_stats::get_weekly_history_stats(&state.database_connection, weeks_ago)
        .await
        .map_err(|e| {
            error!(err=?e, "Could not look up audit stat history");
            StatusCode::INTERNAL_SERVER_ERROR
        })
        .unwrap();

    Ok(Json(stats))
}

pub async fn get_failed_keys_handler(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let sub_protocol = get_subprotocol_from_params(&http_args);

    let strategy: &str = match http_args.get("strategy") {
        // Set a default for each subprotocol
        None => match sub_protocol {
            SubProtocol::History => "Sync",
        },
        Some(strategy) => &strategy.to_string(),
    };
    let strategy = SelectionStrategy::try_from_str(sub_protocol, strategy).map_err(|err| {
        error!(?sub_protocol, %strategy, %err, "Unkown strategy");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let page: u32 = match http_args.get("page") {
        None => 1,
        Some(page) => page.parse::<u32>().unwrap_or(1),
    };

    let failed_keys = audit::get_failed_keys(strategy, page, &state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Could not fetch failed keys");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .into_iter()
        .map(|failed_key| hex_encode(failed_key.content_key))
        .collect::<Vec<_>>();
    Ok(Json(failed_keys))
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

    if list_census_page_id > max_census_id / 50 + 1 {
        list_census_page_id = max_census_id / 50 + 1;
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
            Expr::col((census::Entity, census::Column::Id)).eq(Expr::col((
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
        max_census_id,
    };

    Ok(HtmlTemplate(template))
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct NodeStatus {
    enr_id: Option<i32>,
    census_time: DateTime<Utc>,
    census_id: i32,
    node_id: Option<Vec<u8>>,
    present: bool,
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct NodeEnrInfo {
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
                census.started_at AS census_time,
                census.id AS census_id,
                node.node_id,
                node_enr.id as enr_id,
                CASE
                    WHEN node_enr.id IS NOT NULL THEN true
                    ELSE false
                END AS present
            FROM
                (
                    SELECT * FROM census
                    WHERE sub_protocol = $2
                    AND started_at >= NOW() - INTERVAL '1 day' * ($1 + 1)
                    AND started_at < NOW() - INTERVAL '1 day' * $1
                ) AS census
            LEFT JOIN
                census_node ON census.id = census_node.census_id
            LEFT JOIN
                node_enr ON node_enr.id = census_node.node_enr_id
            LEFT JOIN
                node ON node.id = node_enr.node_id
            ORDER BY
                census.started_at, node.node_id;",
            vec![days_ago.into(), subprotocol.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup census node timeseries data");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Load all ENRs found in the census
    let node_enr_ids = node_statuses
        .iter()
        .filter_map(|n| n.enr_id)
        .collect::<HashSet<i32>>() // Collect into a HashSet to remove duplicates
        .into_iter()
        .collect::<Vec<i32>>();
    let node_enr_ids_str = format!(
        "{{{}}}",
        node_enr_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let node_enrs: Vec<NodeEnrInfo> =
        NodeEnrInfo::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
        SELECT id, raw
        FROM node_enr
        WHERE id = ANY($1::int[]);",
            vec![node_enr_ids_str.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup census node timeseries data");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let (node_ids, censuses) = decouple_nodes_and_censuses(node_statuses);
    let found_node_ids_with_nicknames: Vec<(String, Option<String>)> = node_ids
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

    let missing_bootnodes_with_nicknames: Vec<(String, Option<String>)> = node::BOOTNODE_NICKNAMES
        .iter()
        .filter(|(id, _)| !node_ids.contains(id))
        .map(|(id, nickname)| (id.clone(), Some(nickname.clone())))
        .collect();

    let missing_bootnodes_enrs = (-1..(-(missing_bootnodes_with_nicknames.len() as i32)))
        .map(|index| (index, "not found in period".to_string()));

    let node_ids_with_nicknames = [
        found_node_ids_with_nicknames,
        missing_bootnodes_with_nicknames,
    ]
    .concat();

    let enrs: HashMap<i32, String> = node_enrs
        .into_iter()
        .map(|r| (r.id, r.raw))
        .chain(missing_bootnodes_enrs)
        .collect();

    Ok(Json(CensusTimeSeriesData {
        node_ids_with_nicknames,
        censuses,
        enrs,
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
        let entry = census_map
            .entry(status.census_id)
            .or_insert((status.census_time, HashMap::new()));

        if let (Some(node_id), Some(enr_id)) = (status.node_id, status.enr_id) {
            let hex_id = hex_encode(node_id);
            node_set.insert(hex_id.clone());

            let enr_opt = if status.present { Some(enr_id) } else { None };

            entry.1.insert(hex_id, enr_opt);
        }
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

#[derive(Debug, Clone, Serialize, FromQueryResult)]
#[serde(rename_all = "camelCase")]
pub struct CensusHistoryData {
    census_id: i32,
    start: DateTime<Utc>,
    node_count: i64,
}
pub async fn weekly_census_history(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<CensusHistoryData>>, StatusCode> {
    let weeks_ago: i32 = match http_args.get("weeks-ago") {
        None => 0,
        Some(weeks_ago) => weeks_ago.parse::<i32>().unwrap_or(0),
    };

    let subprotocol = get_subprotocol_from_params(&http_args);

    let census_history: Vec<CensusHistoryData> =
        CensusHistoryData::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT
                census.id AS census_id,
                ANY_VALUE(DATE_TRUNC('second', census.started_at)) AS start,
                COUNT(1) AS node_count
            FROM census
            LEFT JOIN census_node ON census.id = census_node.census_id
            WHERE
                census.sub_network = $2 AND
                census.started_at >= NOW() - INTERVAL '1 week' * ($1 + 1) AND
                census.started_at < NOW() - INTERVAL '1 week' * $1
            GROUP BY
                census.id
            ORDER BY census.started_at
        ",
            vec![weeks_ago.into(), subprotocol.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup census node timeseries data");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(census_history))
}

#[derive(FromQueryResult)]
pub struct WeeklyCensusClientsData {
    census_id: i32,
    start: DateTime<Utc>,
    client: Client,
    node_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyCensusClientsDataCompact {
    census_id: i32,
    start: DateTime<Utc>,
    client_slug: String,
    node_count: i64,
}

impl From<WeeklyCensusClientsData> for WeeklyCensusClientsDataCompact {
    fn from(value: WeeklyCensusClientsData) -> Self {
        WeeklyCensusClientsDataCompact {
            census_id: value.census_id,
            start: value.start,
            client_slug: value.client.to_string(),
            node_count: value.node_count,
        }
    }
}

pub async fn weekly_census_clients(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<WeeklyCensusClientsDataCompact>>, StatusCode> {
    let weeks_ago: i32 = match http_args.get("weeks-ago") {
        None => 0,
        Some(weeks_ago) => weeks_ago.parse::<i32>().unwrap_or(0),
    };

    let subprotocol = get_subprotocol_from_params(&http_args);

    let census_history: Vec<WeeklyCensusClientsData> =
        WeeklyCensusClientsData::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT
                census.id AS census_id,
                ANY_VALUE(DATE_TRUNC('second', census.started_at)) AS start,
                census_node.client_name AS client,
                COUNT(1) AS node_count
            FROM census
            LEFT JOIN census_node ON census.id = census_node.census_id
            WHERE
                census.sub_network = $2 AND
                census.started_at >= NOW() - INTERVAL '1 week' * ($1 + 1) AND
                census.started_at < NOW() - INTERVAL '1 week' * $1
            GROUP BY
              census.id,
              census_node.client_name
            ORDER BY census.started_at
        ",
            vec![weeks_ago.into(), subprotocol.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup census node timeseries by clients data");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let census_history_compact: Vec<WeeklyCensusClientsDataCompact> =
        census_history.into_iter().map(|c| c.into()).collect();

    Ok(Json(census_history_compact))
}

#[derive(FromQueryResult, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyCensusClientVersionsData {
    census_id: i32,
    start: DateTime<Utc>,
    version: Version,
    node_count: i64,
}

pub async fn weekly_census_client_versions(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<WeeklyCensusClientVersionsData>>, StatusCode> {
    let weeks_ago: i32 = match http_args.get("weeks-ago") {
        None => 0,
        Some(weeks_ago) => weeks_ago.parse::<i32>().unwrap_or(0),
    };

    let subprotocol = get_subprotocol_from_params(&http_args);

    let Some(client_slug) = http_args.get("client") else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let client: Client = client_slug.to_string().into();

    let census_history: Vec<WeeklyCensusClientVersionsData> =
        WeeklyCensusClientVersionsData::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT
                census.id AS census_id,
                ANY_VALUE(DATE_TRUNC('second', census.started_at)) AS start,
                census_node.client_version AS version,
                COUNT(1) AS node_count
            FROM census
            LEFT JOIN census_node ON census.id = census_node.census_id
            WHERE
                census.sub_network = $2 AND
                census.started_at >= NOW() - INTERVAL '1 week' * ($1 + 1) AND
                census.started_at < NOW() - INTERVAL '1 week' * $1 AND
                census_node.client_name = $3
             GROUP BY
              census.id,
              census_node.client_version
            ORDER BY census.started_at
        ",
            vec![
                weeks_ago.into(),
                subprotocol.into(),
                client.to_string().into(),
            ],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup census node timeseries by client versions data");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(census_history))
}

#[derive(FromQueryResult)]
pub struct CensusHistoryOperatinSytemData {
    census_id: i32,
    start: DateTime<Utc>,
    operating_system: OperatingSystem,
    node_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CensusHistoryOperatinSytemDataCompact {
    census_id: i32,
    start: DateTime<Utc>,
    operating_system_slug: String,
    node_count: i64,
}

impl From<CensusHistoryOperatinSytemData> for CensusHistoryOperatinSytemDataCompact {
    fn from(value: CensusHistoryOperatinSytemData) -> Self {
        CensusHistoryOperatinSytemDataCompact {
            census_id: value.census_id,
            start: value.start,
            operating_system_slug: value.operating_system.to_string(),
            node_count: value.node_count,
        }
    }
}
pub async fn weekly_census_operating_systems(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<CensusHistoryOperatinSytemDataCompact>>, StatusCode> {
    let weeks_ago: i32 = match http_args.get("weeks-ago") {
        None => 0,
        Some(weeks_ago) => weeks_ago.parse::<i32>().unwrap_or(0),
    };

    let subprotocol = get_subprotocol_from_params(&http_args);

    let census_history: Vec<CensusHistoryOperatinSytemData> =
        CensusHistoryOperatinSytemData::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT
                census.id AS census_id,
                ANY_VALUE(DATE_TRUNC('second', census.started_at)) AS start,
                census_node.operating_system,
                COUNT(1) AS node_count
            FROM census
            LEFT JOIN census_node ON census.id = census_node.census_id
            WHERE
                census.sub_network = $2 AND
                census.started_at >= NOW() - INTERVAL '1 week' * ($1 + 1) AND
                census.started_at < NOW() - INTERVAL '1 week' * $1
            GROUP BY
              census.id,
              census_node.operating_system
            ORDER BY census.started_at
        ",
            vec![weeks_ago.into(), subprotocol.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup census node timeseries by operating system data");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let census_history_compact: Vec<CensusHistoryOperatinSytemDataCompact> =
        census_history.into_iter().map(|c| c.into()).collect();

    Ok(Json(census_history_compact))
}

#[derive(FromQueryResult)]
pub struct CensusHistoryProtocolVersionsData {
    census_id: i32,
    start: DateTime<Utc>,
    protocol_versions: Vec<u8>,
    node_count: i64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CensusHistoryProtocolVersionsDataCompact {
    census_id: i32,
    start: DateTime<Utc>,
    min_protocol_version: u8,
    max_protocol_version: u8,
    node_count: i64,
}

impl From<CensusHistoryProtocolVersionsData> for CensusHistoryProtocolVersionsDataCompact {
    fn from(value: CensusHistoryProtocolVersionsData) -> Self {
        let protocol_version_list: ProtocolVersionList =
            Decodable::decode(&mut value.protocol_versions.as_slice())
                .expect("Error decoding supported protocol versions");

        let protocol_versions: Vec<u8> =
            protocol_version_list.iter().map(|p| u8::from(*p)).collect();

        CensusHistoryProtocolVersionsDataCompact {
            census_id: value.census_id,
            start: value.start,
            min_protocol_version: *protocol_versions.iter().min().unwrap(),
            max_protocol_version: *protocol_versions.iter().max().unwrap(),
            node_count: value.node_count,
        }
    }
}
pub async fn weekly_census_protocol_versions(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<CensusHistoryProtocolVersionsDataCompact>>, StatusCode> {
    let weeks_ago: i32 = match http_args.get("weeks-ago") {
        None => 0,
        Some(weeks_ago) => weeks_ago.parse::<i32>().unwrap_or(0),
    };

    let subprotocol = get_subprotocol_from_params(&http_args);

    let census_history: Vec<CensusHistoryProtocolVersionsData> =
        CensusHistoryProtocolVersionsData::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT
                census.id AS census_id,
                ANY_VALUE(DATE_TRUNC('second', census.started_at)) AS start,
                key_value.value AS protocol_versions,
                COUNT(1) AS node_count
            FROM census
            LEFT JOIN census_node ON census.id = census_node.census_id
            LEFT JOIN record ON census_node.node_enr_id = record.id
            LEFT JOIN key_value ON record.id = key_value.node_enr_id
            WHERE
                census.sub_network = $2 AND
                census.started_at >= NOW() - INTERVAL '1 week' * ($1 + 1) AND
                census.started_at < NOW() - INTERVAL '1 week' * $1 AND
                key = '\\x7076' -- hex('pv')
            GROUP BY
              census.id,
              key_value.value
            ORDER BY census.started_at
        ",
            vec![weeks_ago.into(), subprotocol.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup census node timeseries by client protocol versions");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let census_history_compact: Vec<CensusHistoryProtocolVersionsDataCompact> =
        census_history.into_iter().map(|c| c.into()).collect();

    Ok(Json(census_history_compact))
}

#[derive(FromQueryResult, Debug, Clone, Serialize)]
pub struct TransferFailureBatches {
    start: DateTime<Utc>,
    client: String,
    failures: i64,
}

pub async fn weekly_transfer_failures(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<TransferFailureBatches>>, StatusCode> {
    let weeks_ago: i32 = match http_args.get("weeks-ago") {
        None => 0,
        Some(weeks_ago) => weeks_ago.parse::<i32>().unwrap_or(0),
    };

    let transfer_failures: Vec<TransferFailureBatches> =
        TransferFailureBatches::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT
                DATE_BIN('15 minutes', ca.created_at, TIMESTAMP '2001-01-01') AS start,
                convert_from(COALESCE(substr(substr(kv.value, 1, 2), length(substr(kv.value, 1, 2)), 1), 'unknown'), 'utf8') AS client,
                count(*) AS failures
            FROM audit_internal_failure AS aif
            LEFT JOIN record AS r ON aif.sender_node_enr_id=r.id
            LEFT JOIN key_value AS kv ON (r.id=kv.node_enr_id AND kv.key = '\x63') -- hex('c')
            LEFT JOIN content_audit AS ca ON aif.audit=ca.id
            WHERE
                ca.strategy_used = 5 AND -- Only consider 4444s audits now, there are too many History validation failures during current protocol transition
                ca.created_at IS NOT NULL AND
                ca.created_at > NOW() - INTERVAL '1 week' * ($1 + 1) AND
                ca.created_at < NOW() - INTERVAL '1 week' * $1
            GROUP BY start, client
            ORDER BY start;
        ",
            vec![weeks_ago.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup weekly transfer failures");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(transfer_failures))
}

fn nest_protocol_versions_clients(
    census: Vec<CensusProtocolVersionsClientsData>,
) -> HashMap<String, HashMap<String, i64>> {
    let mut nested = HashMap::<String, HashMap<String, i64>>::new();

    for row in census.into_iter() {
        let protocol_versions = match row.protocol_versions {
            Some(raw_protocol_versions) => {
                let protocol_version_list: ProtocolVersionList =
                    Decodable::decode(&mut raw_protocol_versions.as_slice())
                        .expect("Error decoding supported protocol versions");

                let protocol_versions: Vec<u8> =
                    protocol_version_list.iter().map(|p| u8::from(*p)).collect();

                protocol_versions
                    .iter()
                    .map(|pv| "v".to_string() + &pv.to_string())
                    .collect()
            }
            None => vec!["Unknown".to_string()],
        };

        for protocol_version in protocol_versions.iter() {
            *nested
                .entry(protocol_version.to_string())
                .or_default()
                .entry(row.client.to_string())
                .or_default() += row.node_count;
        }
    }
    nested
}

#[derive(FromQueryResult, Debug)]
pub struct CensusProtocolVersionsClientsData {
    protocol_versions: Option<Vec<u8>>,
    client: Client,
    node_count: i64,
}
pub async fn census_protocol_versions_clients(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<HashMap<String, HashMap<String, i64>>>, StatusCode> {
    let subprotocol = get_subprotocol_from_params(&http_args);

    let census: Vec<CensusProtocolVersionsClientsData> =
        CensusProtocolVersionsClientsData::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT
                protocol_versions.versions AS protocol_versions,
                client_name as client,
                COUNT(1) AS node_count
            FROM census
            LEFT JOIN census_node ON census.id = census_node.census_id
            LEFT JOIN record ON census_node.node_enr_id = record.id
            LEFT JOIN (
                SELECT
                    node_enr_id,
                    value AS versions
                FROM key_value                           // WHAT
                WHERE key = '\\x7076'
            ) protocol_versions ON record.id = protocol_versions.node_enr_id
            WHERE
                census.id = (
                    SELECT MAX(id)
                    FROM census
                    WHERE sub_network = $1
                ) AND
                census_node.census_id = (
                    SELECT MAX(id)
                    FROM census
                    WHERE sub_network = $1
                )
            GROUP BY
                versions,
                client_name
        ",
            vec![subprotocol.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup census node timeseries by client protocol versions");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(nest_protocol_versions_clients(census)))
}

#[derive(Debug, Clone, Serialize, FromQueryResult)]
#[serde(rename_all = "camelCase")]
pub struct AuditBlockStatusData {
    start: DateTime<Utc>,
    success: i64,
    error: i64,
    unaudited: i64,
}
pub async fn audit_block_status(
    http_args: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<AuditBlockStatusData>>, StatusCode> {
    const GENESIS_TIMESTAMP: &str = "1438269976"; // 2015-07-30 15:26:13
    const LAST_MINED_BLOCK_TIMESTAMP: &str = "1663224162"; // 2022-09-15 06:42:42
    const BIN_COUNT: i64 = 80; // ~200k blocks per bin for all pre-merge data

    let Ok(start) = i64::from_str(
        http_args
            .get("start")
            .unwrap_or(&(GENESIS_TIMESTAMP.to_string())),
    ) else {
        debug!("Audit Block Status: invalid start argument");
        return Err(StatusCode::BAD_REQUEST);
    };

    let Ok(end) = i64::from_str(
        http_args
            .get("end")
            .unwrap_or(&(LAST_MINED_BLOCK_TIMESTAMP.to_string())),
    ) else {
        debug!("Audit Block Status: invalid end argument");
        return Err(StatusCode::BAD_REQUEST);
    };

    let interval_size_seconds: i64 = (end - start) / BIN_COUNT;

    let Ok(content_type) =
        ContentType::from_str(http_args.get("content_type").unwrap_or(&("".to_string())))
    else {
        debug!("Audit Block Status: invalid content_type argument");
        return Err(StatusCode::BAD_REQUEST);
    };

    let audit_block_status: Vec<AuditBlockStatusData> =
    AuditBlockStatusData::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT
              start,
              success,
              error,
              (COALESCE((LEAD(number) OVER (ORDER BY start)), last_block) - number) - (success + error) AS unaudited
            FROM (
              SELECT
                 date_bin($1 * INTERVAL '1 second' , first_available_at, TO_TIMESTAMP($2)) AS start,
                 SUM(CASE WHEN RESULT = 1 THEN 1 ELSE 0 END) AS success,
                 SUM(CASE WHEN RESULT = 0 THEN 1 ELSE 0 END) AS error,
                 0::INT8 AS unaudited
              FROM public.audit_result_latest
              WHERE
                 content_type = $4 AND
                 first_available_at >= TO_TIMESTAMP($2) AND
                 first_available_at <= TO_TIMESTAMP($3)
              GROUP BY 1
            ) ranges,
            LATERAL (
              SELECT
                number,
                timestamp
              FROM block
              WHERE ranges.start <= block.timestamp
              ORDER BY timestamp
              LIMIT 1
            )
            CROSS JOIN (
              SELECT number AS last_block
                FROM block
                WHERE timestamp <= TO_TIMESTAMP($3)
                ORDER BY timestamp DESC
                LIMIT 1
            ) AS max_block_number
            ORDER BY start
            ",
            vec![interval_size_seconds.into(), start.into(), end.into(), content_type.into()],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup audit block status data");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(audit_block_status))
}

pub async fn single_census_view(
    params: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<SingleCensusViewTemplate>, StatusCode> {
    let subprotocol = get_subprotocol_from_params(&params);

    let Some(max_census_id) = get_max_census_id(&state, subprotocol).await else {
        return Err(StatusCode::from_u16(404).unwrap());
    };

    let census_id: i32 = match params.get("census-id") {
        Some(census_id) => census_id.parse::<i32>().unwrap_or(max_census_id),
        None => max_census_id,
    };
    if census_id < 1 || census_id > max_census_id {
        return Err(StatusCode::from_u16(404).unwrap());
    }

    let Some(client_diversity_data) = generate_client_diversity_data(&state, census_id).await
    else {
        return Err(StatusCode::from_u16(404).unwrap());
    };

    let Some(enr_list) = generate_enr_list_from_census_id(&state, census_id).await else {
        return Err(StatusCode::from_u16(404).unwrap());
    };

    let template = SingleCensusViewTemplate {
        client_diversity_data,
        node_count: enr_list.len() as i32,
        enr_list,
        census_id,
        max_census_id,
        created_at: get_created_data_from_census_id(&state, census_id).await,
    };

    Ok(HtmlTemplate(template))
}

async fn generate_enr_list_from_census_id(
    state: &Arc<State>,
    census_id: i32,
) -> Option<Vec<NodeEnr>> {
    let builder = state.database_connection.get_database_backend();
    NodeEnr::find_by_statement(
        census_node::Entity::find()
            .find_also_related(node_enr::Entity)
            .and_also_related(node::Entity)
            .select_only()
            .column(node_enr::Column::Raw)
            .column(node::Column::NodeId)
            .column(node_enr::Column::SequenceNumber)
            .filter(census_node::Column::CensusId.eq(census_id))
            .build(builder),
    )
    .all(&state.database_connection)
    .await
    .inspect_err(|err| {
        error!(census_id, %err, "Error getting enr list for census");
    })
    .ok()
}

async fn get_created_data_from_census_id(state: &Arc<State>, census_id: i32) -> String {
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
    let created_data = match CensusCreatedAt::find_by_statement(builder.build(&created_data))
        .one(&state.database_connection)
        .await
    {
        Ok(Some(data)) => data.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        Ok(None) => "".to_string(),
        Err(err) => {
            error!(err=?err, "Failed to lookup census creation time");
            "".to_string()
        }
    };
    created_data
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
    pub client_name: client_info::Client,
    pub client_count: i64,
}

#[derive(Debug, FromQueryResult, Serialize)]
pub struct NodeEnr {
    pub raw: String,
    pub node_id: Vec<u8>,
    pub sequence_number: i64,
}

impl NodeEnr {
    pub fn node_id(&self) -> String {
        hex_encode(&self.node_id)
    }
}

async fn generate_radius_graph_data(
    state: &Arc<State>,
    subprotocol: SubProtocol,
) -> Vec<CalculatedRadiusChartData> {
    let radius_chart_data = RadiusChartData::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "
        WITH latest_census AS (
            SELECT id
            FROM census
            WHERE sub_network = $1
            ORDER BY id DESC
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
            AND census_node.census_id == latest_census.id
            AND census_node.node_enr_id = record.id
            AND record.node_id = node.id
            ",
        vec![subprotocol.into()],
    ))
    .all(&state.database_connection)
    .await
    .unwrap();

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

async fn get_max_census_id(state: &Arc<State>, subprotocol: SubProtocol) -> Option<i32> {
    census::Entity::find()
        .select_only()
        .column_as(census::Column::Id.max(), "id")
        .filter(census::Column::SubProtocol.eq(subprotocol))
        .into_tuple::<i32>()
        .one(&state.database_connection)
        .await
        .inspect_err(|err| warn!("Census data unavailable: {err}"))
        .ok()
        .flatten()
}

async fn generate_client_diversity_data(
    state: &Arc<State>,
    census_id: i32,
) -> Option<Vec<ClientDiversityResult>> {
    census_node::Entity::find()
        .select_only()
        .column(census_node::Column::ClientName)
        .column_as(census_node::Column::Id.count(), "client_count")
        .filter(census_node::Column::CensusId.eq(census_id))
        .group_by(census_node::Column::ClientName)
        .into_model()
        .all(&state.database_connection)
        .await
        .inspect_err(|err| error!(census.id = census_id, %err, "Error getting client diversity"))
        .ok()
}

#[derive(FromQueryResult, Debug, Clone)]
pub struct TransferFailurePrimitive {
    pub audit: i32,
    pub client: String,
    pub created_at: DateTimeUtc,
    pub failure_type: TransferFailureType,
}

#[derive(FromQueryResult, Debug, Clone)]
pub struct TransferFailure {
    pub audit: i32,
    pub client: Client,
    pub created_at: DateTime<Utc>,
    pub failure_type: String,
}

pub async fn diagnostics(
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<DiagnosticsTemplate>, StatusCode> {
    // Query to get the 20 most recent internal failures joined with audits for timestamps
    // TODO: include more than 4444 errors (or maybe a dropdown to select which kind of error)
    // TODO(milos): fix
    let transfer_failures: Vec<TransferFailurePrimitive> =
        TransferFailurePrimitive::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "
            SELECT
                aif.audit,
                convert_from(COALESCE(substr(substr(kv.value, 1, 2), length(substr(kv.value, 1, 2)), 1), 'unknown'), 'utf8') AS client,
                ca.created_at,
                aif.failure_type
            FROM audit_transfer_failure AS aif
            LEFT JOIN record AS r ON aif.sender_node_enr_id=r.id
            LEFT JOIN key_value AS kv ON (r.id=kv.node_enr_id AND kv.key = '\x63') -- hex('c')  // WHAT
            LEFT JOIN content_audit AS ca ON aif.audit=ca.id
            WHERE ca.strategy_used = 5 AND ca.created_at IS NOT NULL
            ORDER BY created_at DESC
            LIMIT 20;",
            vec![],
        ))
        .all(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Failed to lookup weekly transfer failures");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let transfer_failures: Vec<TransferFailure> = transfer_failures
        .into_iter()
        .map(|f| {
            Ok::<TransferFailure, StatusCode>(TransferFailure {
                audit: f.audit,
                client: Client::from(f.client),
                created_at: f.created_at,
                failure_type: format!("{:?}", f.failure_type),
            })
        })
        .collect::<Result<Vec<_>, StatusCode>>()?;

    let template = DiagnosticsTemplate {
        failures: transfer_failures,
    };

    Ok(HtmlTemplate(template))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_nest_protocol_versions_clients() {
        let census = vec![
            CensusProtocolVersionsClientsData {
                client: Client::from("shisui".to_string()),
                protocol_versions: Some(vec![0u8]),
                node_count: 1,
            },
            CensusProtocolVersionsClientsData {
                client: Client::from("ultralight".to_string()),
                protocol_versions: Some(vec![0u8]),
                node_count: 7,
            },
            CensusProtocolVersionsClientsData {
                client: Client::from("shisui".to_string()),
                protocol_versions: Some(vec![130u8, 0u8, 1u8]),
                node_count: 8,
            },
            CensusProtocolVersionsClientsData {
                client: Client::from("trin".to_string()),
                protocol_versions: Some(vec![130u8, 0u8, 1u8]),
                node_count: 213,
            },
            CensusProtocolVersionsClientsData {
                client: Client::from(None),
                protocol_versions: Some(vec![130u8, 0u8, 1u8]),
                node_count: 191,
            },
            CensusProtocolVersionsClientsData {
                client: Client::from("trin".to_string()),
                protocol_versions: None,
                node_count: 21,
            },
            CensusProtocolVersionsClientsData {
                client: Client::from(None),
                protocol_versions: None,
                node_count: 1,
            },
        ];

        let nested = nest_protocol_versions_clients(census);

        let expected_nested = HashMap::from_iter([
            (
                "Unknown".to_string(),
                HashMap::from_iter([("trin".to_string(), 21), ("unknown".to_string(), 1)]),
            ),
            (
                "v0".to_string(),
                HashMap::from_iter([
                    ("trin".to_string(), 213),
                    ("shisui".to_string(), 9),
                    ("ultralight".to_string(), 7),
                    ("unknown".to_string(), 191),
                ]),
            ),
            (
                "v1".to_string(),
                HashMap::from_iter([
                    ("trin".to_string(), 213),
                    ("shisui".to_string(), 8),
                    ("unknown".to_string(), 191),
                ]),
            ),
        ]);

        assert_eq!(nested, expected_nested);
    }
}
