use sea_orm::Database;

use clap::Parser;

use migration::{Migrator, MigratorTrait};

use glados_monitor::{cli::Args, run_glados_monitor};

#[tokio::main]
async fn main() {
    // parse command line arguments
    let args = Args::parse();

    // connect to the database
    let conn = Database::connect(args.database_url)
        .await
        .expect("Database connection failed");

    Migrator::up(&conn, None).await.unwrap();

    println!("Setting up web3 connection");
    let transport =
        web3::transports::Http::new(&args.provider_url).expect("Failed to setup web3 transport");
    let w3 = web3::Web3::new(transport);

    run_glados_monitor(conn, w3).await;
}
