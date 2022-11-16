//use sea_orm::Database;
use std::time::Duration;

use clap::Parser;

use tokio::time::sleep;
use tokio::sync::mpsc;

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
    let transport = web3::transports::Http::new(&args.provider_url)
        .expect("Failed to setup web3 transport");
    let w3 = web3::Web3::new(transport);

    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(follow_chain_head(w3.clone(), tx));
    tokio::spawn(retrieve_new_blocks(w3.clone(), rx));

    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");
}

async fn follow_chain_head(
    w3: web3::Web3<web3::transports::Http>,
    tx: mpsc::Sender<web3::types::U64>,
) {
    println!("Initializing block number...");

    let start_block_number = w3.eth().block_number()
        .await
        .expect("Failed to fetch initial block number");

    println!("Starting Block Number={}", start_block_number);

    tx.send(start_block_number).await.expect("Failed to send new block number");
    
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
            tx.send(block_number).await.expect("Failed to send new block number");
            if block_number > start_block_number + 10 {
                break;
            }
        } else {
            println!("Same block: {}", candidate_block_number);
        }
    }
}


async fn retrieve_new_blocks(
    w3: web3::Web3<web3::transports::Http>,
    mut rx: mpsc::Receiver<web3::types::U64>,
) {
    loop {
        let block_number = rx.recv().await.expect("Failed to retrieve new block number");
        println!("Receiver got block: {}", block_number);
    }
}
