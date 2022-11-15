//use sea_orm::Database;
use std::time::Duration;

use clap::Parser;

use tokio::time::sleep;

use web3;

mod cli;
use cli::Args;

#[tokio::main]
async fn main() {
    // parse command line arguments
    let args = Args::parse();

    // connect to the database
    //let conn = Database::connect(args.database_url)
    //    .await
    //    .expect("Database connection failed");

    println!("Setting up web3 connection");
    let transport = web3::transports::Http::new(&args.provider_url).unwrap();
    let w3 = web3::Web3::new(transport);

    monitor_blocks(w3).await;
}

async fn monitor_blocks(w3: web3::Web3<web3::transports::Http>) {
    println!("Initializing block number...");

    let start_block_number = w3.eth().block_number().await.unwrap();

    println!("Starting Block Number={}", start_block_number);
    
    // TODO: long running process that fetches latest block...
    let mut block_number = start_block_number;

    loop {
        println!("Sleeping....");
        sleep(Duration::from_secs(5)).await;
        println!("Checking for new block...");

        let candidate_block_number = w3.eth().block_number().await.unwrap();

        if candidate_block_number > block_number {
            block_number = candidate_block_number;
            println!("New block: {}", block_number);
            if block_number > start_block_number + 10 {
                break;
            }
        } else {
            println!("Same block: {}", candidate_block_number);
        }
    }
}
