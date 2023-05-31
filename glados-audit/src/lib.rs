use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    thread::available_parallelism,
};

use anyhow::Result;
use cli::Args;
use ethportal_api::types::content_key::{HistoryContentKey, OverlayContentKey};
use ethportal_api::utils::bytes::{hex_decode, hex_encode};
use sea_orm::DatabaseConnection;
use tokio::{
    sync::mpsc::{self, Receiver},
    time::{sleep, Duration},
};
use tracing::{debug, error, info, warn};

use entity::{
    client_info, content,
    content_audit::{self, SelectionStrategy},
    execution_metadata, node,
};
use glados_core::jsonrpc::PortalClient;

use crate::{selection::start_audit_selection_task, validation::content_is_valid};

pub mod cli;
pub(crate) mod selection;
pub(crate) mod validation;

/// Configuration created from CLI arguments.
#[derive(Clone, Debug)]
pub struct AuditConfig {
    /// For Glados-related data.
    pub database_url: String,
    /// Specific strategies to run.
    pub strategies: Vec<SelectionStrategy>,
    /// Weight for each strategy.
    pub weights: HashMap<SelectionStrategy, u8>,
    /// Number requests to a Portal node active at the same time.
    pub concurrency: u8,
    /// Portal Clients
    pub portal_clients: Vec<PortalClient>,
}

impl AuditConfig {
    pub async fn from_args(args: Args) -> Result<AuditConfig> {
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
                SelectionStrategy::SpecificContentKey => 0,
            };
            weights.insert(strat.clone(), weight);
        }

        let mut portal_clients: Vec<PortalClient> = vec![];
        for client_url in args.portal_client {
            let client = PortalClient::from(client_url).await?;
            info!("Found a portal client with type: {:?}", client.client_info);
            portal_clients.push(client);
        }
        Ok(AuditConfig {
            database_url: args.database_url,
            strategies,
            weights,
            concurrency: args.concurrency,
            portal_clients,
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
pub struct TaskChannel {
    strategy: SelectionStrategy,
    weight: u8,
    rx: Receiver<AuditTask>,
}

pub async fn run_glados_command(conn: DatabaseConnection, command: cli::Command) -> Result<()> {
    let (content_key, portal_client) = match command {
        cli::Command::Audit {
            content_key,
            portal_client,
            ..
        } => (content_key, portal_client),
    };
    let content_key = hex_decode(&content_key).unwrap();
    let content_key = HistoryContentKey::try_from(content_key).unwrap();

    let task = AuditTask {
        strategy: SelectionStrategy::SpecificContentKey,
        content_key,
    };
    let client = PortalClient::from(portal_client).await?;
    let active_threads = Arc::new(AtomicU8::new(0));
    perform_single_audit(active_threads, task, client.clone(), conn).await;
    Ok(())
}

pub async fn run_glados_audit(conn: DatabaseConnection, config: AuditConfig) {
    let mut task_channels: Vec<TaskChannel> = vec![];
    for strategy in &config.strategies {
        // Each strategy sends tasks to a separate channel.
        let (tx, rx) = mpsc::channel::<AuditTask>(100);
        let Some(weight) = config.weights.get(strategy) else {
            error!(strategy=?strategy, "no weight for strategy");
            return
        };
        let task_channel = TaskChannel {
            strategy: strategy.clone(),
            weight: *weight,
            rx,
        };
        task_channels.push(task_channel);
        // Strategies generate tasks in their own thread for their own channel.
        tokio::spawn(start_audit_selection_task(
            strategy.clone(),
            tx,
            conn.clone(),
        ));
    }
    // Collation of generated tasks, taken proportional to weights.
    let (collation_tx, collation_rx) = mpsc::channel::<AuditTask>(100);
    tokio::spawn(start_collation(collation_tx, task_channels));
    // Perform collated audit tasks.
    tokio::spawn(perform_content_audits(config, collation_rx, conn));
    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

/// Listens to tasks coming on different strategy channels and selects
/// according to strategy weight. Collated audit tasks are sent in a single
/// channel for completion.
async fn start_collation(
    collation_tx: mpsc::Sender<AuditTask>,
    mut task_channels: Vec<TaskChannel>,
) {
    loop {
        for tasks in task_channels.iter_mut() {
            debug!(strategy=?tasks.strategy, max=tasks.weight, "collating");
            for _ in 0..tasks.weight {
                match tasks.rx.try_recv() {
                    Ok(task) => collation_tx
                        .send(task)
                        .await
                        .expect("Unable to collate task"),
                    Err(_) => break,
                }
            }
        }
    }
}

async fn perform_content_audits(
    config: AuditConfig,
    mut rx: mpsc::Receiver<AuditTask>,
    conn: DatabaseConnection,
) {
    let concurrency = config.concurrency;
    let active_threads = Arc::new(AtomicU8::new(0));

    let mut cycle_of_clients = config.portal_clients.iter().cycle();

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
                let client = match cycle_of_clients.next() {
                    Some(client) => client,
                    None => {
                        error!("Empty list of clients for audit.");
                        return;
                    }
                };
                tokio::spawn(perform_single_audit(
                    active_threads.clone(),
                    task,
                    client.clone(),
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
    client: PortalClient,
    conn: DatabaseConnection,
) {
    let client_info = client.client_info.clone();

    debug!(
        content.key = hex_encode(task.content_key.to_bytes()),
        client.url = client.api.client_url.clone(),
        "auditing content",
    );
    let (content_response, trace) = if client.clone().is_trin() {
        match client.api.get_content_with_trace(&task.content_key).await {
            Ok(c) => c,
            Err(e) => {
                error!(
                    content.key=hex_encode(task.content_key.to_bytes()),
                    err=?e,
                    "Problem requesting content with trace from Portal node."
                );
                active_threads.fetch_sub(1, Ordering::Relaxed);
                return;
            }
        }
    } else {
        match client.api.get_content(&task.content_key).await {
            Ok(c) => (c, "".to_owned()),
            Err(e) => {
                error!(
                    content.key=hex_encode(task.content_key.to_bytes()),
                    err=?e,
                    "Problem requesting content from Portal node."
                );
                active_threads.fetch_sub(1, Ordering::Relaxed);
                return;
            }
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

    let client_info_id = match client_info::get_or_create(client_info, &conn).await {
        Ok(client_info) => client_info.id,
        Err(error) => {
            error!(content.key=?task.content_key,
                err=?error,
                "Could not create/lookup client info in db."
            );
            return;
        }
    };

    let node_id = match node::get_or_create(client.enr.node_id().into(), &conn).await {
        Ok(enr) => enr.id,
        Err(err) => {
            error!(
                err=?err,
                "Failed to created node."
            );
            return;
        }
    };
    if let Err(e) = content_audit::create(
        content_key_model.id,
        client_info_id,
        node_id,
        audit_result,
        task.strategy,
        trace,
        &conn,
    )
    .await
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
                content.key=hex_encode(task.content_key.to_bytes()),
                audit.pass=?audit_result,
                block = b.block_number,
            );
        }
        Ok(None) => {
            error!(
                content.key=hex_encode(task.content_key.to_bytes()),
                audit.pass=?audit_result,
                "Block metadata absent for key."
            );
        }
        Err(e) => error!(
                content.key=hex_encode(task.content_key.to_bytes()),
                err=?e,
                "Problem getting block metadata."),
    };
    active_threads.fetch_sub(1, Ordering::Relaxed);
}
