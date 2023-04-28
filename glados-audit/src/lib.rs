use std::{
    collections::HashMap,
    env,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, bail};
use clap::Parser;
use cli::Args;
use entity::{
    content,
    content_audit::{self, SelectionStrategy},
    execution_metadata,
};
use ethportal_api::{HistoryContentKey, OverlayContentKey};
use glados_core::jsonrpc::{PortalClient, TransportConfig};
use reqwest::header::{HeaderMap, HeaderValue};
use sea_orm::DatabaseConnection;
use tokio::{
    sync::mpsc::{self, Receiver},
    time::{sleep, Duration},
};
use tracing::{debug, error, info};
use url::Url;
use validation::Provider;
use web3::{transports::Http, Web3};

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
    /// An Ethereum execution node to validate content received from
    /// the Portal node against.
    pub trusted_provider: Provider,
}

impl AuditConfig {
    pub fn from_args() -> anyhow::Result<AuditConfig> {
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
        let trusted_provider: Provider = match args.trusted_provider {
            cli::TrustedProvider::HTTP => {
                match args.provider_http_url{
                    Some(url) => {
                        let transport = Http::new(url.as_str())?;
                        let w3 = Web3::new(transport);
                        Provider::Http(w3)
                    },
                    None => bail!("The '--provider-http-url' flag is required if 'http' is selected for the '--trusted-provider'"),
                }
            },
            cli::TrustedProvider::Pandaops => {
                match args.provider_pandaops {
                    Some(provider_url) => {
                        let mut headers = HeaderMap::new();
                        let client_id = env::var("PANDAOPS_CLIENT_ID")
                            .map_err(|_| anyhow!("PANDAOPS_CLIENT_ID env var not set."))?;
                        let client_id = HeaderValue::from_str(&client_id);
                        let client_secret = env::var("PANDAOPS_CLIENT_SECRET")
                            .map_err(|_| anyhow!("PANDAOPS_CLIENT_SECRET env var not set."))?;
                        let client_secret = HeaderValue::from_str(&client_secret);
                        headers.insert("CF-Access-Client-Id", client_id?);
                        headers.insert("CF-Access-Client-Secret", client_secret?);

                        let client = reqwest::Client::builder()
                            .default_headers(headers)
                            .build()?;
                        let url = Url::parse(&provider_url)?;
                        let transport = Http::with_client(client, url);
                        let w3 = Web3::new(transport);
                        Provider::Pandaops(w3)
                    },
                    None => bail!("The '--provider-pandaops' flag is required if 'pandaops' is selected for the '--trusted-provider'"),
                }
            }
        }
        ;

        Ok(AuditConfig {
            database_url: args.database_url,
            transport,
            strategies,
            weights,
            concurrency: args.concurrency,
            trusted_provider,
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

    // If content was absent or invalid the audit result is 'fail'.
    let audit_result = match content_response {
        Some(content_bytes) => {
            match content_is_valid(
                &config.trusted_provider,
                &task.content_key,
                &content_bytes.raw,
            )
            .await
            {
                Ok(res) => res,
                Err(e) => {
                    error!(
                        content.key=?task.content_key.to_hex(),
                        err=?e,
                        "Problem requesting validation from Trusted provider node.");
                    active_threads.fetch_sub(1, Ordering::Relaxed);
                    return;
                }
            }
        }
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
    if let Err(e) = content_audit::create(
        content_key_model.id,
        audit_result,
        task.strategy,
        "".to_owned(),
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
