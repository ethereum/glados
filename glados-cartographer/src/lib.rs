use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use cli::Args;
use enr::NodeId as DiscV5NodeId;
use ethereum_types::H256;
use ethportal_api::jsonrpsee::http_client::HttpClientBuilder;
use ethportal_api::types::discv5::{Enr, NodeId};
use ethportal_api::HistoryNetworkApiClient;
use sea_orm::DatabaseConnection;
use std::collections::hash_set::HashSet;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        OwnedSemaphorePermit, RwLock, Semaphore,
    },
    time,
};
use tracing::{debug, error, info};

use entity::record;
use glados_core::jsonrpc::TransportConfig;

use crate::cli::TransportType;

pub mod cli;

/// Configuration created from CLI arguments.
#[derive(Clone, Debug)]
pub struct CartographerConfig {
    /// For Glados-related data.
    pub database_url: String,
    /// For communication with a Portal Network node.
    pub transport: TransportConfig,
    /// Defines the rate at which the network is probed in seconds
    pub census_interval: u64,
    /// Total number of concurrent requests to portal client
    pub concurrency: usize,
}

impl CartographerConfig {
    pub fn from_args() -> Result<CartographerConfig> {
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
        Ok(CartographerConfig {
            database_url: args.database_url,
            transport,
            census_interval: args.census_interval,
            concurrency: args.concurrency,
        })
    }
}

