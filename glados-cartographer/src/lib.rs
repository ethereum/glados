use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use cli::Args;
use ethereum_types::H256;
use ethportal_api::HistoryNetworkApiClient;
use ethportal_api::{generate_random_remote_enr, jsonrpsee::http_client::HttpClientBuilder};
use ethportal_api::{Enr, NodeId};
use primitive_types::U256;
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        RwLock, Semaphore,
    },
    time::{self},
};
use tracing::{debug, error, info, warn};

use entity::{census, census_node, record};
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

struct DHTCensusRecord {
    enr: Enr,
    record_id: i32,
    data_radius: U256,
    surveyed_at: DateTime<Utc>,
}

struct DHTCensus {
    known: RwLock<HashSet<[u8; 32]>>,
    pub alive: RwLock<HashMap<[u8; 32], DHTCensusRecord>>,
    finished: RwLock<HashSet<[u8; 32]>>,
    errored: RwLock<HashSet<[u8; 32]>>,
    pub started_at: DateTime<Utc>,
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
        let alive: RwLock<HashMap<[u8; 32], DHTCensusRecord>> = RwLock::new(HashMap::new());
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
            .checked_div(duration.num_seconds().try_into().unwrap()) // should always fit into usize
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

    async fn add_alive(&self, enr: Enr, record_id: i32, data_radius: U256) {
        if self.alive.read().await.contains_key(&enr.node_id().raw()) {
            return;
        }
        let census_record = DHTCensusRecord {
            enr,
            record_id,
            data_radius,
            surveyed_at: Utc::now(),
        };
        let mut alive = self.alive.write().await;
        alive.insert(census_record.enr.node_id().raw(), census_record);
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
/// 3. For each node-id, enumerate its routing table entries until we find empty buckets.
/// 4. Track all seen node-ids until we find no new ones.
async fn perform_dht_census(config: CartographerConfig, conn: DatabaseConnection) {
    let client = match &config.transport {
        TransportConfig::HTTP(http_url) => {
            match HttpClientBuilder::default()
                .request_timeout(StdDuration::from_secs(62))
                .build(http_url.as_ref())
            {
                Ok(client) => {
                    debug!(client.http_url=?http_url, "Portal JSON-RPC HTTP client initialized");
                    client
                }
                Err(err) => {
                    error!(client.http_url=?http_url, err=?err, "Error initializing Portal JSON-RPC HTTP client");
                    return;
                }
            }
        }
        TransportConfig::IPC(_path) => panic!("not implemented"),
    };

    let target_enr = generate_random_remote_enr().1;
    let target = target_enr.node_id();
    let census = Arc::new(DHTCensus::new());

    // Initial un-processed ENRs to be pinged
    let (to_ping_tx, to_ping_rx): (Sender<Enr>, Receiver<Enr>) = mpsc::channel(256);

    // ENRs that have been pinged and now need to have their routing tables enumerated
    let (to_enumerate_tx, to_enumerate_rx): (Sender<Enr>, Receiver<Enr>) = mpsc::channel(256);

    info!(
        target.node_id=?H256::from(target.raw()),
        "Starting DHT census",
    );

    // Initialize our search with a random-ish set of ENRs
    let initial_enrs = match client.recursive_find_nodes(target).await {
        Ok(initial_enrs) => initial_enrs,
        Err(err) => {
            error!(target.node_id=?H256::from(target.raw()), err=?err, "Error during census initialization");
            return;
        }
    };

    for enr in initial_enrs {
        census.add_known(NodeId(enr.node_id().raw())).await;
        match to_ping_tx.send(enr).await {
            Ok(_) => (),
            Err(err) => {
                error!(err=?err, "Error during census initialization");
                return;
            }
        };
    }

    // Give each semaphore half of the concurrency to use, with a lower limit
    let num_permits = std::cmp::max(config.concurrency / 2, 1);
    let ping_limiter = Arc::new(Semaphore::new(num_permits));
    let enumeration_limiter = Arc::new(Semaphore::new(num_permits));

    let ping_handle = tokio::task::spawn(orchestrate_liveliness_checks(
        to_ping_rx,
        to_enumerate_tx.clone(),
        census.clone(),
        config.to_owned(),
        conn.to_owned(),
        ping_limiter.clone(),
    ));
    let enumerate_handle = tokio::task::spawn(orchestrate_routing_table_enumerations(
        to_enumerate_rx,
        to_ping_tx.clone(),
        census.clone(),
        config.to_owned(),
        enumeration_limiter.clone(),
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
            ap_enumeration = enumeration_limiter.available_permits(),
            ap_ping = ping_limiter.available_permits(),
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

        if to_ping_tx.is_closed() {
            warn!("The `to_ping_tx` channel is closed");
        }
        if to_enumerate_tx.is_closed() {
            warn!("The `to_enumerate_tx` channel is closed");
        }
    }

    debug!("Waiting for channels to exit");
    ping_handle.abort();
    enumerate_handle.abort();

    let duration: u32 = census.duration().num_seconds().try_into().unwrap();

    let census_model = match census::create(census.started_at, duration, &conn).await {
        Ok(census_model) => census_model,
        Err(err) => {
            error!(err=?err, "Error saving census model to database");
            return;
        }
    };

    for (_, census_record) in census.alive.read().await.iter() {
        match census_node::create(
            census_model.id,
            census_record.record_id,
            census_record.data_radius,
            census_record.surveyed_at,
            &conn,
        )
        .await
        {
            Ok(census_node_model) => debug!(
                census.id = census_model.id,
                census.node.id = census_node_model.id,
                "Saved new census_node record"
            ),
            Err(err) => error!(
                census.id=census_model.id,
                census_node.record_id=census_record.record_id,
                census_node.data_radius=?census_record.data_radius,
                census_node.surveyed_at=?census_record.surveyed_at,
                err=?err,
                "Error saving new census_node record"
            ),
        };
    }

    info!("Census finished");
}

/// Sub-component of perform_dht_census()
///
async fn orchestrate_liveliness_checks(
    mut to_ping_rx: mpsc::Receiver<Enr>,
    to_enumerate_tx: mpsc::Sender<Enr>,
    census: Arc<DHTCensus>,
    config: CartographerConfig,
    conn: DatabaseConnection,
    limiter: Arc<Semaphore>,
) {
    while let Some(enr) = to_ping_rx.recv().await {
        let permit = limiter
            .clone()
            .acquire_owned()
            .await
            .expect("Unable to acquire permit");
        let handle = do_liveliness_check(
            enr,
            to_enumerate_tx.clone(),
            census.clone(),
            config.clone(),
            conn.clone(),
        );
        tokio::spawn(async move {
            handle.await;
            drop(permit);
        });
    }
}

async fn do_liveliness_check(
    enr: Enr,
    to_enumerate_tx: mpsc::Sender<Enr>,
    census: Arc<DHTCensus>,
    config: CartographerConfig,
    conn: DatabaseConnection,
) {
    let client = match config.transport {
        TransportConfig::HTTP(http_url) => {
            match HttpClientBuilder::default()
                .request_timeout(StdDuration::from_secs(2))
                .build(http_url.as_ref())
            {
                Ok(client) => client,
                Err(err) => {
                    error!(client.http_url=?http_url, err=?err, "Error initializing Portal JSON-RPC HTTP client");
                    census.add_errored(NodeId(enr.node_id().raw())).await;
                    return;
                }
            }
        }
        TransportConfig::IPC(_path) => panic!("not implemented"),
    };

    // Save record to database
    let record_model = match record::get_or_create(&enr, &conn).await {
        Ok(record_model) => {
            debug!(enr.base64 = enr.to_base64(), "Saved ENR");
            record_model
        }
        Err(err) => {
            error!(enr.node_id=?H256::from(enr.node_id().raw()), err=?err, "Error saving ENR to database");
            census.add_errored(NodeId(enr.node_id().raw())).await;
            return;
        }
    };

    // Perform liviliness check
    debug!(node_id=?H256::from(enr.node_id().raw()), "Liveliness check");

    match client.ping(enr.to_owned()).await {
        Ok(pong_info) => {
            debug!(node_id=?H256::from(enr.node_id().raw()), "Liveliness passed");

            // Mark node as known to be alive
            census
                .add_alive(enr.clone(), record_model.id, pong_info.data_radius)
                .await;

            // Send enr to process that enumerates its routing table
            match to_enumerate_tx.send(enr.clone()).await {
                Ok(_) => (),
                Err(err) => {
                    error!(err=?err, "Error queueing enr for routing table enumeration");
                    census.add_finished(NodeId(enr.node_id().raw())).await;
                }
            }
        }
        Err(err) => {
            warn!(node_id=?H256::from(enr.node_id().raw()), err=?err, "Liveliness failed");

            // Add node to error list.
            census.add_errored(NodeId(enr.node_id().raw())).await;
        }
    }
}

async fn orchestrate_routing_table_enumerations(
    mut to_enumerate_rx: mpsc::Receiver<Enr>,
    to_ping_tx: mpsc::Sender<Enr>,
    census: Arc<DHTCensus>,
    config: CartographerConfig,
    limiter: Arc<Semaphore>,
) {
    while let Some(enr) = to_enumerate_rx.recv().await {
        let permit = limiter
            .clone()
            .acquire_owned()
            .await
            .expect("Unable to acquire permit");
        let handle =
            do_routing_table_enumeration(enr, to_ping_tx.clone(), census.clone(), config.clone());
        tokio::spawn(async move {
            handle.await;
            drop(permit);
        });
    }
}

async fn do_routing_table_enumeration(
    enr: Enr,
    to_ping_tx: mpsc::Sender<Enr>,
    census: Arc<DHTCensus>,
    config: CartographerConfig,
) {
    let client = match config.transport {
        TransportConfig::HTTP(http_url) => {
            match HttpClientBuilder::default()
                .request_timeout(StdDuration::from_secs(2))
                .build(http_url.as_ref())
            {
                Ok(client) => client,
                Err(err) => {
                    error!(client.http_url=?http_url, err=?err, "Error initializing Portal JSON-RPC HTTP client");
                    census.add_errored(NodeId(enr.node_id().raw())).await;
                    return;
                }
            }
        }
        TransportConfig::IPC(_path) => panic!("not implemented"),
    };

    debug!(enr.node_id=?H256::from(enr.node_id().raw()), "Enumerating Routing Table");

    for distance in 245..257 {
        let enrs_at_distance = match client.find_nodes(enr.to_owned(), vec![distance]).await {
            Ok(result) => result,
            Err(msg) => {
                warn!(enr.node_id=?H256::from(enr.node_id().raw()), distance=?distance, msg=?msg, "Error fetching routing table info");
                continue;
            }
        };
        debug!(enr.node_id=?H256::from(enr.node_id().raw()), distance=distance, count=enrs_at_distance.len(), "Routing Table Info");
        for found_enr in enrs_at_distance {
            if census.is_known(NodeId(found_enr.node_id().raw())).await {
                continue;
            } else {
                census.add_known(NodeId(found_enr.node_id().raw())).await;
                to_ping_tx
                    .send(found_enr)
                    .await
                    .expect("Error queuing liveliness check");
            }
        }
    }
    census.add_finished(NodeId(enr.node_id().raw())).await;
}
