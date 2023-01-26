use std::path::PathBuf;

use clap::Parser;
use sea_orm::Database;
use tracing::{debug, info};

use glados_audit::{cli::Args, run_glados_audit};
use migration::{Migrator, MigratorTrait};

#[tokio::main]
async fn main() {
    // Setup logging
    env_logger::init();

    info!("Starting glados-audit");

    //
    // CLI argument parsing
    //
    debug!("Parsing CLI arguments");

    let args = Args::parse();

    //
    // Database Connection
    //
    debug!(database_url = &args.database_url, "Connecting to database");

    let conn = Database::connect(&args.database_url)
        .await
        .expect("Database connection failed");

    info!(
        database_url = &args.database_url,
        "database connection established"
    );

    Migrator::up(&conn, None).await.unwrap();

    let ipc_path: PathBuf = args.ipc_path;

    run_glados_audit(conn, ipc_path).await;
}
