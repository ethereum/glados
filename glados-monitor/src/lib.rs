use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Error, Result};
use ethportal_api::types::content_key::{
    BlockBodyKey, BlockHeaderKey, BlockReceiptsKey, EpochAccumulatorKey, HistoryContentKey,
    OverlayContentKey,
};
use ethportal_api::utils::bytes::{hex_decode, hex_encode};
use futures::future::join_all;
use reqwest::header;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection};
use std::env;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio::{fs::read_dir, sync::mpsc, time::sleep};
use tracing::{debug, error, info, warn};
use web3::transports::Http;
use web3::types::{BlockId, H256};
use web3::Web3;

use url::Url;

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

        let block_hash = match fetch_block_hash(block_number_to_retrieve, &w3).await {
            Ok(block_hash) => block_hash,
            Err(e) => {
                error!(block.number=?block_number_to_retrieve, err=?e, "Failed to fetch block");
                continue;
            }
        };

        let block_num =
            i32::try_from(block_number_to_retrieve).expect("Block num does not fit in i32.");
        store_block_keys(block_num, block_hash.as_fixed_bytes(), &conn).await;
    }
}

/// Gets the block hash for the given block number.
async fn fetch_block_hash(
    block_number: web3::types::U64,
    w3: &web3::Web3<web3::transports::Http>,
) -> Result<H256> {
    let block = w3
        .eth()
        .block(BlockId::from(block_number))
        .await
        .map_err(|e| anyhow!("Failed to retrieve block: {}", e))?
        .ok_or_else(|| anyhow!("No block found at {block_number}"))?;

    let block_hash = block
        .hash
        .ok_or_else(|| anyhow!("Fetched block has no hash (skipping)"))?;

    info!(
        block.hash=?block_hash,
        block.number=?block_number,
        "received block",
    );

    Ok(block_hash)
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
        DbOutcome::Success => debug!(
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

/// Bulk download block data from a remote provider.
pub async fn bulk_download_block_data(
    conn: DatabaseConnection,
    start: u64,
    end: u64,
    provider_url: String,
    concurrency: u32,
) -> Result<()> {
    if start > end {
        Err(anyhow!(
            "End block number must be greater than or equal to start block number"
        ))?;
    }
    info!(
        start = start,
        end = end,
        provider_url = provider_url.as_str(),
        "Starting bulk download of block data",
    );

    let w3 = panda_ops_web3(&provider_url)?;

    // On Sqlite, a pool having `concurrency` requests + inserts running at all times is most efficient
    if conn.get_database_backend() == DatabaseBackend::Sqlite {
        let semaphore = Arc::new(Semaphore::new(concurrency as usize));
        for block_number in start..end {
            let w3 = w3.clone();
            let conn = conn.clone();
            let permit = semaphore.clone().acquire_owned().await?;
            tokio::spawn(async move {
                // In case of failure, retry until successful
                let block_hash = loop {
                    match fetch_block_hash(block_number.into(), &w3).await {
                        Ok(block_hash) => break block_hash,
                        Err(err) => {
                            warn!(
                                block_number = block_number,
                                error = err.to_string().as_str(),
                                "Failed to download block"
                            );
                        }
                    };
                    sleep(Duration::from_secs(1)).await;
                };

                let block_number =
                    i32::try_from(block_number).expect("Block num does not fit in i32.");
                store_block_keys(block_number, block_hash.as_fixed_bytes(), &conn).await;
                drop(permit);
            });
        }

        // Wait for all tasks to complete
        let _ = semaphore.clone().acquire_many(concurrency).await?;
    } else {
        // On Postgres, a brief pause in between large amounts of inserts is most efficient.
        // Currently that pause is done while requesting the next batch of block data.
        // An approach that decouples downloading/inserting and sets the rate of insertions
        // based on knowledge of postgres internals could get improved performance.

        // Chunk the block numbers into groups of `concurrency` size
        let range: Vec<u64> = (start..end).collect();
        let chunks = range.chunks(concurrency as usize);

        for chunk in chunks {
            info!("Downloading blocks {}-{}", chunk[0], chunk[chunk.len() - 1]);

            // Request & store all blocks in the chunk concurrently
            let join_handles: Vec<JoinHandle<_>> = chunk
                .iter()
                .map(|block_number| {
                    let w3 = w3.clone();
                    let conn = conn.clone();
                    let block_number = *block_number;
                    tokio::spawn(async move {
                        // In case of failure, retry until successful
                        let block_hash = loop {
                            match fetch_block_hash(block_number.into(), &w3).await {
                                Ok(block_hash) => break block_hash,
                                Err(err) => {
                                    warn!(
                                        block_number = block_number,
                                        error = err.to_string().as_str(),
                                        "Failed to download block"
                                    );
                                }
                            };
                            sleep(Duration::from_secs(1)).await;
                        };

                        let block_number =
                            i32::try_from(block_number).expect("Block num does not fit in i32.");
                        store_block_keys(block_number, block_hash.as_fixed_bytes(), &conn).await;
                    })
                })
                .collect();

            join_all(join_handles).await;
        }
    }
    Ok(())
}

pub fn panda_ops_web3(provider_url: &str) -> Result<Web3<Http>> {
    let mut headers = header::HeaderMap::new();
    let client_id = env::var("PANDAOPS_CLIENT_ID")
        .map_err(|_| anyhow!("PANDAOPS_CLIENT_ID env var not set."))?;
    let client_id = header::HeaderValue::from_str(&client_id);
    let client_secret = env::var("PANDAOPS_CLIENT_SECRET")
        .map_err(|_| anyhow!("PANDAOPS_CLIENT_SECRET env var not set."))?;
    let client_secret = header::HeaderValue::from_str(&client_secret);
    headers.insert("CF-Access-Client-Id", client_id?);
    headers.insert("CF-Access-Client-Secret", client_secret?);

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    let url = Url::parse(provider_url)?;
    let transport = web3::transports::Http::with_client(client, url);
    let w3 = web3::Web3::new(transport);
    Ok(w3)
}
