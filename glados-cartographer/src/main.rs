use anyhow::Result;
use sea_orm::Database;
use tracing::{debug, info};

use glados_cartographer::{run_glados_cartographer, CartographerConfig};
use migration::{Migrator, MigratorTrait};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    env_logger::init();

    info!("Starting glados-cartographer");

    //
    // CLI argument parsing
    //
    debug!("Parsing CLI arguments");

    let config = CartographerConfig::from_args()?;

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

    run_glados_cartographer(conn, config).await;
    Ok(())
}
