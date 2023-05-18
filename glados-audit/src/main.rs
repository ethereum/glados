use anyhow::Result;
use clap::Parser;
use sea_orm::Database;
use tracing::{debug, info};

use glados_audit::cli::{Args, Command};
use glados_audit::{run_glados_audit, run_glados_command, AuditConfig};
use migration::{Migrator, MigratorTrait};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    env_logger::init();
    let args = Args::parse();
    info!("Starting glados-audit");

    //
    // CLI argument parsing
    //
    debug!("Parsing CLI arguments");

    match args.subcommand {
        Some(command) => run_command(command).await?,
        None => run_audit(args).await?,
    }
    Ok(())
}

async fn run_command(command: Command) -> Result<()> {
    //
    // Database Connection
    //
    let database_url = match &command {
        Command::Audit { database_url, .. } => database_url,
    };
    debug!(database_url = database_url, "Connecting to database");

    let conn = Database::connect(database_url).await?;
    info!(
        database_url = database_url,
        "database connection established"
    );

    Migrator::up(&conn, None).await?;
    run_glados_command(conn, command).await
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
    run_glados_audit(conn, config).await;
    Ok(())
}
