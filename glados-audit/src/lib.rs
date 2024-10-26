use anyhow::Result;
use chrono::Utc;
use cli::Args;
use ethportal_api::{utils::bytes::hex_encode, HistoryContentKey, OverlayContentKey};
use sea_orm::DatabaseConnection;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    thread::available_parallelism,
    vec,
};

use tokio::{
    sync::mpsc::{self, Receiver},
    time::{sleep, Duration},
};
use tracing::{debug, error, info, warn};

use entity::{
    client_info,
    content::{self, SubProtocol},
    content_audit::{
        self, BeaconSelectionStrategy, HistorySelectionStrategy, SelectionStrategy,
        StateSelectionStrategy,
    },
    execution_metadata, node,
};
use glados_core::jsonrpc::PortalClient;

use crate::{
    selection::start_audit_selection_task, state::spawn_state_audit, validation::content_is_valid,
};

pub mod cli;
pub(crate) mod selection;
mod state;
pub mod stats;
pub(crate) mod validation;

/// Configuration created from CLI arguments.
#[derive(Clone, Debug)]
pub struct AuditConfig {
    /// For Glados-related data.
    pub database_url: String,
    /// For getting on-the-fly block information.
    pub provider_url: String,
    /// Audit History
    pub history: bool,
    /// Specific history audit strategies to run.
    pub history_strategies: Vec<HistorySelectionStrategy>,
    /// Audit Beacon
    pub beacon: bool,
    /// Specific beacon audit strategies to run.
    pub beacon_strategies: Vec<BeaconSelectionStrategy>,
    /// Audit State
    pub state: bool,
    /// Specific state audit strategies to run.
    pub state_strategies: Vec<StateSelectionStrategy>,
    /// Weight for each strategy.
    pub weights: HashMap<HistorySelectionStrategy, u8>,
    /// Number requests to a Portal node active at the same time.
    pub concurrency: u8,
    /// Portal Clients
    pub portal_clients: Vec<PortalClient>,
    /// Number of seconds between recording the current audit performance in audit_stats table.
    pub stats_recording_period: u64,
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

