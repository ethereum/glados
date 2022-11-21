use std::sync::Arc;

use sea_orm::Database;

use clap::Parser;

use migration::{Migrator, MigratorTrait};

use glados_web::{cli::Args, run_glados_web, state::State};

#[tokio::main]
async fn main() {
    // parse command line arguments
    let args = Args::parse();

    let conn = Database::connect(args.database_url)
        .await
        .expect("Database connection failed");
    Migrator::up(&conn, None).await.unwrap();

    let config = Arc::new(State {
        ipc_path: args.ipc_path,
        database_connection: conn,
    });

    run_glados_web(config).await;
}
