use tokio::signal;
use tokio::task;

use sea_orm::{Database, DatabaseConnection, ConnectOptions};

use tracing::{debug, info};

use clap::Parser;

use migration::{Migrator, MigratorTrait};

use glados_monitor::{
    cli::{Cli, Commands},
    import_pre_merge_accumulators, run_glados_monitor,
};

#[tokio::main]
async fn main() {
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

    let mut opt = ConnectOptions::new(cli.database_url.to_owned());
    opt.sqlx_logging(false);

    let conn = Database::connect(opt)
        .await
        .expect("Database connection failed");

    info!(
        DATABASE_URL = &cli.database_url,
        "database connection established"
    );

    if cli.migrate {
        info!("running database migrations");
        Migrator::up(&conn, None).await.unwrap();
    }

    let task_handle = match &cli.command {
        Some(Commands::FollowHead { provider_url }) => {
            info!("Running follow head");
            task::spawn(follow_head_command(conn, provider_url.to_string()))
        }
        Some(Commands::ImportPreMergeAccumulators { path }) => {
            info!("Importing pre-merge accumulators");
            task::spawn(import_pre_merge_accumulators(conn, path.to_path_buf()))
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
        _ = task_handle => {
            println!("Command completed, exiting");
        }
    }
}

async fn do_nothing() {}

async fn follow_head_command(conn: DatabaseConnection, provider_url: String) {
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
}
