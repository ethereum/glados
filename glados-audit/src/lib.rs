use std::{fmt::Display, path::PathBuf};

use anyhow::Result;
use ethereum_types::H256;
use ethportal_api::types::content_key::{
    BlockBodyKey, BlockHeaderKey, BlockReceiptsKey, HistoryContentKey,
};
use migration::DbErr;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder, QuerySelect};
use tokio::{
    sync::mpsc,
    time::{interval, Duration},
};
use tracing::{debug, error, info};

use entity::{contentaudit, contentkey, executionbody, executionheader, executionreceipts};
use glados_core::jsonrpc::PortalClient;

pub mod cli;

const AUDIT_PERIOD_SECONDS: u64 = 120;

pub async fn run_glados_audit(conn: DatabaseConnection, ipc_path: PathBuf) {
    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(do_audit_orchestration(tx, conn.clone()));
    tokio::spawn(perform_content_audits(rx, ipc_path, conn));

    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

async fn do_audit_orchestration(tx: mpsc::Sender<HistoryContentKey>, conn: DatabaseConnection) -> !
where
    Vec<u8>: From<HistoryContentKey>,
{
    debug!("initializing audit process");

    let mut interval = interval(Duration::from_secs(AUDIT_PERIOD_SECONDS));
    loop {
        interval.tick().await;
        if tx.is_closed() {
            error!("Channel is closed.");
            panic!();
        }
        // Lookup a content key to be audited
        let content_key_db_entries = match contentkey::Entity::find()
            .order_by_desc(contentkey::Column::CreatedAt)
            .limit(10)
            .all(&conn)
            .await
        {
            Ok(content_key_db_entries) => content_key_db_entries,
            Err(err) => {
                error!("DB Error looking up content key: {err}");
                continue;
            }
        };
        debug!(
            "Adding {} content keys to the audit queue.",
            content_key_db_entries.len()
        );
        for content_key_db in content_key_db_entries {
            // Get the block hash (by removing the first byte from the content key)
            let hash = H256::from_slice(&content_key_db.content_key[1..33]);
            let header = HistoryContentKey::BlockHeader(BlockHeaderKey {
                block_hash: hash.to_fixed_bytes(),
            });

            let body = HistoryContentKey::BlockBody(BlockBodyKey {
                block_hash: hash.to_fixed_bytes(),
            });

            let receipts = HistoryContentKey::BlockReceipts(BlockReceiptsKey {
                block_hash: hash.to_fixed_bytes(),
            });

            if let Err(e) = tx.send(header).await {
                debug!(err=?e, "Could not send header for audit, channel might be closed.")
            }
            if let Err(e) = tx.send(body).await {
                debug!(err=?e, "Could not send body for audit, channel might be closed.")
            }
            if let Err(e) = tx.send(receipts).await {
                debug!(err=?e, "Could not send receipts for audit, channel might be closed.")
            }
        }
    }
}

async fn perform_content_audits(
    mut rx: mpsc::Receiver<HistoryContentKey>,
    ipc_path: PathBuf,
    conn: DatabaseConnection,
) -> Result<()>
where
    Vec<u8>: From<HistoryContentKey>,
{
    let mut client = PortalClient::from_ipc(&ipc_path).expect("Could not connect to portal node.");

    while let Some(content_key) = rx.recv().await {
        let content_key_str = format!("0x{}", hex::encode::<Vec<u8>>(content_key.clone().into()));
        debug!(content.key = content_key_str, "auditing content",);
        let content = client.get_content(&content_key)?;

        let raw_data = content.raw;
        let audit_result = raw_data.len() > 2;
        let content_key_model = match contentkey::get(&content_key, &conn).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                error!(
                    content.key=?content_key,
                    audit.pass=?audit_result,
                    "Content_key not found in db."
                );
                continue;
            }
            Err(e) => {
                error!(
                    content.key=?content_key,
                    err=?e,
                    "Could not look up content_key in db."
                );
                continue;
            }
        };
        contentaudit::create(content_key_model.id, audit_result, &conn).await?;

        // Display audit result with block metadata.
        match fetch_block_metadata(content_key_model.id, &conn).await {
            Ok(Some(b)) => {
                debug!(
                    content.key=content_key_str,
                    audit.pass=?audit_result,
                    block = b.to_string(),
                );
            }
            Ok(None) => {
                error!(
                    content.key=?content_key_str,
                    audit.pass=?audit_result,
                    "Block metadata absent for key."
                );
            }
            Err(e) => error!(
                    content.key=?content_key_str,
                    err=?e,
                    "Problem getting block metadata."),
        };
    }
    Ok(())
}

struct BlockMetadata {
    component: Component,
    number: i32,
    hash: Vec<u8>,
}

impl Display for BlockMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let comp = match self.component {
            Component::Header => "header",
            Component::Body => "body",
            Component::Receipts => "receipts",
        };
        let number = self.number;
        let hash = H256::from_slice(&self.hash).to_string();
        write!(f, "{comp} for block {number} (hash: {hash})")
    }
}

enum Component {
    Header,
    Body,
    Receipts,
}

/// Gets execution body, header or receipts details for a
/// single key.
///
/// The key is the database-assigned key for a portal content_key.
/// Search stops as soon as a match is found, otherwise returns None.
async fn fetch_block_metadata(
    content_key_model_id: i32,
    conn: &DatabaseConnection,
) -> Result<Option<BlockMetadata>, DbErr> {
    if let Some(header) = executionheader::get(content_key_model_id, conn).await? {
        return Ok(Some(BlockMetadata {
            component: Component::Header,
            number: header.block_number,
            hash: header.block_hash,
        }));
    }
    if let Some(body) = executionbody::get(content_key_model_id, conn).await? {
        return Ok(Some(BlockMetadata {
            component: Component::Body,
            number: body.block_number,
            hash: body.block_hash,
        }));
    }
    if let Some(receipts) = executionreceipts::get(content_key_model_id, conn).await? {
        return Ok(Some(BlockMetadata {
            component: Component::Receipts,
            number: receipts.block_number,
            hash: receipts.block_hash,
        }));
    }
    Ok(None)
}
