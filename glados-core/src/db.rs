use anyhow::Error;
use chrono::{DateTime, Utc};
use entity::{
    content::{self, SubProtocol},
    execution_metadata,
};
use ethportal_api::{
    utils::bytes::hex_encode, BlockBodyKey, BlockHeaderKey, BlockReceiptsKey, HistoryContentKey,
    OverlayContentKey,
};
use sea_orm::DatabaseConnection;
use tracing::{debug, error};

/// Stores the content keys and block metadata for the given block.
///
/// The metadata included is the block number and hash under the execution
/// header, body and receipts tables.
///
/// Errors are logged.
pub async fn store_block_keys(
    block_number: i32,
    block_hash: &[u8; 32],
    available_at: DateTime<Utc>,
    conn: &DatabaseConnection,
) -> Vec<content::Model> {
    let header = HistoryContentKey::BlockHeaderWithProof(BlockHeaderKey {
        block_hash: *block_hash,
    });
    let body = HistoryContentKey::BlockBody(BlockBodyKey {
        block_hash: *block_hash,
    });
    let receipts = HistoryContentKey::BlockReceipts(BlockReceiptsKey {
        block_hash: *block_hash,
    });

    let header = store_content_key(&header, "block_header", block_number, available_at, conn).await;
    let body = store_content_key(&body, "block_body", block_number, available_at, conn).await;
    let receipts = store_content_key(
        &receipts,
        "block_receipts",
        block_number,
        available_at,
        conn,
    )
    .await;

    let mut returned_values = vec![];
    if let Some(header) = header {
        returned_values.push(header);
    }
    if let Some(body) = body {
        returned_values.push(body);
    }
    if let Some(receipts) = receipts {
        returned_values.push(receipts);
    }
    returned_values
}

/// Accepts a ContentKey from the History and attempts to store it.
///
/// Errors are logged.
pub async fn store_content_key<T: OverlayContentKey>(
    key: &T,
    name: &str,
    block_number: i32,
    available_at: DateTime<Utc>,
    conn: &DatabaseConnection,
) -> Option<content::Model> {
    // Store key
    match content::get_or_create(SubProtocol::History, key, available_at, conn).await {
        Ok(content_model) => {
            log_record_outcome(key, name, DbOutcome::Success);
            // Store metadata
            let metadata_str = format!("{name}_metadata");
            match execution_metadata::get_or_create(content_model.id, block_number, conn).await {
                Ok(_) => log_record_outcome(key, metadata_str.as_str(), DbOutcome::Success),
                Err(e) => log_record_outcome(key, metadata_str.as_str(), DbOutcome::Fail(e)),
            };
            Some(content_model)
        }
        Err(e) => {
            log_record_outcome(key, name, DbOutcome::Fail(e));
            None
        }
    }
}

/// Logs a database record error for the given key.
///
/// Helper function for common error pattern to be logged.
pub fn log_record_outcome<T: OverlayContentKey>(key: &T, name: &str, outcome: DbOutcome) {
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

pub enum DbOutcome {
    Success,
    Fail(Error),
}
