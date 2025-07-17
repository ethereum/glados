use chrono::{DateTime, Duration, Utc};
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

/// The first post-merge block number.
const MERGE_BLOCK_HEIGHT: u64 = 15537394;

const GENESIS_TIMESTAMP: DateTime<Utc> = DateTime::from_timestamp(1438269976, 0).unwrap();

pub async fn execute_audit_strategy(
    strategy: SelectionStrategy,
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
) {
    match &strategy {
        SelectionStrategy::History(HistorySelectionStrategy::Sync) => {
            execute_sync_strategy(tx, conn).await
        }
        SelectionStrategy::History(HistorySelectionStrategy::Random) => {
            execute_random_strategy(tx, conn).await
        }
    }
}

/// Creates and sends audit tasks for [HistorySelectionStrategy::Sync].
///
/// It does following steps:
/// 1. Finds the block number of the latest Sync strategy audit
/// 2. Creates audit task for the following block number
/// 3. Keeps going until the merge block, then restarts from genesis
async fn execute_sync_strategy(tx: mpsc::Sender<AuditTask>, conn: DatabaseConnection) -> ! {
    let strategy = SelectionStrategy::History(HistorySelectionStrategy::Sync);

    let mut block_number = latest_audit_block_number(&conn).await.unwrap_or(0);

    loop {
        block_number += 1;
        if block_number >= MERGE_BLOCK_HEIGHT {
            block_number = 0;
        }

        audit_block_number(block_number, strategy.clone(), &tx, &conn).await;
    }
}

/// Creates and sends audit tasks for [HistorySelectionStrategy::Random].
///
/// Selects the random block number and sends audit tasks.
async fn execute_random_strategy(tx: mpsc::Sender<AuditTask>, conn: DatabaseConnection) -> ! {
    let strategy = SelectionStrategy::History(HistorySelectionStrategy::Random);

    loop {
        let block_number = rand::random_range(0..MERGE_BLOCK_HEIGHT);
        audit_block_number(block_number, strategy.clone(), &tx, &conn).await;
    }
}

async fn audit_block_number(
    block_number: u64,
    strategy: SelectionStrategy,
    tx: &mpsc::Sender<AuditTask>,
    conn: &DatabaseConnection,
) {
    let content_key = HistoryContentKey::new_block_header_by_number(block_number);

    let Some(content) = store_content_key(
        &content_key,
        "block_header_by_number",
        block_number as i32,
        GENESIS_TIMESTAMP + Duration::seconds(12 * block_number as i64), // TODO(milos): fix
        conn,
        SubProtocol::History,
    )
    .await
    else {
        error!(
            ?strategy,
            content.key = content_key.to_hex(),
            "Unable to store content key"
        );
        return;
    };

    debug!(
        ?strategy,
        content.key = content_key.to_hex(),
        "Sending audit task"
    );
    let audit_task = AuditTask { strategy, content };
    if tx.send(audit_task).await.is_err() {
        panic!("Can't send audit task: Channel is closed");
    };
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
