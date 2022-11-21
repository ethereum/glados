use std::sync::Arc;

use axum::{extract::Extension, routing::get, Router};

pub mod routes;
pub mod state;
use crate::state::State;
pub mod cli;
pub mod templates;

pub async fn run_glados_web(config: Arc<State>) {
    // setup router
    let app = Router::new()
        .route("/", get(routes::root))
        .route("/nodes/", get(routes::node_list))
        .route("/contentid/", get(routes::contentid_list))
        .layer(Extension(config));

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3001".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
