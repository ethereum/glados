use axum::{
    extract::{Extension, Path, Query as HttpQuery},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use entity::{audit_stats, census, census_node, client_info};
use entity::{
    content,
    content_audit::{self, AuditResult},
    execution_metadata, key_value, node, record,
};
use ethportal_api::jsonrpsee::core::__reexports::serde_json;
use ethportal_api::types::distance::{Distance, Metric, XorMetric};
use ethportal_api::utils::bytes::{hex_decode, hex_encode};
use ethportal_api::{HistoryContentKey, OverlayContentKey};
use glados_core::stats::{filter_audits, get_audit_stats, AuditFilters, Period};
use migration::{Alias, JoinType, Order};
use sea_orm::sea_query::SimpleExpr;
use sea_orm::sea_query::{Expr, Query, SeaRc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, DynIden, EntityTrait,
    FromQueryResult, LoaderTrait, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Formatter;
use std::sync::Arc;
use std::{fmt::Display, io};
use tracing::error;
use tracing::info;

use crate::templates::{
    AuditDashboardTemplate, AuditTableTemplate, CensusExplorerPageTemplate,
    ContentAuditDetailTemplate, ContentDashboardTemplate, ContentIdDetailTemplate,
    ContentIdListTemplate, ContentKeyDetailTemplate, ContentKeyListTemplate, EnrDetailTemplate,
    HtmlTemplate, IndexTemplate, NetworkDashboardTemplate, NodeDetailTemplate,
    PaginatedCensusListTemplate,
};
use crate::{state::State, templates::AuditTuple};

//
// Routes
//
pub async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
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

async fn generate_radius_graph_data(state: &Arc<State>) -> Vec<CalculatedRadiusChartData> {
    let builder = state.database_connection.get_database_backend();
    let mut radius_density = Query::select();
    radius_density
        .expr(Expr::col((
            census_node::Entity,
            census_node::Column::DataRadius,
        )))
        .expr(Expr::col((
            census_node::Entity,
            census_node::Column::DataRadius,
        )))
        .expr(Expr::col((
            record::Entity,
            record::Column::Raw,
        )))
        .expr(Expr::col((node::Entity, node::Column::NodeId)))
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

    let radius_chart_data = RadiusChartData::find_by_statement(builder.build(&radius_density))
        .all(&state.database_connection)
        .await
        .unwrap();

    let mut radius_percentages: Vec<CalculatedRadiusChartData> = vec![];
    for i in radius_chart_data {
        let radius_high_bytes: [u8; 4] = [
            i.data_radius[0],
            i.data_radius[1],
            i.data_radius[2],
            i.data_radius[3],
        ];
        let radius_int = u32::from_be_bytes(radius_high_bytes);
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

        let percentage = (radius_int as f64 / u32::MAX as f64) * 100.0;
        let formatted_percentage = format!("{:.2}", percentage);

        let mut node_id_bytes: [u8; 32] = [0; 32];
        if i.node_id.len() == 32 {
            node_id_bytes.copy_from_slice(&i.node_id);
        }

        let node_id_string = hex_encode(node_id_bytes);
        radius_percentages.push(CalculatedRadiusChartData {
            data_radius: formatted_percentage.parse().unwrap(),
            node_id: u64::from_be_bytes(node_id_high_bytes),
            node_id_string,
            raw_enr: i.raw,
        });
    }

    radius_percentages
}

async fn get_max_census_id(state: &Arc<State>) -> Option<MaxCensusId> {
    let builder = state.database_connection.get_database_backend();
    let max_census_id = Query::select()
        .from(census::Entity)
        .expr_as(Expr::max(Expr::col(census::Column::Id)), Alias::new("id"))
        .take();
    match MaxCensusId::find_by_statement(builder.build(&max_census_id))
        .one(&state.database_connection)
        .await
    {
        Ok(val) => val,
        Err(err) => {
            error!("{err}");
            None
        }
    }
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

async fn generate_client_diversity_data(
    state: &Arc<State>,
    census_id: Option<i32>,
    max_census_id: MaxCensusId,
) -> Option<Vec<ClientDiversityResult>> {
    let builder = state.database_connection.get_database_backend();
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

    let left_table: DynIden = SeaRc::new(Alias::new("left_table"));
    let right_table: DynIden = SeaRc::new(Alias::new("right_table"));
    let mut client_count = Query::select();
    client_count
        .expr_as(
            Expr::cust("CAST(COUNT(*) AS INT)"),
            Alias::new("client_count"),
        )
        .expr_as(
            Expr::cust("CAST(COALESCE(substr(substr(value, 1, 2), length(substr(value, 1, 2)), 1), 'unknown') AS TEXT)"),
            Alias::new("client_name"),
        )
        .from_subquery(
            Query::select()
                .expr_as(
                    Expr::col(census_node::Column::RecordId),
                    Alias::new("record_id"),
                )
                .from(census_node::Entity)
                .from_subquery(
                    census_selection_query,
                    Alias::new("selected_census_id"),
                )
                .and_where(
                    Expr::col((Alias::new("census_node"), Alias::new("census_id")))
                        .eq(Expr::col((Alias::new("selected_census_id"), Alias::new("id")))),
                )
                .take(),
            left_table.clone(),
        )
        .join_subquery(
            JoinType::LeftJoin,
            Query::select()
                .from(key_value::Entity)
                .column(key_value::Column::RecordId)
                .column(key_value::Column::Value)
                .and_where(
                    Expr::cust("convert_from(key, 'UTF8')").eq("c"),
                )
                .take(),
            right_table.clone(),
            Expr::col((left_table.clone(), Alias::new("record_id")))
                .equals((right_table.clone(), Alias::new("record_id"))),
        )
        .add_group_by([Expr::cust("substr(substr(value, 1, 2), length(substr(value, 1, 2)), 1)")]);

    Some(
        ClientDiversityResult::find_by_statement(builder.build(&client_count))
            .all(&state.database_connection)
            .await
            .unwrap(),
    )
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

pub async fn root(Extension(state): Extension<Arc<State>>) -> impl IntoResponse {
    let client_diversity_data = match get_max_census_id(&state).await {
        None => vec![],
        Some(max_census_id) => generate_client_diversity_data(&state, None, max_census_id)
            .await
            .unwrap(),
    };

    let radius_percentages = generate_radius_graph_data(&state).await;
    let open_filter = content_audit::Entity::find();
    // Run queries for content dashboard data concurrently
    let (hour_stats, day_stats, week_stats) = tokio::join!(
        get_audit_stats(
            open_filter.clone(),
            Period::Hour,
            &state.database_connection
        ),
        get_audit_stats(open_filter.clone(), Period::Day, &state.database_connection),
        get_audit_stats(
            open_filter.clone(),
            Period::Week,
            &state.database_connection
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

    let open_filter = content_audit::Entity::find();
    // Run queries for content dashboard data concurrently
    let (
        audits_of_recent_content,
        recent_audits,
        recent_audit_successes,
        recent_audit_failures,
        hour_stats,
        day_stats,
        week_stats,
    ) = tokio::join!(
        get_audits_for_recent_content(KEY_COUNT, &state.database_connection),
        get_recent_audits(KEY_COUNT, &state.database_connection),
        get_recent_audit_successes(KEY_COUNT, &state.database_connection),
        get_recent_audit_failures(KEY_COUNT, &state.database_connection),
        get_audit_stats(
            open_filter.clone(),
            Period::Hour,
            &state.database_connection
        ),
        get_audit_stats(open_filter.clone(), Period::Day, &state.database_connection),
        get_audit_stats(
            open_filter.clone(),
            Period::Week,
            &state.database_connection
        ),
    );

    // Get results from queries
    let audits_of_recent_content: Vec<AuditTuple> = audits_of_recent_content?;
    let recent_audits: Vec<AuditTuple> = recent_audits?;
    let recent_audit_successes: Vec<AuditTuple> = recent_audit_successes?;
    let recent_audit_failures: Vec<AuditTuple> = recent_audit_failures?;
    let hour_stats = hour_stats.map_err(|e| {
        error!(err=?e, "Could not look up recent audits");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let day_stats = day_stats.map_err(|e| {
        error!(err=?e, "Could not look up recent audits");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let week_stats = week_stats.map_err(|e| {
        error!(err=?e, "Could not look up recent audits");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let template = ContentDashboardTemplate {
        stats: [hour_stats, day_stats, week_stats],
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

pub async fn contentaudit_dashboard() -> Result<HtmlTemplate<AuditDashboardTemplate>, StatusCode> {
    let template = AuditDashboardTemplate {};
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
    HtmlTemplate(template)
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
        let radius = Distance::from(crate::U256::from_big_endian(&dead_zone_data.data_radius));
        let node_id = Distance::from(crate::U256::from_big_endian(&dead_zone_data.node_id));
        if XorMetric::distance(&content_id, &node_id.big_endian()) <= radius {
            enrs.push(dead_zone_data.raw);
        }
    }

    Ok(Json(enrs))
}

pub async fn get_audit_stats_handler(
    Extension(state): Extension<Arc<State>>,
) -> Result<Json<Vec<audit_stats::Model>>, StatusCode> {
    let stats = audit_stats::get_recent_stats(&state.database_connection)
        .await
        .map_err(|e| {
            error!(err=?e, "Could not look up audit stat history");
            StatusCode::INTERNAL_SERVER_ERROR
        })
        .unwrap();

    Ok(Json(stats))
}

pub async fn census_explorer_list(
    page: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<PaginatedCensusListTemplate>, StatusCode> {
    let max_census_id = match get_max_census_id(&state).await {
        None => return Err(StatusCode::from_u16(404).unwrap()),
        Some(max_census_id) => max_census_id,
    };

    let mut list_census_page_id: i32 = match page.get("page") {
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

pub async fn census_explorer(
    census_id: HttpQuery<HashMap<String, String>>,
    Extension(state): Extension<Arc<State>>,
) -> Result<HtmlTemplate<CensusExplorerPageTemplate>, StatusCode> {
    let max_census_id = match get_max_census_id(&state).await {
        None => return Err(StatusCode::from_u16(404).unwrap()),
        Some(max_census_id) => max_census_id,
    };

    let census_id: Option<i32> = match census_id.get("census-id") {
        None => return Err(StatusCode::from_u16(404).unwrap()),
        Some(census_id) => match census_id.parse::<i32>() {
            Ok(census_id) => Some(census_id),
            Err(_) => Some(max_census_id.id),
        },
    };

    let client_diversity_data =
        match generate_client_diversity_data(&state, census_id, max_census_id).await {
            None => return Err(StatusCode::from_u16(404).unwrap()),
            Some(client_diversity_data) => client_diversity_data,
        };

    let enr_list = match generate_enr_list_from_census_id(&state, census_id, max_census_id).await {
        None => return Err(StatusCode::from_u16(404).unwrap()),
        Some(enr_list) => enr_list,
    };

    let template = CensusExplorerPageTemplate {
        client_diversity_data,
        node_count: enr_list.len() as i32,
        enr_list,
        census_id: census_id.unwrap(),
        max_census_id: max_census_id.id,
        created_at: get_created_data_from_census_id(&state, census_id.unwrap())
            .await
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
    };

    Ok(HtmlTemplate(template))
}
