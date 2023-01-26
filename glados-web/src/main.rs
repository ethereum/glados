use std::sync::Arc;

use clap::Parser;
use sea_orm::Database;

use glados_web::{cli::Args, run_glados_web, state::State};
use migration::{Migrator, MigratorTrait};

#[tokio::main]
async fn main() {
    // parse command line arguments
    let args = Args::parse();

    let conn = Database::connect(args.database_url)
        .await
        .expect("Database connection failed");
    Migrator::up(&conn, None).await.unwrap();

    let config = Arc::new(State {
        database_connection: conn,
    });

    run_glados_web(config).await;
}
