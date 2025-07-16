use chrono::DateTime;
use entity::{
    content::SubProtocol,
    content_audit::{self, HistorySelectionStrategy, SelectionStrategy},
    execution_metadata,
};
use ethportal_api::{HistoryContentKey, OverlayContentKey};
use glados_core::db::store_content_key;
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::AuditTask;

use super::add_to_queue;

/// The first post-merge block number.
pub const MERGE_BLOCK_HEIGHT: u64 = 15537394;

/// Creates and sends audit tasks for [HistorySelectionStrategy::Sync].
///
/// It does following steps:
/// 1. Finds the block number of the latest Sync strategy audit
/// 2. Creates audit task for the following block number
/// 3. Keeps going until the merge block, then restarts from genesis
pub async fn select_sync_content_for_audit(
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
) -> ! {
    let mut block_number = latest_audit_block_number(&conn)
        .await
        .map_or(0, |block_number| block_number + 1);

    loop {
        if tx.is_closed() {
            error!("Channel is closed.");
            panic!();
        }

        if block_number >= MERGE_BLOCK_HEIGHT {
            block_number = 0;
        }

        debug!(block_number, "Creating audit task");

        let content_key = HistoryContentKey::new_block_header_by_number(block_number);

        let mut content_to_audit = vec![];
        if let Some(content) = store_content_key(
            &content_key,
            "block_header_by_number",
            block_number as i32,
            DateTime::UNIX_EPOCH,
            &conn,
            SubProtocol::History,
        )
        .await
        {
            content_to_audit.push(content);
        } else {
            error!(
                content.key = content_key.to_hex(),
                "Unable to store content key"
            );
        };

        add_to_queue(
            tx.clone(),
            SelectionStrategy::History(HistorySelectionStrategy::Sync),
            content_to_audit,
        )
        .await;

        block_number += 1;
    }
}

async fn latest_audit_block_number(conn: &DatabaseConnection) -> Option<u64> {
    let Ok(Some(latest_audit)) = content_audit::get_latest_audit(
        SelectionStrategy::History(HistorySelectionStrategy::Sync),
        conn,
    )
    .await
    else {
        warn!("Latest audit not found!");
        return None;
    };

    let Ok(Some(latest_audit_content_metadata)) =
        execution_metadata::get(latest_audit.content_key, conn).await
    else {
        warn!(
            audit.id = latest_audit.id,
            content.id = latest_audit.content_key,
            "Content metadata not found for audit"
        );
        return None;
    };

    Some(latest_audit_content_metadata.block_number as u64)
}
