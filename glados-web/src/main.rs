use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use glados_web::{
    cli::{Cli, Commands},
    run_glados_web,
    state::State,
};
use migration::{Migrator, MigratorTrait, SeedTrait};
use sea_orm::{Database, DatabaseConnection};
use tokio::{signal, task};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    // parse command line arguments
    let cli = Cli::parse();

    let conn = Database::connect(cli.database_url)
        .await
        .expect("Database connection failed");

    let task_handle = match &cli.command {
        Some(Commands::Seed { table_name }) => {
            info!("Running seed");
            task::spawn(Migrator::seed_by_table(conn, table_name.to_string()))
        }
        None => task::spawn(serve(conn, cli.skip_seeding)),
    };

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

async fn serve(conn: DatabaseConnection, skip_seeding: bool) -> Result<()> {
    let previous_migrations = Migrator::get_migration_models(&conn)
        .await
        .unwrap()
        .into_iter()
        .map(|migration| migration.version)
        .collect();

    Migrator::up(&conn, None).await?;

    Migrator::seed_new_migrations(&conn, previous_migrations, skip_seeding).await?;

    let config = Arc::new(State {
        database_connection: conn,
    });

    run_glados_web(config).await?;
    Ok(())
}
