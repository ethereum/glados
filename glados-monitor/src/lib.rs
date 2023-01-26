use std::path::PathBuf;
use std::time::Duration;

use ethereum_types::H256;
use sea_orm::DatabaseConnection;
use tokio::{fs::read_dir, sync::mpsc, time::sleep};
use tracing::{debug, error, info, warn};
use web3::types::BlockId;

use entity::contentkey;
use glados_core::types::{BlockHeaderContentKey, ContentKey, EpochAccumulatorContentKey};
use migration::DbErr;

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

        let Ok(candidate_block_number) = w3.eth().block_number().await else {continue};

        if candidate_block_number <= block_number {
            debug!(head.number=?block_number, "head unchanged");
            continue;
        }
        info!(
            old_head.number=?block_number,
            new_head.number=?candidate_block_number,
            "new head",
        );
        if let Err(e) = tx.send(candidate_block_number).await {
            warn!(head.number=?block_number, err=?e , "Failed to send new block number")
        } else {
            block_number = candidate_block_number
        };
    }
}

async fn retrieve_new_blocks(
    w3: web3::Web3<web3::transports::Http>,
    mut rx: mpsc::Receiver<web3::types::U64>,
    conn: DatabaseConnection,
) {
    loop {
        let Some(block_number_to_retrieve) = rx.recv().await else {continue};
        debug!(block.number=?block_number_to_retrieve, "fetching block");

        let Ok(block) = w3
            .eth()
            .block(BlockId::from(block_number_to_retrieve))
            .await
        else {
            warn!(head.number=?block_number_to_retrieve, "Failed to retrieve block");
            continue
        };

        let Some(blk) = block else {
            error!(
                block.number=?block_number_to_retrieve,
                "failure retrieving block",
            );
            continue
        };
        let Some(block_hash) = blk.hash else {
            error!(head.number=?block_number_to_retrieve, "Fetched block has no hash (skipping)");
            continue
        };

        info!(
            block.hash=?block_hash,
            block.number=?block_number_to_retrieve,
            "received block",
        );

        let hash = H256::from_slice(block_hash.as_bytes());

        let header = BlockHeaderContentKey { hash };
        let body = BlockHeaderContentKey { hash };
        let receipts = BlockHeaderContentKey { hash };

        store_content_key(&header, "block-header", &conn).await;
        store_content_key(&body, "block-body", &conn).await;
        store_content_key(&receipts, "block-receipts", &conn).await;
    }
}

/// Accepts a ContentKey and attempts to store it.
///
/// Errors are logged.
async fn store_content_key<T: ContentKey>(key: &T, name: &str, conn: &DatabaseConnection) {
    debug!(
        content.key=key.hex_encode(),
        content.id=?key.content_id(),
        content.kind=name,
        "Creating content database record",
    );

    if let Err(e) = contentkey::get_or_create(key, conn).await {
        error!(
            content.key=key.hex_encode(),
            content.id=?key.content_id(),
            content.kind=name,
            err=?e,
            "Failed to create database record",
        );
    }
}

pub async fn import_pre_merge_accumulators(
    conn: DatabaseConnection,
    base_path: PathBuf,
) -> Result<(), DbErr> {
    info!(base_path = %base_path.as_path().display(), "Starting import of pre-merge accumulators");

    let mut entries = read_dir(base_path).await.unwrap();

    while let Some(entry) = entries.next_entry().await.unwrap() {
        let path = entry.path();

        debug!(path = path.as_path().to_str(), "Processing path");

        if path.is_file() {
            if let Some(file_stem) = path.file_stem() {
                if let Some(file_stem_str) = file_stem.to_str() {
                    if file_stem_str.len() != 68 {
                        error!(file_stem = file_stem.to_str(), "Filename wrong length");
                        continue;
                    }
                    match &file_stem_str[..2] {
                        "0x" => match hex::decode(&file_stem_str[2..]) {
                            Ok(content_key_raw) => {
                                let content_key = EpochAccumulatorContentKey {
                                    hash: H256::from_slice(&content_key_raw[1..]),
                                };
                                debug!(content_key = %content_key, "Importing");
                                let content_key_db =
                                    contentkey::get_or_create(&content_key, &conn).await?;
                                info!(content_key = %content_key, database_id = content_key_db.id, "Imported");
                            }
                            Err(_) => info!(
                                path = %path.as_path().display(),
                                file_stem = file_stem_str,
                                "Hex decoding error on file"
                            ),
                        },
                        _ => info!(
                            path = %path.as_path().display(),
                            "File name is not 0x prefixed"
                        ),
                    }
                }
            }
        } else {
            info!(
                path = %path.as_path().display(),
                "Skipping non-file path"
            );
        }
    }
    Ok(())
}
