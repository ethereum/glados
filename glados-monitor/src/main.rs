use clap::Parser;
use sea_orm::{Database, DatabaseConnection};
use tokio::{signal, task};
use tracing::{debug, info};

use glados_monitor::{
    cli::{Cli, Commands},
    import_pre_merge_accumulators, run_glados_monitor,
};
use migration::{DbErr, Migrator, MigratorTrait};

#[tokio::main]
async fn main() -> Result<(), DbErr> {
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

    if no_tables(&conn).await {
        info!("No database tables present, creating new tables.");
        Migrator::up(&conn, None)
            .await
            .expect("could not create new database tables");
    }

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
    Ok(())
}

async fn do_nothing() -> Result<(), DbErr> {
    Ok(())
}

async fn follow_head_command(conn: DatabaseConnection, provider_url: String) -> Result<(), DbErr> {
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

/// Returns true if there are no tables in the database.
///
/// Useful if deciding whether to create a new database or not.
///
/// Rather than looking for individual tables (which may break
/// if new tables are added/removed), this handles the search generally
/// for the supported tables in Sea_Orm.
async fn no_tables(db: &DatabaseConnection) -> bool {
    // A statement finding all tables for the given backend.
    let statement = match db.get_database_backend() {
        backend @ DbBackend::MySql => {
            let q = "SHOW TABLES";
            warn!("This {backend:?} query to retrieve all tables is untested: {q}");
            Statement::from_sql_and_values(backend, q, vec![])
        }
        backend @ DbBackend::Postgres => {
            let q = "SELECT * FROM information_schema.tables WHERE table_schema='public';";
            warn!("This {backend:?} query to retrieve all tables is untested: {q}");
            Statement::from_sql_and_values(backend, q, vec![])
        }
        backend @ DbBackend::Sqlite => {
            let q = "SELECT name FROM sqlite_master WHERE type='table';";
            Statement::from_sql_and_values(backend, q, vec![])
        }
    };
    JsonValue::find_by_statement(statement)
        .all(db)
        .await
        .expect("Couldn't query database for tables")
        .is_empty()
}
