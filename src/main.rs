use std::sync::Arc;

use sea_orm::Database;

use clap::Parser;

use axum::{extract::Extension, routing::get, Router};

use migration::{Migrator, MigratorTrait};

use glados_core::cli::Args;

use glados_web::{routes, state::State};

#[tokio::main]
async fn main() {
    // parse command line arguments
    let args = Args::parse();

    let conn = Database::connect(args.database_url)
        .await
        .expect("Database connection failed");
    Migrator::up(&conn, None).await.unwrap();

    let shared_state = Arc::new(State {
        ipc_path: args.ipc_path,
        database_connection: conn,
    });

    // setup router
    let app = Router::new()
        .route("/", get(routes::root))
        .route("/nodes/", get(routes::node_list))
        .layer(Extension(shared_state));

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3001".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
