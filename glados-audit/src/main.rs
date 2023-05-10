use anyhow::Result;
use sea_orm::Database;
use tracing::{debug, info};

use glados_audit::{run_glados_audit, AuditConfig};
use migration::{Migrator, MigratorTrait};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    env_logger::init();

    info!("Starting glados-audit");

    //
    // CLI argument parsing
    //
    debug!("Parsing CLI arguments");

    let config = AuditConfig::from_args().await?;

    //
    // Database Connection
    //
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

    run_glados_audit(conn, config).await;
    Ok(())
}
