use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    thread::available_parallelism,
};

use anyhow::{bail, Result};
use clap::Parser;
use cli::Args;
use ethportal_api::{types::content_key::OverlayContentKey, HistoryContentKey};
use sea_orm::DatabaseConnection;
use tokio::{
    sync::mpsc,
    time::{sleep, Duration},
};
use tracing::{debug, error, info, warn};

use entity::{
    content,
    content_audit::{self, SelectionStrategy},
    execution_metadata,
};
use glados_core::jsonrpc::{PortalClient, TransportConfig};

use crate::{cli::TransportType, selection::start_audit_selection_task};

pub mod cli;
pub(crate) mod selection;

/// Configuration created from CLI arguments.
#[derive(Clone, Debug)]
pub struct AuditConfig {
    /// Maximum amount of threads that audits will be performed with.
    pub concurrency: u8,
    /// For Glados-related data.
    pub database_url: String,
    /// For communication with a Portal Network node.
    pub transport: TransportConfig,
}

impl AuditConfig {
    pub fn from_args() -> Result<AuditConfig> {
        let args = Args::parse();
        let transport: TransportConfig = match args.transport {
            TransportType::IPC => match args.ipc_path {
                Some(p) => TransportConfig::IPC(p),
                None => {
                    bail!("The '--ipc-path' flag is required if '--transport ipc' variant is selected.")
                }
            },
            TransportType::HTTP => match args.http_url {
                Some(h) => TransportConfig::HTTP(h),
                None => {
                    bail!("The '--http-url' flag is required if '--transport http' variant is selected.");
                }
            },
        };
        let parallelism = available_parallelism()?.get() as u8;
        if args.concurrency > parallelism {
            warn!(
                selected.concurrency = args.concurrency,
                system.concurrency = parallelism,
                "Selected concurrency greater than system concurrency."
            )
        } else {
            info!(
                selected.concurrency = args.concurrency,
                system.concurrency = parallelism,
                "Selected concurrency set."
            )
        }
        Ok(AuditConfig {
            concurrency: args.concurrency,
            database_url: args.database_url,
            transport,
        })
    }
}

#[derive(Clone, Debug)]
pub struct AuditTask {
    pub strategy: SelectionStrategy,
    pub content_key: HistoryContentKey,
}

pub async fn run_glados_audit(conn: DatabaseConnection, config: AuditConfig) {
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

    tokio::spawn(perform_content_audits(config, rx, conn.clone()));
    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

async fn perform_content_audits(
    config: AuditConfig,
    mut rx: mpsc::Receiver<AuditTask>,
    conn: DatabaseConnection,
) {
    let active_threads = Arc::new(AtomicU8::new(0));
    loop {
        let active_count = active_threads.load(Ordering::Relaxed);
        if active_count >= config.concurrency {
            // Each audit is performed in new thread if enough concurrency is available.
            debug!(
                active.threads = active_count,
                max.threads = config.concurrency,
                "Waiting for responses on all audit threads... Sleeping..."
            );
            sleep(Duration::from_millis(5000)).await;
            continue;
        }

        debug!(
            active.threads = active_count,
            max.threads = config.concurrency,
            "Checking Rx channel for audits"
        );

        match rx.recv().await {
            Some(task) => {
                active_threads.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(perform_single_audit(
                    active_threads.clone(),
                    task,
                    config.clone(),
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
/// After auditing finishes the thread counter is deprecated. This
/// applies even if the audit process encounters an error.
async fn perform_single_audit(
    active_threads: Arc<AtomicU8>,
    task: AuditTask,
    config: AuditConfig,
    conn: DatabaseConnection,
) {
    let client = match PortalClient::from_config(&config.transport) {
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
    let content = match client.get_content(&task.content_key).await {
        Ok(c) => c,
        Err(e) => {
            error!(
                content.key=?task.content_key.to_hex(),
                err=?e,
                "Problem requesting content from Portal node."
            );
            active_threads.fetch_sub(1, Ordering::Relaxed);
            return;
        }
    };

    let audit_result = content.is_some();
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
