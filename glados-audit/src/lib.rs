use std::path::PathBuf;

use tracing::{debug, info};

use sea_orm::{DatabaseConnection, EntityTrait};

use tokio::sync::mpsc;

use ethereum_types::H256;

use glados_core::jsonrpc::PortalClient;
use glados_core::types::{BlockHeaderContentKey, ContentKey};

use entity::contentkey;

pub mod cli;

pub async fn run_glados_audit(conn: DatabaseConnection, ipc_path: PathBuf) {
    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(do_audit_orchestration(tx, conn));
    tokio::spawn(perform_content_audits(rx, ipc_path));

    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

async fn do_audit_orchestration(tx: mpsc::Sender<BlockHeaderContentKey>, conn: DatabaseConnection) {
    debug!("initializing audit process");

    loop {
        // Lookup a content key to be audited
        let content_key_db = contentkey::Entity::find().one(&conn).await.unwrap();
        if let Some(content_key_db) = content_key_db {
            let content_key = BlockHeaderContentKey {
                hash: H256::from_slice(&content_key_db.content_key),
            };

            // Send it to the audit process
            tx.send(content_key).await.unwrap();
        } else {
            debug!("No content found to audit");
        }
    }
}

async fn perform_content_audits(mut rx: mpsc::Receiver<BlockHeaderContentKey>, ipc_path: PathBuf) {
    let mut client = PortalClient::from_ipc(&ipc_path).unwrap();

    while let Some(content_key) = rx.recv().await {
        //let content_key_db = contentkey::Entity::find_by_id(content_key_id).one(&conn).await.unwrap();

        debug!(
            content.key=?content_key.hex_encode(),
            content.id=?content_key.content_id(),
            "auditing content",
        );
        let _content = client.get_content(&content_key);
        info!("success auditing content");
    }
}
