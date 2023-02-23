use std::path::PathBuf;

use anyhow::Result;
use ethportal_api::{types::content_key::OverlayContentKey, HistoryContentKey};
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use entity::{content, content_audit, execution_metadata};
use glados_core::jsonrpc::PortalClient;

use crate::selection::SelectionStrategy;

pub mod cli;
pub(crate) mod selection;

pub async fn run_glados_audit(conn: DatabaseConnection, ipc_path: PathBuf) {
    let (tx, rx) = mpsc::channel::<HistoryContentKey>(100);
    let strategies = vec![
        SelectionStrategy::Latest,
        SelectionStrategy::Random,
        SelectionStrategy::Failed,
        SelectionStrategy::OldestMissing,
    ];
    for strategy in strategies {
        tokio::spawn(strategy.start_audit_selection_task(tx.clone(), conn.clone()));
    }

    tokio::spawn(perform_content_audits(rx, ipc_path, conn));

    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

async fn perform_content_audits<T>(
    mut rx: mpsc::Receiver<T>,
    ipc_path: PathBuf,
    conn: DatabaseConnection,
) -> Result<()>
where
    T: OverlayContentKey,
{
    let mut client = PortalClient::from_ipc(&ipc_path).expect("Could not connect to portal node.");

    while let Some(content_key) = rx.recv().await {
        let content_key_str = format!("0x{}", hex::encode(content_key.to_bytes()));
        debug!(content.key = content_key_str, "auditing content",);
        let content = client.get_content(&content_key)?;

        let raw_data = content.raw;
        let audit_result = raw_data.len() > 2;
        let content_key_model = match content::get(&content_key, &conn).await {
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
        content_audit::create(content_key_model.id, audit_result, &conn).await?;

        // Display audit result with block metadata.
        match execution_metadata::get(content_key_model.id, &conn).await {
            Ok(Some(b)) => {
                info!(
                    content.key=content_key_str,
                    audit.pass=?audit_result,
                    block = b.block_number,
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
