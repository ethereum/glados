use entity::{audit, content, HistorySelectionStrategy, SelectionStrategy};
use ethportal_api::{
    jsonrpsee::http_client::HttpClient, types::portal::GetContentInfo, ContentValue,
    HistoryContentKey, HistoryContentValue, HistoryNetworkApiClient, OverlayContentKey,
};
use glados_core::db::store_history_content_key;
use sea_orm::{DatabaseConnection, EntityTrait};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::{config::AuditConfig, AuditTask};

pub async fn execute_audit_strategy(
    strategy: SelectionStrategy,
    tx: mpsc::Sender<AuditTask>,
    config: AuditConfig,
) {
    match &strategy {
        SelectionStrategy::History(HistorySelectionStrategy::Sync) => {
            execute_sync_strategy(tx, config).await
        }
        SelectionStrategy::History(HistorySelectionStrategy::Random) => {
            execute_random_strategy(tx, config).await
        }
    }
}

/// Creates and sends audit tasks for [HistorySelectionStrategy::Sync].
///
/// It does following steps:
/// 1. Finds the block number of the latest Sync strategy audit
/// 2. Creates audit task for the following block number
/// 3. Keeps going until the merge block, then restarts from genesis
async fn execute_sync_strategy(tx: mpsc::Sender<AuditTask>, config: AuditConfig) -> ! {
    let block_range = config.block_range;
    let conn = config.database_connection;

    let strategy = SelectionStrategy::History(HistorySelectionStrategy::Sync);

    let mut block_number = match latest_audit_block_number(&conn).await {
        Some(block_number) => block_number + 1,
        None => 0,
    };

    let client = &config.portal_clients[0].api.client;

    loop {
        if !block_range.contains(&block_number) {
            block_number = *block_range.start();
        }

        audit_block_number(block_number, &strategy, client, &tx, &conn).await;

        block_number += 1;
    }
}

/// Creates and sends audit tasks for [HistorySelectionStrategy::Random].
///
/// Selects the random block number and sends audit tasks.
async fn execute_random_strategy(tx: mpsc::Sender<AuditTask>, config: AuditConfig) -> ! {
    let block_range = config.block_range;
    let conn = config.database_connection;

    let strategy = SelectionStrategy::History(HistorySelectionStrategy::Random);

    loop {
        let block_number = rand::random_range(block_range.clone());
        audit_block_number(
            block_number,
            &strategy,
            &config.portal_clients[0].api.client,
            &tx,
            &conn,
        )
        .await;
    }
}

async fn audit_block_number(
    block_number: u64,
    strategy: &SelectionStrategy,
    client: &HttpClient,
    tx: &mpsc::Sender<AuditTask>,
    conn: &DatabaseConnection,
) {
    let header_content_key = HistoryContentKey::new_block_header_by_number(block_number);

    let content = match client.get_content(header_content_key.clone()).await {
        Ok(GetContentInfo { content, .. }) => content,
        Err(err) => {
            warn!(block_number, %err, "Enable to get header");
            return;
        }
    };

    let block_hash = match HistoryContentValue::decode(&header_content_key, &content) {
        Ok(HistoryContentValue::BlockHeaderWithProof(header_with_proof)) => {
            header_with_proof.header.hash_slow()
        }
        Ok(content_value) => {
            warn!(
                block_number,
                ?content_value,
                "Wrong content type, expected header"
            );
            return;
        }
        Err(err) => {
            warn!(block_number, %err, "Enable to decode header");
            return;
        }
    };

    audit_content(
        HistoryContentKey::new_block_body(block_hash),
        block_number,
        strategy,
        tx,
        conn,
    )
    .await;

    audit_content(
        HistoryContentKey::new_block_receipts(block_hash),
        block_number,
        strategy,
        tx,
        conn,
    )
    .await;
}

async fn audit_content(
    content_key: HistoryContentKey,
    block_number: u64,
    strategy: &SelectionStrategy,
    tx: &mpsc::Sender<AuditTask>,
    conn: &DatabaseConnection,
) {
    let Some(content) = store_history_content_key(&content_key, block_number, conn).await else {
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
    let audit_task = AuditTask {
        strategy: strategy.clone(),
        content,
    };
    if tx.send(audit_task).await.is_err() {
        panic!("Can't send audit task: Channel is closed");
    };
}

async fn latest_audit_block_number(conn: &DatabaseConnection) -> Option<u64> {
    let Ok(Some(latest_audit)) = audit::get_latest_audit(
        SelectionStrategy::History(HistorySelectionStrategy::Sync),
        conn,
    )
    .await
    else {
        warn!("Latest audit not found!");
        return None;
    };
    match content::Entity::find_by_id(latest_audit.content_id)
        .one(conn)
        .await
    {
        Ok(Some(content)) => content.block_number(),
        _ => {
            warn!(
                audit.id = latest_audit.id,
                content.id = latest_audit.content_id,
                "Content not found for latest audit",
            );
            None
        }
    }
}
