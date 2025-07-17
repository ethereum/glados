use anyhow::Result;
use clap::Parser;
use glados_audit::stats::periodically_record_stats;
use sea_orm::Database;
use tokio::time::Duration;
use tracing::{debug, info};

use glados_audit::cli::Args;
use glados_audit::{run_glados_audit, AuditConfig};
use migration::{Migrator, MigratorTrait};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    env_logger::init();

    debug!("Parsing CLI arguments");
    let args = Args::parse();

    info!("Starting glados-audit");
    run_audit(args).await
}

async fn run_audit(args: Args) -> Result<()> {
    //
    // Database Connection
    //
    let config = AuditConfig::from_args(args).await?;
    debug!(
        database_url = &config.database_url,
        "Connecting to database"
    );

    let conn = Database::connect(&config.database_url).await?;
    info!(
        database_url = &config.database_url,
        "database connection established"
    );

    Migrator::up(&conn, None).await?;
    tokio::spawn(periodically_record_stats(
        Duration::from_secs(config.stats_recording_period),
        conn.clone(),
    ));
    run_glados_audit(conn, config).await;
    Ok(())
}
