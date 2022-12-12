use std::path::Path;
use std::sync::Arc;

use axum::{
    extract::Extension,
    routing::{get, get_service},
    Router,
};

use tower_http::services::ServeDir;

pub mod cli;
pub mod routes;
pub mod state;
pub mod templates;

use crate::state::State;

pub async fn run_glados_web(config: Arc<State>) {
    let assets_path = Path::new(std::file!())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("assets");
    let serve_dir = get_service(ServeDir::new(assets_path)).handle_error(routes::handle_error);

    // setup router
    let app = Router::new()
        .route("/", get(routes::root))
        .route("/nodes/", get(routes::node_list))
        .route("/content/", get(routes::content_dashboard))
        .route("/content/id/", get(routes::contentid_list))
        .route("/content/id/:content_id_hex", get(routes::contentid_detail))
        .route("/content/key/", get(routes::contentkey_list))
        .route(
            "/content/key/:content_key_hex",
            get(routes::contentkey_detail),
        )
        .nest_service("/static/", serve_dir.clone())
        .fallback_service(serve_dir)
        .layer(Extension(config));

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3001".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
