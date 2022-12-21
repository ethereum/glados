use std::path::PathBuf;

use ethereum_types::H256;
use tracing::{debug, error, info};

use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder, QuerySelect};

use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

use glados_core::jsonrpc::PortalClient;
use glados_core::types::{BlockHeaderContentKey, ContentKey};

use entity::{contentaudit, contentkey};

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

async fn do_audit_orchestration(
    tx: mpsc::Sender<BlockHeaderContentKey>,
    conn: DatabaseConnection,
) -> ! {
    debug!("initializing audit process");

    let mut interval = interval(Duration::from_secs(AUDIT_PERIOD_SECONDS));
    loop {
        interval.tick().await;

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
            info!("Content Key: {:?}", content_key_db.content_key);
            // Get the block hash (by removing the first byte from the content key)
            let hash = H256::from_slice(&content_key_db.content_key[1..33]);
            let content_key = BlockHeaderContentKey { hash };

            // Send it to the audit process
            tx.send(content_key)
                .await
                .expect("Channel closed, perform_content_audits task likely crashed");
        }
    }
}

async fn perform_content_audits(
    mut rx: mpsc::Receiver<BlockHeaderContentKey>,
    ipc_path: PathBuf,
    conn: DatabaseConnection,
) {
    let mut client = PortalClient::from_ipc(&ipc_path).unwrap();

    while let Some(content_key) = rx.recv().await {
        debug!(
            content.key=?content_key.hex_encode(),
            content.id=?content_key.content_id(),
            "auditing content",
        );
        let (content, trace) = client.get_content_with_trace(&content_key);

        let raw_data = content.raw;
        let content_key_id = contentkey::get(&content_key, &conn).await.unwrap().id;
        contentaudit::create(content_key_id, raw_data.len() > 2, &conn, trace).await;

        info!("Successfully audited content.");
    }
}
