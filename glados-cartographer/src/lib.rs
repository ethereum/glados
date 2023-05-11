use anyhow::{bail, Result};
use clap::Parser;
use cli::Args;
use ethereum_types::H256;
use ethportal_api::HistoryNetworkApiClient;
use ethportal_api::{jsonrpsee::http_client::HttpClientBuilder, NodeId as EthPortalNodeId};
use sea_orm::DatabaseConnection;
use tokio::time::{self, Duration};
use tracing::{debug, info};
use trin_types::node_id::NodeId;

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
    pub probe_interval: u64,
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
            probe_interval: args.probe_interval,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DHTAudit {
    pub origin: NodeId,
    pub seen_nodes: Vec<NodeId>,
    pub pending_nodes: Vec<NodeId>,
}

pub async fn run_glados_cartographer(conn: DatabaseConnection, config: CartographerConfig) {
    tokio::spawn(perform_dht_audits(config, conn.clone()));
    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
}

async fn perform_dht_audits(config: CartographerConfig, conn: DatabaseConnection) {
    let mut interval = time::interval(Duration::from_secs(config.probe_interval));

    loop {
        debug!("Begin main cartographer audit loop");
        interval.tick().await;
        perform_dht_probe(&config, &conn).await;
        debug!("End main cartographer audit loop");
    }
}

/// Performs an audit against a Portal node.
///
/// After auditing finishes the thread counter is deprecated. This
/// applies even if the audit process encounters an error.
async fn perform_dht_probe(config: &CartographerConfig, conn: &DatabaseConnection) {
    let client = match &config.transport {
        TransportConfig::HTTP(http_url) => HttpClientBuilder::default()
            .build(http_url.as_ref())
            .unwrap(),
        TransportConfig::IPC(_path) => panic!("not implemented"),
    };

    let target = NodeId::random();
    let target_display = H256::from(target.0);

    info!(
        target.node_id=?target_display,
        "Performing RFN on DHT",
    );

    let found_enrs = client
        .recursive_find_nodes(EthPortalNodeId(target.raw()))
        .await
        .unwrap();

    info!(
        target.node_id=?target_display,
        count=?found_enrs.len(),
        "RFN found ENR records",
    );

    for enr in found_enrs {
        record::get_or_create(&enr, conn).await.unwrap();
        info!(
        enr.base64=?enr,
        enr.seq=?enr.seq(),
        enr.node_id=?H256::from(enr.node_id().raw()),
        "ENR saved",
        );
    }
}
