use sea_orm::Database;

use tracing::{debug, info};

use clap::Parser;

use migration::{Migrator, MigratorTrait};

use glados_monitor::{cli::Args, run_glados_monitor};

#[tokio::main]
async fn main() {
    // Setup logging
    env_logger::init();

    info!("Starting glados-monitor");

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

    //
    // Web3 Connection
    //
    debug!("Connecting to web3 provider");

    let transport =
        web3::transports::Http::new(&args.provider_url).expect("Failed to setup web3 transport");
    let w3 = web3::Web3::new(transport);

    info!(
        provider_url = &args.provider_url,
        "web3 provider connection established"
    );

    run_glados_monitor(conn, w3).await;
}
