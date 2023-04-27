use std::sync::Arc;
use std::{net::SocketAddr, path::Path};

use anyhow::{bail, Result};
use axum::http::StatusCode;
use axum::{
    extract::Extension,
    routing::{get, get_service},
    Router,
};
use tower_http::services::ServeDir;
use tracing::info;

pub mod cli;
pub mod routes;
pub mod state;
pub mod templates;

use crate::state::State;

const SOCKET: &str = "0.0.0.0:3001";

pub async fn run_glados_web(config: Arc<State>) -> Result<()> {
    let Some(parent) = Path::new(std::file!()).parent() else {bail!("No parent of config file")};
    let Some(grandparent) = parent.parent() else {bail!("No grandparent of config file")};
    let assets_path = grandparent.join("assets");

    let serve_dir = get_service(ServeDir::new(assets_path)).handle_error(routes::handle_error);

    // setup router
    let app = Router::new()
        .route("/", get(routes::root))
        .route("/network/", get(routes::network_dashboard))
        .route("/network/node/:node_id_hex/", get(routes::node_detail))
        .route(
            "/network/node/:node_id_hex/enr/:enr_seq/",
            get(routes::enr_detail),
        )
        .route("/content/", get(routes::content_dashboard))
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
        .nest_service("/static/", serve_dir.clone())
        .fallback_service(serve_dir)
        .layer(Extension(config));

    let app = app.fallback(handler_404);

    let socket: SocketAddr = SOCKET.parse()?;
    info!("Serving glados-web at {}", socket);
    Ok(axum::Server::bind(&socket)
        .serve(app.into_make_service())
        .await?)
}

/// Global routing error handler to prevent panics.
async fn handler_404() -> StatusCode {
    tracing::error!("404: Non-existent page visited");
    StatusCode::NOT_FOUND
}
