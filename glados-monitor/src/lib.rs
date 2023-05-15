use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Error, Result};
use ethportal_api::types::content_key::{
    BlockBodyKey, BlockHeaderKey, BlockReceiptsKey, EpochAccumulatorKey, HistoryContentKey,
    OverlayContentKey,
};
use sea_orm::DatabaseConnection;
use tokio::{fs::read_dir, sync::mpsc, time::sleep};
use tracing::{debug, error, info, warn};
use trin_utils::bytes::{hex_decode, hex_encode};
use web3::types::{BlockId, H256};

use entity::{content, execution_metadata};

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

/// Listens on a channel, requests blocks from an Execution node and stores derived content keys.
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
        let block_num =
            i32::try_from(block_number_to_retrieve).expect("Block num does not fit in i32.");
        store_block_keys(block_num, block_hash.as_fixed_bytes(), &conn).await;
    }
}

/// Stores the content keys and block metadata for the given block.
///
/// The metadata included is the block number and hash under the execution
/// header, body and receipts tables.
///
/// Errors are logged.
async fn store_block_keys(block_number: i32, block_hash: &[u8; 32], conn: &DatabaseConnection) {
    let header = HistoryContentKey::BlockHeaderWithProof(BlockHeaderKey {
        block_hash: *block_hash,
    });
    let body = HistoryContentKey::BlockBody(BlockBodyKey {
        block_hash: *block_hash,
    });
    let receipts = HistoryContentKey::BlockReceipts(BlockReceiptsKey {
        block_hash: *block_hash,
    });

    store_content_key(&header, "block_header", block_number, conn).await;
    store_content_key(&body, "block_body", block_number, conn).await;
    store_content_key(&receipts, "block_receipts", block_number, conn).await;
}

/// Accepts a ContentKey from the History and attempts to store it.
///
/// Errors are logged.
async fn store_content_key<T: OverlayContentKey>(
    key: &T,
    name: &str,
    block_number: i32,
    conn: &DatabaseConnection,
) {
    // Store key
    match content::get_or_create(key, conn).await {
        Ok(content_model) => {
            log_record_outcome(key, name, DbOutcome::Success);
            // Store metadata
            let metadata_str = format!("{name}_metadata");
            match execution_metadata::get_or_create(content_model.id, block_number, conn).await {
                Ok(_) => log_record_outcome(key, metadata_str.as_str(), DbOutcome::Success),
                Err(e) => log_record_outcome(key, metadata_str.as_str(), DbOutcome::Fail(e)),
            };
        }
        Err(e) => log_record_outcome(key, name, DbOutcome::Fail(e)),
    }
}

/// Logs a database record error for the given key.
///
/// Helper function for common error pattern to be logged.
fn log_record_outcome<T: OverlayContentKey>(key: &T, name: &str, outcome: DbOutcome) {
    match outcome {
        DbOutcome::Success => info!(
            content.key = hex_encode(key.to_bytes()),
            content.kind = name,
            "Imported new record",
        ),
        DbOutcome::Fail(e) => error!(
            content.key=hex_encode(key.to_bytes()),
            content.kind=name,
            err=?e,
            "Failed to create database record",
        ),
    }
}

enum DbOutcome {
    Success,
    Fail(Error),
}

pub async fn import_pre_merge_accumulators(
    conn: DatabaseConnection,
    base_path: PathBuf,
) -> Result<()> {
    info!(base_path = %base_path.as_path().display(), "Starting import of pre-merge accumulators");

    let mut entries = read_dir(base_path).await?;

    while let Some(entry) = entries.next_entry().await? {
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
                        "0x" => match hex_decode(file_stem_str) {
                            Ok(content_key_raw) => {
                                let content_key =
                                    HistoryContentKey::EpochAccumulator(EpochAccumulatorKey {
                                        epoch_hash: H256::from_slice(&content_key_raw[1..]),
                                    });
                                debug!(content_key = %content_key, "Importing");
                                let content_key_db =
                                    content::get_or_create(&content_key, &conn).await?;
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
