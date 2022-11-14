use sea_orm::Database;

use clap::Parser;

use web3;

use crate::cli::Args

#[tokio::main]
async fn main() {
    // parse command line arguments
    let args = Args::parse();

    // connect to the database
    let conn = Database::connect(args.database_url)
        .await
        .expect("Database connection failed");

    let transport = web3::transports::Http::new(&args.database_url)?;
    let web3 = web3::Web3::new(transport);

    // TODO: long running process that fetches latest block...
}