pub async fn run_glados_cartographer(conn: DatabaseConnection, config: CartographerConfig) {
    tokio::spawn(orchestrate_dht_census(config, conn));

    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

async fn orchestrate_dht_census(config: CartographerConfig, conn: DatabaseConnection) {
    let mut interval = time::interval(StdDuration::from_secs(config.census_interval));

    loop {
        interval.tick().await;

        perform_dht_census(config.clone(), conn.clone()).await;
    }
}

struct DHTCensus {
    known: RwLock<HashSet<[u8; 32]>>,
    alive: RwLock<HashSet<[u8; 32]>>,
    finished: RwLock<HashSet<[u8; 32]>>,
    errored: RwLock<HashSet<[u8; 32]>>,
    started_at: DateTime<Utc>,
}

struct DHTCensusStats {
    known: usize,
    alive: usize,
    finished: usize,
    errored: usize,
    pending: usize,
    duration: Duration,
    requests_per_second: usize,
}

impl DHTCensus {
    fn new() -> Self {
        let known: RwLock<HashSet<[u8; 32]>> = RwLock::new(HashSet::new());
        let alive: RwLock<HashSet<[u8; 32]>> = RwLock::new(HashSet::new());
        let finished: RwLock<HashSet<[u8; 32]>> = RwLock::new(HashSet::new());
        let errored: RwLock<HashSet<[u8; 32]>> = RwLock::new(HashSet::new());

        let started_at = chrono::offset::Utc::now();

        DHTCensus {
            known,
            alive,
            finished,
            errored,
            started_at,
        }
    }

    fn duration(&self) -> Duration {
        chrono::offset::Utc::now() - self.started_at
    }

    async fn is_done(&self) -> bool {
        let known = self.known.read().await.len();
        let finished = self.finished.read().await.len();
        let errored = self.errored.read().await.len();

        if known == 0 {
            false
        } else {
            errored + finished == known
        }
    }

    async fn stats(&self) -> DHTCensusStats {
        let known = self.known.read().await.len();
        let alive = self.alive.read().await.len();
        let finished = self.finished.read().await.len();
        let errored = self.errored.read().await.len();
        let pending = known.saturating_sub(finished).saturating_sub(errored);
        let duration = self.duration();

        let requests_per_second = (alive + finished + errored)
            .checked_div(duration.num_seconds().try_into().unwrap())
            .unwrap_or(0);

        DHTCensusStats {
            known,
            alive,
            finished,
            errored,
            pending,
            duration,
            requests_per_second,
        }
    }

    async fn is_known(&self, node_id: NodeId) -> bool {
        let known = self.known.read().await;
        known.contains(&node_id.0)
    }

    async fn add_known(&self, node_id: NodeId) -> bool {
        let mut known = self.known.write().await;
        known.insert(node_id.0)
    }

    async fn add_alive(&self, node_id: NodeId) -> bool {
        let mut alive = self.alive.write().await;
        alive.insert(node_id.0)
    }

    async fn add_finished(&self, node_id: NodeId) -> bool {
        let mut finished = self.finished.write().await;
        finished.insert(node_id.0)
    }

    async fn add_errored(&self, node_id: NodeId) -> bool {
        let mut errored = self.errored.write().await;
        errored.insert(node_id.0)
    }
}

/// Performs a full census of the DHT
///
/// 1. Start with a random node-id
/// 2. Use RFN with a random node-id to initialize our view of the network
/// 3. For each node-id, enumerate it's routing table entries until we find empty buckets.
/// 4. Track all seen node-ids until we find no new ones.
async fn perform_dht_census(config: CartographerConfig, conn: DatabaseConnection) {
    let client = match &config.transport {
        TransportConfig::HTTP(http_url) => HttpClientBuilder::default()
            .build(http_url.as_ref())
            .unwrap(),
        TransportConfig::IPC(_path) => panic!("not implemented"),
    };

    let origin = NodeId(DiscV5NodeId::random().raw());

    info!(
        origin.node_id=?H256::from(origin.0),
        "Starting DHT census",
    );

    let census = Arc::new(DHTCensus::new());

    // Initial un-processed ENRs to be pinged
    let (to_ping_tx, to_ping_rx): (Sender<Enr>, Receiver<Enr>) = mpsc::channel(256);

    // ENRs that have been pinged and now need to have their routing tables enumerated
    let (to_enumerate_tx, to_enumerate_rx): (Sender<Enr>, Receiver<Enr>) = mpsc::channel(256);

    // Initialize our search with a random-ish set of ENRs
    let initial_enrs = client.recursive_find_nodes(origin).await.unwrap();
    for enr in initial_enrs {
        census.add_known(NodeId(enr.node_id().raw())).await;
        to_ping_tx.send(enr).await.unwrap();
    }

    let limiter = Arc::new(Semaphore::new(config.concurrency));

    let ping_handle = tokio::task::spawn(orchestrate_liveliness_checks(
        to_ping_rx,
        to_enumerate_tx,
        census.clone(),
        config.to_owned(),
        conn.to_owned(),
        limiter.clone(),
    ));
    let enumerate_handle = tokio::task::spawn(orchestrate_routing_table_enumerations(
        to_enumerate_rx,
        to_ping_tx,
        census.clone(),
        config.to_owned(),
        limiter.clone(),
    ));

    let mut interval = time::interval(StdDuration::from_secs(5));

    loop {
        interval.tick().await;
        let stats = census.stats().await;

        info!(
            known = stats.known,
            alive = stats.alive,
            finished = stats.finished,
            errored = stats.errored,
            pending = stats.pending,
            elapsed = stats.duration.num_seconds(),
            rps = stats.requests_per_second,
            "Census progress",
        );

        if census.is_done().await {
            let final_stats = census.stats().await;
            info!(
                known = final_stats.known,
                alive = final_stats.alive,
                finished = final_stats.finished,
                errored = final_stats.errored,
                pending = final_stats.pending,
                duration = final_stats.duration.num_seconds(),
                rps = final_stats.requests_per_second,
                "Census complete",
            );
            break;
        }
    }

    debug!("Waiting for channels to exit");
    ping_handle.abort();
    enumerate_handle.abort();
    info!("Census finished");
}

/// Sub-component of perform_dht_census()
///
async fn orchestrate_liveliness_checks(
    mut rx: mpsc::Receiver<Enr>,
    tx: mpsc::Sender<Enr>,
    census: Arc<DHTCensus>,
    config: CartographerConfig,
    conn: DatabaseConnection,
    limiter: Arc<Semaphore>,
) {
    while let Some(enr) = rx.recv().await {
        let permit = limiter
            .clone()
            .acquire_owned()
            .await
            .expect("Unable to acquire permit");
        tokio::spawn(do_liveliness_check(
            enr,
            tx.clone(),
            census.clone(),
            config.clone(),
            conn.clone(),
            permit,
        ));
    }
}

async fn do_liveliness_check(
    enr: Enr,
    tx: mpsc::Sender<Enr>,
    census: Arc<DHTCensus>,
    config: CartographerConfig,
    conn: DatabaseConnection,
    permit: OwnedSemaphorePermit,
) {
    let client = match config.transport {
        TransportConfig::HTTP(http_url) => HttpClientBuilder::default()
            .build(http_url.as_ref())
            .unwrap(),
        TransportConfig::IPC(_path) => panic!("not implemented"),
    };

    // Save record to database
    match record::get_or_create(&enr, &conn).await {
        Ok(_) => debug!(enr.base64 = enr.to_base64(), "Saved ENR"),
        Err(err) => {
            error!(enr.node_id=?H256::from(enr.node_id().raw()), err=?err, "Error saving ENR to database")
        }
    }

    // Perform liviliness check
    debug!(node_id=?H256::from(enr.node_id().raw()), "Liveliness check");

    match client.ping(enr.to_owned(), None).await {
        Ok(_pong_info) => {
            debug!(node_id=?H256::from(enr.node_id().raw()), "Liveliness passed");

            // Mark node as known to be alive
            census.add_alive(NodeId(enr.node_id().raw())).await;

            // Send enr to process that enumerates it's routing table
            tx.send(enr).await.unwrap();
        }
        Err(err) => {
            debug!(node_id=?H256::from(enr.node_id().raw()), err=?err, "Liveliness failed");

            // Add node to error list.
            census.add_errored(NodeId(enr.node_id().raw())).await;
        }
    }

    drop(permit);
}

async fn orchestrate_routing_table_enumerations(
    mut rx: mpsc::Receiver<Enr>,
    tx: mpsc::Sender<Enr>,
    census: Arc<DHTCensus>,
    config: CartographerConfig,
    limiter: Arc<Semaphore>,
) {
    while let Some(enr) = rx.recv().await {
        let permit = limiter
            .clone()
            .acquire_owned()
            .await
            .expect("Unable to acquire permit");
        tokio::spawn(do_routing_table_enumeration(
            enr,
            tx.clone(),
            census.clone(),
            config.clone(),
            permit,
        ));
    }
}

async fn do_routing_table_enumeration(
    enr: Enr,
    tx: mpsc::Sender<Enr>,
    census: Arc<DHTCensus>,
    config: CartographerConfig,
    permit: OwnedSemaphorePermit,
) {
    let client = match config.transport {
        TransportConfig::HTTP(http_url) => HttpClientBuilder::default()
            .build(http_url.as_ref())
            .unwrap(),
        TransportConfig::IPC(_path) => panic!("not implemented"),
    };

    debug!(enr.node_id=?H256::from(enr.node_id().raw()), "Enumerating Routing Table");

    for distance in 245..257 {
        let enrs_at_distance = match client.find_nodes(enr.to_owned(), vec![distance]).await {
            Ok(result) => result,
            Err(msg) => {
                debug!(enr.node_id=?H256::from(enr.node_id().raw()), msg=?msg, "Error fetching routing table info");
                continue;
            }
        };
        debug!(enr.node_id=?H256::from(enr.node_id().raw()), distance=distance, total=enrs_at_distance.total, count=enrs_at_distance.enrs.len(), "Routing Table Info");
        for found_enr in enrs_at_distance.enrs {
            if census.is_known(NodeId(found_enr.node_id().raw())).await {
                continue;
            } else {
                census.add_known(NodeId(found_enr.node_id().raw())).await;
                tx.send(found_enr)
                    .await
                    .expect("Error queuing liveliness check");
            }
        }
    }
    census.add_finished(NodeId(enr.node_id().raw())).await;

    drop(permit);
}
