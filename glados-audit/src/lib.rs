use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use ethportal_api::{types::content_key::OverlayContentKey, HistoryContentKey};
use sea_orm::DatabaseConnection;
use tokio::{
    sync::mpsc,
    time::{sleep, Duration},
};
use tracing::{debug, error, info};

use entity::{
    content,
    content_audit::{self, SelectionStrategy},
    execution_metadata,
};
use glados_core::jsonrpc::PortalClient;

use crate::selection::start_audit_selection_task;

pub mod cli;
pub(crate) mod selection;

#[derive(Clone, Debug)]
pub struct AuditTask {
    pub strategy: SelectionStrategy,
    pub content_key: HistoryContentKey,
}

pub async fn run_glados_audit(conn: DatabaseConnection, ipc_path: PathBuf, max_concurrency: u8) {
    info!(max.concurrency = max_concurrency, "starting glados audit.");
    let (tx, rx) = mpsc::channel::<AuditTask>(100);
    let strategies = vec![
        SelectionStrategy::Latest,
        SelectionStrategy::Random,
        SelectionStrategy::Failed,
        SelectionStrategy::SelectOldestUnaudited,
    ];
    for strategy in strategies {
        tokio::spawn(start_audit_selection_task(
            strategy,
            tx.clone(),
            conn.clone(),
        ));
    }

    tokio::spawn(perform_content_audits(max_concurrency, rx, ipc_path, conn));

    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

async fn perform_content_audits(
    max_concurrency: u8,
    mut rx: mpsc::Receiver<AuditTask>,
    ipc_path: PathBuf,
    conn: DatabaseConnection,
) {
    let active_threads = Arc::new(AtomicU8::new(0));
    loop {
        let active_count = active_threads.load(Ordering::Relaxed);
        if active_count >= max_concurrency {
            // Each audit is performed in new thread if enough concurrency is available.
            debug!(
                active.threads = active_count,
                max.threads = max_concurrency,
                "Max concurrency reached. Sleeping..."
            );
            sleep(Duration::from_millis(1000)).await;
            continue;
        }

        debug!(
            active.threads = active_count,
            max.threads = max_concurrency,
            "Checking Rx channel for audits"
        );

        match rx.recv().await {
            Some(task) => {
                active_threads.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(perform_single_audit(
                    active_threads.clone(),
                    task,
                    ipc_path.clone(),
                    conn.clone(),
                ))
            }
            None => {
                continue;
            }
        };
    }
}

/// Performs an audit against a Portal node.
///
/// After auditing finishes the thread counter is decremented. This
/// applies even if the audit process encounters an error.
async fn perform_single_audit(
    active_threads: Arc<AtomicU8>,
    task: AuditTask,
    ipc_path: PathBuf,
    conn: DatabaseConnection,
) {
    let mut client = match PortalClient::from_ipc(&ipc_path) {
        Ok(c) => c,
        Err(e) => {
            error!(
                content.key=?task.content_key,
                err=?e,
                "Could not connect to Portal node."
            );
            active_threads.fetch_sub(1, Ordering::Relaxed);
            return;
        }
    };

    debug!(content.key = task.content_key.to_hex(), "auditing content",);
    let content = match client.get_content(&task.content_key) {
        Ok(c) => c,
        Err(e) => {
            error!(
                content.key=?task.content_key,
                err=?e,
                "Could not get content from Portal node."
            );
            active_threads.fetch_sub(1, Ordering::Relaxed);
            return;
        }
    };

    let raw_data = content.raw;
    let audit_result = raw_data.len() > 2;
    let content_key_model = match content::get(&task.content_key, &conn).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            error!(
                content.key=?task.content_key,
                audit.pass=?audit_result,
                "Content key not found in db."
            );
            active_threads.fetch_sub(1, Ordering::Relaxed);
            return;
        }
        Err(e) => {
            error!(
                content.key=?task.content_key,
                err=?e,
                "Could not look up content key in db."
            );
            active_threads.fetch_sub(1, Ordering::Relaxed);
            return;
        }
    };
    if let Err(e) =
        content_audit::create(content_key_model.id, audit_result, task.strategy, &conn).await
    {
        error!(
            content.key=?task.content_key,
            err=?e,
            "Could not create audit entry in db."
        );
        active_threads.fetch_sub(1, Ordering::Relaxed);
        return;
    };

    // Display audit result with block metadata.
    match execution_metadata::get(content_key_model.id, &conn).await {
        Ok(Some(b)) => {
            info!(
                content.key=task.content_key.to_hex(),
                audit.pass=?audit_result,
                block = b.block_number,
            );
        }
        Ok(None) => {
            error!(
                content.key=task.content_key.to_hex(),
                audit.pass=?audit_result,
                "Block metadata absent for key."
            );
        }
        Err(e) => error!(
                content.key=task.content_key.to_hex(),
                err=?e,
                "Problem getting block metadata."),
    };
    active_threads.fetch_sub(1, Ordering::Relaxed);
}
