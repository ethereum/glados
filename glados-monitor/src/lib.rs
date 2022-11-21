use std::time::Duration;

use tracing::{debug, error, info};

use sea_orm::{ActiveModelTrait, DatabaseConnection, NotSet, Set};

use tokio::sync::mpsc;
use tokio::time::sleep;

use web3::types::BlockId;

use ethereum_types::H256;

use glados_core::types::BlockHeaderContentKey;

use entity::contentid;
use entity::contentkey;

pub mod cli;

pub async fn run_glados_monitor(conn: DatabaseConnection, w3: web3::Web3<web3::transports::Http>) {
    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(follow_chain_head(w3.clone(), tx));
    tokio::spawn(retrieve_new_blocks(w3.clone(), rx, conn));

    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

async fn follow_chain_head(
    w3: web3::Web3<web3::transports::Http>,
    tx: mpsc::Sender<web3::types::U64>,
) {
    debug!("initializing head block number");

    let start_block_number = w3
        .eth()
        .block_number()
        .await
        .expect("Failed to fetch initial block number");

    info!(head_block.number=?start_block_number, "following chain head");

    tx.send(start_block_number)
        .await
        .expect("Failed to send new block number");

    // TODO: long running process that fetches latest block...
    let mut block_number = start_block_number;

    loop {
        debug!("sleeping....");
        sleep(Duration::from_secs(5)).await;

        debug!(head.number=?block_number, "checking for new block");

        let candidate_block_number = w3.eth().block_number().await.unwrap();

        if candidate_block_number > block_number {
            info!(
                old_head.number=?block_number,
                new_head.number=?candidate_block_number,
                "new head",
            );
            block_number = candidate_block_number;
            tx.send(block_number)
                .await
                .expect("Failed to send new block number");
            if block_number > start_block_number + 2 {
                break;
            }
        } else {
            debug!(head.number=?block_number, "head unchanged");
        }
    }
}

async fn retrieve_new_blocks(
    w3: web3::Web3<web3::transports::Http>,
    mut rx: mpsc::Receiver<web3::types::U64>,
    conn: DatabaseConnection,
) {
    while let Some(block_number_to_retrieve) = rx.recv().await {
        debug!(block.number=?block_number_to_retrieve, "fetching block");

        let block = w3
            .eth()
            .block(BlockId::from(block_number_to_retrieve))
            .await
            .expect("failed to retrieve block");

        // If we got a block back
        if let Some(blk) = block {
            // And if that block has a hash
            if let Some(block_hash) = blk.hash {
                info!(
                    block.hash=?block_hash,
                    block.number=?block_number_to_retrieve,
                    "received block",
                );
                let raw_content_key = BlockHeaderContentKey {
                    hash: H256::from_slice(block_hash.as_bytes()),
                };
                let raw_content_id = raw_content_key.content_id();

                debug!(
                    content.key=hex::encode(raw_content_key.encoded()),
                    content.id=?raw_content_id,
                    content.kind="block-header",
                    "block header content",
                );

                // TODO: check if record exists
                let content_id = contentid::ActiveModel {
                    id: NotSet,
                    content_id: Set(raw_content_id.as_bytes().to_vec()),
                };

                let content_id = content_id.insert(&conn).await.unwrap();
                debug!(content_id.id = content_id.id, "inserted content_id");

                let encoded_content_key = raw_content_key.encoded();

                let content_key = contentkey::ActiveModel {
                    id: NotSet,
                    content_id: Set(content_id.id),
                    content_key: Set(encoded_content_key),
                };

                let content_key = content_key.insert(&conn).await.unwrap();
                debug!(content_key.id = content_key.id, "inserted content_key")
            }
        } else {
            error!(
                block.number=?block_number_to_retrieve,
                "failure retrieving block",
            );
        }
    }
}
