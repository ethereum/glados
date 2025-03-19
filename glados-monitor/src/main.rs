use anyhow::Result;
use clap::Parser;
use glados_monitor::{
    beacon::{panda_ops_http, PANDA_OPS_BEACON},
    bulk_download_block_data,
    cli::{Cli, Commands},
    panda_ops_web3, run_glados_monitor, run_glados_monitor_beacon,
    state::{follow_head_state_command, populate_state_roots_range_command},
};
use migration::{Migrator, MigratorTrait, SeedTrait};
use sea_orm::{Database, DatabaseConnection};
use tokio::{signal, task};
use tracing::{debug, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    env_logger::init();

    info!("Starting glados-monitor");

    //
    // CLI argument parsing
    //
    debug!("Parsing CLI arguments");

    let cli = Cli::parse();
    //
    // Database Connection
    //
    debug!(DATABASE_URL = &cli.database_url, "Connecting to database");

    let conn = Database::connect(&cli.database_url)
        .await
        .expect("Database connection failed");

    info!(
        DATABASE_URL = &cli.database_url,
        "database connection established"
    );

    if cli.migrate {
        info!("running database migrations");
        Migrator::up(&conn, None)
            .await
            .expect("Database migration failed");
    }

    let task_handle = match &cli.command {
        Some(Commands::FollowHead { provider_url }) => {
            info!("Running follow head");
            task::spawn(follow_head_command(conn, provider_url.to_string()))
        }
        Some(Commands::FollowHeadPandaops { provider_url }) => {
            info!("Running follow head beacon");
            task::spawn(follow_head_command_pandaops(conn, provider_url.to_string()))
        }
        Some(Commands::FollowBeacon { beacon_base_url }) => {
            info!("Running follow beacon");
            task::spawn(follow_beacon_command(conn, beacon_base_url.to_string()))
        }
        Some(Commands::FollowBeaconPandaops {}) => {
            task::spawn(follow_beacon_command_pandaops(conn))
        }
        Some(Commands::BulkDownloadBlockData {
            start_block_number,
            end_block_number,
            provider_url,
            concurrency,
        }) => {
            info!("Bulk downloading block data");
            task::spawn(bulk_download_block_data(
                conn,
                *start_block_number,
                *end_block_number,
                provider_url.to_string(),
                *concurrency,
            ))
        }
        Some(Commands::FollowHeadState { provider_url }) => {
            info!("Running follow head state");
            task::spawn(follow_head_state_command(conn, provider_url.to_string()))
        }
        Some(Commands::PopulateStateRootsRange {
            start_block_number,
            end_block_number,
            provider_url,
            concurrency,
        }) => {
            info!("Downloading state roots for selected range");
            task::spawn(populate_state_roots_range_command(
                conn,
                *start_block_number,
                *end_block_number,
                provider_url.to_string(),
                *concurrency,
            ))
        }
        Some(Commands::Seed { table_name }) => {
            info!("Running seed");
            task::spawn(Migrator::seed_by_table(conn, table_name.to_string()))
        }
        &None => {
            info!("No command specified");
            task::spawn(do_nothing())
        }
    };

    // Wait for either the signal stream or the oneshot channel to receive a message
    tokio::select! {
        _ = signal::ctrl_c() => {
            println!("Received a CTRL+C signal, exiting");
        }
        task_result = task_handle => {
            match task_result {
                Ok(Err(err)) => {
                    println!("Error: {:?}", err);
                },
                Ok(Ok(())) => {
                    println!("Command completed successfully");
                }
                Err(e) => println!("Task failed: {:?}", e),

            }

        }
    }
    Ok(())
}

async fn do_nothing() -> Result<()> {
    Ok(())
}

async fn follow_head_command(conn: DatabaseConnection, provider_url: String) -> Result<()> {
    //
    // Web3 Connection
    //
    debug!("Connecting to web3 provider");

    let transport =
        web3::transports::Http::new(&provider_url).expect("Failed to setup web3 transport");
    let w3 = web3::Web3::new(transport);

    info!(
        provider_url = &provider_url,
        "web3 provider connection established"
    );

    run_glados_monitor(conn, w3).await;
    Ok(())
}

async fn follow_beacon_command(conn: DatabaseConnection, beacon_url: String) -> Result<()> {
    let http_client = reqwest::Client::builder().build().unwrap();
    run_glados_monitor_beacon(conn, http_client, beacon_url).await;
    Ok(())
}

async fn follow_beacon_command_pandaops(conn: DatabaseConnection) -> Result<()> {
    let http_client = panda_ops_http()?;
    run_glados_monitor_beacon(conn, http_client, PANDA_OPS_BEACON.to_string()).await;
    Ok(())
}

async fn follow_head_command_pandaops(
    conn: DatabaseConnection,
    provider_url: String,
) -> Result<()> {
    //
    // Web3 Connection
    //
    debug!("Connecting to pandaops provider");

    let w3 = panda_ops_web3(&provider_url)?;
    let client_version = w3.web3().client_version().await?;
    info!(
        client_version = client_version,
        provider_url = &provider_url,
        "web3 pandaops connection established"
    );

    run_glados_monitor(conn, w3).await;
    Ok(())
}
