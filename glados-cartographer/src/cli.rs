use clap::{Parser, ValueEnum};
use entity::content::SubProtocol;
use std::path::PathBuf;
use url::Url;

// 15 minutes
const DEFAULT_CENSUS_INTERVAL: &str = "900";

// Number of concurrent requests that can be in progress towards the connected portal client.
const DEFAULT_CONCURRENCY: &str = "4";

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub database_url: String,
    #[arg(short = 'p', long, requires = "transport")]
    pub ipc_path: Option<PathBuf>,
    #[arg(short = 'u', long, requires = "transport")]
    pub http_url: Option<Url>,
    #[arg(short, long)]
    pub transport: TransportType,
    #[arg(short = 'i', long, default_value = DEFAULT_CENSUS_INTERVAL)]
    pub census_interval: u64,
    #[arg(short, long, default_value = DEFAULT_CONCURRENCY)]
    pub concurrency: usize,
    #[arg(short, long)]
    pub subnetwork: PortalSubnet,
}

/// Used by a user to specify the intended form of transport
/// to connect to a Portal node.
#[derive(Debug, Clone, Eq, PartialEq, ValueEnum)]
#[clap(rename_all = "snake_case")]
pub enum TransportType {
    IPC,
    HTTP,
}

#[derive(Copy, Debug, Clone, Eq, PartialEq, ValueEnum)]
#[clap(rename_all = "snake_case")]
pub enum PortalSubnet {
    History,
    Beacon,
}

impl From<PortalSubnet> for SubProtocol {
    fn from(value: PortalSubnet) -> Self {
        match value {
            PortalSubnet::History => SubProtocol::History,
            PortalSubnet::Beacon => SubProtocol::Beacon,
        }
    }
}
