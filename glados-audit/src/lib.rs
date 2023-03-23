use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use anyhow::{bail, Result};
use clap::Parser;
use cli::Args;
use ethportal_api::{HistoryContentKey, OverlayContentKey};
use sea_orm::DatabaseConnection;
use tokio::{
    sync::mpsc::{self, Receiver},
    time::{sleep, Duration},
};
use tracing::{debug, error, info, warn};

use entity::{
    content,
    content_audit::{self, SelectionStrategy},
    execution_metadata,
};
use glados_core::jsonrpc::{PortalClient, TransportConfig};

use crate::{
    cli::TransportType, selection::start_audit_selection_task, validation::content_is_valid,
};

pub mod cli;
pub(crate) mod selection;
pub(crate) mod validation;

/// Configuration created from CLI arguments.
#[derive(Clone, Debug)]
pub struct AuditConfig {
    /// For Glados-related data.
    pub database_url: String,
    /// For communication with a Portal Network node.
    pub transport: TransportConfig,
    /// Specific strategies to run.
    pub strategies: Vec<SelectionStrategy>,
    /// Weight for each strategy.
    pub weights: HashMap<SelectionStrategy, u8>,
    /// Number requests to a Portal node active at the same time.
    pub concurrency: u8,
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
        let strategies = match args.strategy {
            Some(s) => s,
            None => {
                vec![
                    SelectionStrategy::Latest,
                    SelectionStrategy::Random,
                    SelectionStrategy::Failed,
                    SelectionStrategy::SelectOldestUnaudited,
                ]
            }
        };
        let mut weights: HashMap<SelectionStrategy, u8> = HashMap::new();
        for strat in &strategies {
            let weight = match strat {
                SelectionStrategy::Latest => args.latest_strategy_weight,
                SelectionStrategy::Random => args.random_strategy_weight,
                SelectionStrategy::Failed => args.failed_strategy_weight,
                SelectionStrategy::SelectOldestUnaudited => args.oldest_strategy_weight,
            };
            weights.insert(strat.clone(), weight);
        }
        Ok(AuditConfig {
            database_url: args.database_url,
            transport,
            strategies,
            weights,
            concurrency: args.concurrency,
        })
    }
}

#[derive(Clone, Debug)]
pub struct AuditTask {
    pub strategy: SelectionStrategy,
    pub content_key: HistoryContentKey,
}

// Associates strategies with their channels and weights.
#[derive(Debug)]
pub struct TaskChannels {
    strategy: SelectionStrategy,
    weight: u8,
    channel_recv: Receiver<AuditTask>,
}

pub async fn run_glados_audit(conn: DatabaseConnection, config: AuditConfig) {
    let mut task_channels: Vec<TaskChannels> = vec![];
    let mut total_weight: u8 = 0;
    for strategy in config.strategies {
        // Each strategy sends tasks to a separate channel.
        let (tx, rx) = mpsc::channel::<AuditTask>(100);
        let Some(weight) = config.weights.get(&strategy) else {
            warn!(strategy=?strategy, "no weight for strategy");
            return
        };
        total_weight += weight;
        let task_channel = TaskChannels {
            strategy: strategy.clone(),
            weight: *weight,
            channel_recv: rx,
        };
        task_channels.push(task_channel);
        // Strategies generate tasks in their own thread for their own channel.
        tokio::spawn(start_audit_selection_task(
            strategy.clone(),
            tx.clone(),
            conn.clone(),
        ));
    }
    // Collation of generated tasks, taken proportional to weights.
    let (collation_tx, collation_rx) = mpsc::channel::<AuditTask>(100);
    tokio::spawn(start_collation(collation_tx, task_channels, total_weight));
    // Perform collated audit tasks.
    tokio::spawn(perform_content_audits(
        config.transport,
        config.concurrency,
        collation_rx,
        conn,
    ));
    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

/// Listens to tasks coming on different strategy channels and selects
/// according to strategy weight. Collated audit tasks are sent in a single
/// channel for completion.
///
/// ### Weighting algorithm
/// 1. Start loop and check how much capacity the rx side has.
/// 2. Divide available capacity using weight for each strategy to get quota.
/// 3. For each strategy, move generated tasks to collation channel as per quota.
/// 4. Incomplete quotas are capacity left for the next loop.
async fn start_collation(
    collation_tx: mpsc::Sender<AuditTask>,
    mut task_channels: Vec<TaskChannels>,
    total_weight: u8,
) {
    loop {
        let cap = collation_tx.capacity() as u8;
        for tasks in task_channels.iter_mut() {
            let quota = tasks.weight * cap / total_weight;
            debug!(strategy=?tasks.strategy, quota=quota, "collating strategies");
            for _ in 0..quota {
                let Some(task) = tasks.channel_recv.recv().await else {continue};
                if let Err(err) = collation_tx.send(task).await {
                    error!(err=?err, strategy=?tasks.strategy, "could not move task for collation")
                }
            }
        }
        sleep(Duration::from_millis(5000)).await;
    }
}

async fn perform_content_audits(
    transport: TransportConfig,
    concurrency: u8,
    mut rx: mpsc::Receiver<AuditTask>,
    conn: DatabaseConnection,
) {
    let active_threads = Arc::new(AtomicU8::new(0));
    loop {
        let active_count = active_threads.load(Ordering::Relaxed);
        if active_count >= concurrency {
            // Each audit is performed in new thread if enough concurrency is available.
            debug!(
                active.threads = active_count,
                max.threads = concurrency,
                "Waiting for responses on all audit threads... Sleeping..."
            );
            sleep(Duration::from_millis(5000)).await;
            continue;
        }

        debug!(
            active.threads = active_count,
            max.threads = concurrency,
            "Checking Rx channel for audits"
        );

        match rx.recv().await {
            Some(task) => {
                active_threads.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(perform_single_audit(
                    active_threads.clone(),
                    task,
                    transport.clone(),
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
    transport: TransportConfig,
    conn: DatabaseConnection,
) {
    let client = match PortalClient::from_config(&transport) {
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
    let content_response = match client.get_content(&task.content_key).await {
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

    // If content was absent audit result is 'fail'.
    let audit_result = match content_response {
        Some(content_bytes) => content_is_valid(&task.content_key, &content_bytes.raw),
        None => false,
    };

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
