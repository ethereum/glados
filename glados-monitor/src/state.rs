use std::time::Duration;

use alloy_primitives::B256;
use anyhow::{anyhow, Result};
use chrono::{DateTime, TimeZone, Utc};
use futures::future::join_all;
use glados_core::db::store_state_root;
use sea_orm::DatabaseConnection;
use tokio::{sync::mpsc, task::JoinHandle, time::sleep};
use tracing::{debug, error, info, warn};
use web3::types::BlockId;

use crate::{follow_chain_head, panda_ops_web3};

pub async fn follow_head_state_command(
    conn: DatabaseConnection,
    provider_url: String,
) -> Result<()> {
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

    run_glados_monitor_state(conn, w3).await;
    Ok(())
}

/// Bulk download block data from a remote provider.
pub async fn populate_state_roots_range_command(
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

    // On Postgres, a brief pause in between large amounts of inserts is most efficient.
    // Currently that pause is done while requesting the next batch of block data.
    // An approach that decouples downloading/inserting and sets the rate of insertions
    // based on knowledge of postgres internals could get improved performance.

    // Chunk the block numbers into groups of `concurrency` size
    let range: Vec<u64> = (start..end).collect();
    let chunks = range.chunks(concurrency as usize);

    for chunk in chunks {
        info!(
            "Downloading state roots from block {}-{}",
            chunk[0],
            chunk[chunk.len() - 1]
        );

        // Request & store all blocks in the chunk concurrently
        let join_handles: Vec<JoinHandle<_>> = chunk
            .iter()
            .map(|block_number| {
                let w3 = w3.clone();
                let conn = conn.clone();
                let block_number = *block_number;
                tokio::spawn(async move {
                    // In case of failure, retry until successful
                    let (state_root, block_time) = loop {
                        match fetch_state_root(block_number.into(), &w3).await {
                            Ok(state_root) => break state_root,
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
                    if let Err(err) =
                        store_state_root(block_number, state_root.0.to_vec(), block_time, &conn)
                            .await
                    {
                        warn!(
                            error = ?err,
                            block_number = block_number,
                            "Failed to store state root"
                        );
                    }
                })
            })
            .collect();

        join_all(join_handles).await;
    }
    Ok(())
}

/// Gets the state root and timestamp for the given block number.
async fn fetch_state_root(
    block_number: web3::types::U64,
    w3: &web3::Web3<web3::transports::Http>,
) -> Result<(B256, DateTime<Utc>)> {
    let block = w3
        .eth()
        .block(BlockId::from(block_number))
        .await
        .map_err(|e| anyhow!("Failed to retrieve block: {}", e))?
        .ok_or_else(|| anyhow!("No block found at {block_number}"))?;

    let state_root = block.state_root;

    info!(
        block.state_root=?state_root,
        block.number=?block_number,
        block.timestamp=?block.timestamp,
        "received block",
    );

    let timestamp = block.timestamp.as_u64() as i64;
    let block_timestamp = match Utc.timestamp_opt(timestamp, 0) {
        chrono::LocalResult::Single(time) => time,
        _ => {
            return Err(anyhow!(
                "Failed to convert block timestamp to Utc: {}",
                timestamp
            ))
        }
    };

    Ok((state_root.0.into(), block_timestamp))
}

pub async fn run_glados_monitor_state(
    conn: DatabaseConnection,
    w3: web3::Web3<web3::transports::Http>,
) {
    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(follow_chain_head(w3.clone(), tx));
    tokio::spawn(retrieve_new_state_roots(w3.clone(), rx, conn));

    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

/// Listens on a channel, requests blocks from an Execution node and stores derived content keys.
async fn retrieve_new_state_roots(
    w3: web3::Web3<web3::transports::Http>,
    mut rx: mpsc::Receiver<web3::types::U64>,
    conn: DatabaseConnection,
) {
    loop {
        let Some(block_number_to_retrieve) = rx.recv().await else {
            continue;
        };
        debug!(block.number=?block_number_to_retrieve, "fetching block");

        let (state_root, block_time) = match fetch_state_root(block_number_to_retrieve, &w3).await {
            Ok(state_root) => state_root,
            Err(e) => {
                error!(block.number=?block_number_to_retrieve, err=?e, "Failed to fetch block");
                continue;
            }
        };

        let block_num =
            i32::try_from(block_number_to_retrieve).expect("Block num does not fit in i32.");
        if let Err(err) =
            store_state_root(block_num, state_root.0.to_vec(), block_time, &conn).await
        {
            warn!(
                error = ?err,
                block_number = block_num,
                "Failed to store state root"
            );
        }
    }
}