        let strategies = match args.history_strategy {
            Some(s) => s,
            None => {
                vec![
                    HistorySelectionStrategy::Latest,
                    HistorySelectionStrategy::Random,
                    HistorySelectionStrategy::Failed,
                    HistorySelectionStrategy::FourFours,
                ]
            }
        };
        let mut weights: HashMap<HistorySelectionStrategy, u8> = HashMap::new();
        for strat in &strategies {
            let weight = match strat {
                HistorySelectionStrategy::Latest => args.latest_strategy_weight,
                HistorySelectionStrategy::Random => args.random_strategy_weight,
                HistorySelectionStrategy::Failed => args.failed_strategy_weight,
                HistorySelectionStrategy::SelectOldestUnaudited => args.oldest_strategy_weight,
                HistorySelectionStrategy::FourFours => args.four_fours_strategy_weight,
                HistorySelectionStrategy::SpecificContentKey => 0,
            };
            weights.insert(strat.clone(), weight);
        }
        if args.provider_url.is_empty()
            && args.history
            && strategies.contains(&HistorySelectionStrategy::FourFours)
        {
            return Err(anyhow::anyhow!(
                "No provider URL provided, required when `four_fours` strategy is enabled."
            ));
        }
        let mut portal_clients: Vec<PortalClient> = vec![];
        for client_url in args.portal_client {
            let client = PortalClient::from(client_url).await?;
            info!("Found a portal client with type: {:?}", client.client_info);
            portal_clients.push(client);
        }
        Ok(AuditConfig {
            database_url: args.database_url,
            provider_url: args.provider_url,
            weights,
            concurrency: args.concurrency,
            portal_clients,
            stats_recording_period: args.stats_recording_period,
            history: args.history,
            history_strategies: strategies,
            beacon: args.beacon,
            beacon_strategies: args.beacon_strategy.unwrap_or_default(),
            state: args.state,
            state_strategies: args.state_strategy.unwrap_or_default(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct AuditTask {
    pub strategy: SelectionStrategy,
    pub content: content::Model,
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
    let content_key =
        HistoryContentKey::try_from_hex(&content_key).expect("needs valid hex-encoded history key");

    let task = AuditTask {
        strategy: SelectionStrategy::History(HistorySelectionStrategy::SpecificContentKey),
        content: content::get_or_create(SubProtocol::History, &content_key, Utc::now(), &conn)
            .await?,
    };
    let client = PortalClient::from(portal_client).await?;
    let active_threads = Arc::new(AtomicU8::new(0));
    perform_single_audit(active_threads, task, client.clone(), conn).await;
    Ok(())
}

pub async fn run_glados_audit(conn: DatabaseConnection, config: AuditConfig) {
    // if state network is enabled, run state audits
    if config.state {
        spawn_state_audit(conn.clone(), config.clone()).await;
    }

    if config.beacon {
        let strategies = config
            .beacon_strategies
            .iter()
            .map(|strats| {
                (
                    SelectionStrategy::Beacon(strats.clone()),
                    /* weight= */ 1,
                )
            })
            .collect();
        start_audit(conn.clone(), config.clone(), strategies).await;
    }

    if config.history {
        let strategies = config
            .history_strategies
            .iter()
            .filter_map(|strats| {
                let strategy = SelectionStrategy::History(strats.clone());
                match config.weights.get(strats) {
                    Some(weight) => Some((strategy, *weight)),
                    None => {
                        error!(strategy=?strategy, "no weight for strategy");
                        None
                    }
                }
            })
            .collect();
        start_audit(conn.clone(), config.clone(), strategies).await;
    }

    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
    std::process::exit(0);
}

async fn start_audit(
    conn: DatabaseConnection,
    config: AuditConfig,
    strategies: Vec<(SelectionStrategy, u8)>,
) {
    let mut task_channels: Vec<TaskChannel> = vec![];
    for (strategy, weight) in strategies {
        // Each strategy sends tasks to a separate channel.
        let (tx, rx) = mpsc::channel::<AuditTask>(100);
        let task_channel = TaskChannel {
            strategy: strategy.clone(),
            weight,
            rx,
        };
        task_channels.push(task_channel);
        // Strategies generate tasks in their own thread for their own channel.
        tokio::spawn(start_audit_selection_task(
            strategy.clone(),
            tx,
            conn.clone(),
            config.clone(),
        ));
    }
    // Collation of generated tasks, taken proportional to weights.
    let (collation_tx, collation_rx) = mpsc::channel::<AuditTask>(100);
    tokio::spawn(start_collation(collation_tx, task_channels));
    // Perform collated audit tasks.
    tokio::spawn(perform_content_audits(config, collation_rx, conn));
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
            // Limit check for new tasks to 5/sec
            sleep(Duration::from_millis(200)).await;
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
        content.key = hex_encode(&task.content.content_key),
        client.url =? client.api.client,
        "auditing content",
    );
    let (content_response, trace) = if client.supports_trace() {
        match client.api.get_content_with_trace(&task.content).await {
            Ok(c) => c,
            Err(e) => {
                error!(
                    content.key=hex_encode(&task.content.content_key),
                    err=?e,
                    "Problem requesting content with trace from Portal node."
                );
                active_threads.fetch_sub(1, Ordering::Relaxed);
                return;
            }
        }
    } else {
        match client.api.get_content(&task.content).await {
            Ok(c) => (c, "".to_owned()),
            Err(e) => {
                error!(
                    content.key=hex_encode(task.content.content_key),
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
        Some(content_bytes) => content_is_valid(&task.content, &content_bytes.raw),
        None => false,
    };

    let client_info_id = match client_info::get_or_create(client_info, &conn).await {
        Ok(client_info) => client_info.id,
        Err(error) => {
            error!(content.key=?task.content,
                err=?error,
                "Could not create/lookup client info in db."
            );
            return;
        }
    };

    let node_id = match node::get_or_create(client.enr.node_id(), &conn).await {
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
        task.content.id,
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
            content.key=?task.content,
            err=?e,
            "Could not create audit entry in db."
        );
        active_threads.fetch_sub(1, Ordering::Relaxed);
        return;
    };

    // Display audit result.
    match task.content.protocol_id {
        SubProtocol::History => {
            display_history_audit_result(task.content, audit_result, &conn).await;
        }
        SubProtocol::Beacon => {
            info!(
                content.key = hex_encode(task.content.content_key),
                audit.pass = audit_result,
                content.protocol = "Beacon",
            );
        }
        SubProtocol::State => {
            info!(
                content.key = hex_encode(task.content.content_key),
                audit.pass = audit_result,
                content.protocol = "State",
            );
        }
    }

    active_threads.fetch_sub(1, Ordering::Relaxed);
}

async fn display_history_audit_result(
    content: content::Model,
    audit_result: bool,
    conn: &DatabaseConnection,
) {
    match execution_metadata::get(content.id, conn).await {
        Ok(Some(b)) => {
            info!(
                content.key=hex_encode(content.content_key),
                audit.pass=?audit_result,
                block = b.block_number,
                "History content audit"
            );
        }
        Ok(None) => {
            info!(
                content.key=hex_encode(content.content_key),
                audit.pass=?audit_result,
                "Block metadata absent for history key."
            );
        }
        Err(e) => error!(
                    content.key=hex_encode(content.content_key),
                    err=?e,
                    "Problem getting block metadata for history key."),
    };
}
