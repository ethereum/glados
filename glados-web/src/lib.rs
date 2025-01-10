use std::sync::Arc;
use std::{net::SocketAddr, path::Path};

use anyhow::{bail, Result};
use axum::{
    extract::Extension,
    routing::{get, get_service},
    Router,
};
use tower_http::services::ServeDir;
use tracing::info;

use alloy_primitives::U256;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

pub mod cli;
pub mod routes;
pub mod state;
pub mod templates;

use crate::state::State;

const SOCKET: &str = "0.0.0.0:3001";

const ASSET_PATH_ENV_VAR: &str = "GLADOS_WEB_ASSETS_PATH";

pub async fn run_glados_web(config: Arc<State>) -> Result<()> {
    let assets_path = match std::env::var(ASSET_PATH_ENV_VAR) {
        Ok(path) => Path::new(&path).to_path_buf(),
        Err(_) => {
            let Some(parent) = Path::new(std::file!()).parent() else {
                bail!("No parent of config file")
            };
            let Some(grandparent) = parent.parent() else {
                bail!("No grandparent of config file")
            };
            grandparent.join("assets")
        }
    };

    let serve_dir = get_service(ServeDir::new(assets_path)).handle_error(routes::handle_error);

    let nodes_with_zero_high_bits = entity::node::Entity::find()
        .filter(entity::node::Column::NodeIdHigh.eq(0))
        .all(&config.database_connection)
        .await
        .unwrap();

    info!(rows=?nodes_with_zero_high_bits.len(), "One time migration: setting high bits for node model");

    for node_model in nodes_with_zero_high_bits {
        let raw_node_id = U256::from_be_slice(&node_model.get_node_id().raw());
        let node_id_high: i64 = raw_node_id.wrapping_shr(193).to::<i64>();

        let mut node: entity::node::ActiveModel = node_model.into();
        let previous_value = node.node_id_high;
        node.node_id_high = Set(node_id_high);
        let updated = node.update(&config.database_connection).await?;
        info!(row.id=?updated.id, old=?previous_value, new=?updated.node_id_high, "Setting high bits");
    }

    // setup router
    let app = Router::new()
        .route("/", get(routes::network_overview))
        .route("/census/census-list/", get(routes::census_explorer_list))
        .route("/census/", get(routes::single_census_view))
        .route("/census/explorer", get(routes::census_explorer))
        .route("/network/node/:node_id_hex/", get(routes::node_detail))
        .route(
            "/network/node/:node_id_hex/enr/:enr_seq/",
            get(routes::enr_detail),
        )
        .route("/content/id/", get(routes::contentid_list))
        .route(
            "/content/id/:content_id_hex/",
            get(routes::contentid_detail),
        )
        .route("/content/key/", get(routes::contentkey_list))
        .route(
            "/content/key/:content_key_hex/",
            get(routes::contentkey_detail),
        )
        .route("/audit/id/:audit_id", get(routes::contentaudit_detail))
        .route("/audits/", get(routes::contentaudit_dashboard))
        .route("/audits/filter/", get(routes::contentaudit_filter))
        .route(
            "/api/hourly-success-rate/",
            get(routes::hourly_success_rate),
        )
        .route(
            "/api/is-content-in-deadzone/:content_key",
            get(routes::is_content_in_deadzone),
        )
        .route(
            "/api/stats-history/",
            get(routes::get_history_audit_stats_handler),
        )
        .route(
            "/api/stats-state/",
            get(routes::get_state_audit_stats_handler),
        )
        .route(
            "/api/stats-beacon/",
            get(routes::get_beacon_audit_stats_handler),
        )
        .route("/api/failed-keys/", get(routes::get_failed_keys_handler))
        .route("/api/census-weekly/", get(routes::weekly_census_history))
        .route(
            "/census/census-node-timeseries-data/",
            get(routes::census_timeseries),
        )
        .nest_service("/static/", serve_dir.clone())
        .fallback_service(serve_dir)
        .layer(Extension(config));

    let socket: SocketAddr = SOCKET.parse()?;
    info!("Serving glados-web at {}", socket);
    Ok(axum::Server::bind(&socket)
        .serve(app.into_make_service())
        .await?)
}
