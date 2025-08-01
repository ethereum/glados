use clap::{Parser, ValueEnum};
use entity::Subprotocol;
use std::path::PathBuf;
use url::Url;

/// 15 minutes
const DEFAULT_CENSUS_INTERVAL: &str = "900";

/// Number of concurrent requests that can be in progress towards the connected portal client.
const DEFAULT_CONCURRENCY: &str = "4";

/// How long to keep census data.
const DEFAULT_RETENTION_PERIOD_DAYS: &str = "30";

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long)]
    pub subprotocol: Subprotocol,
    #[arg(short, long)]
    pub database_url: String,
    #[arg(long, requires = "transport")]
    pub ipc_path: Option<PathBuf>,
    #[arg(long, requires = "transport")]
    pub http_url: Option<Url>,
    #[arg(long)]
    pub transport: TransportType,
    #[arg(long, default_value = DEFAULT_CONCURRENCY)]
    pub concurrency: usize,
    #[arg(long, default_value = DEFAULT_CENSUS_INTERVAL)]
    pub census_interval: u64,
    #[arg(long, default_value = DEFAULT_RETENTION_PERIOD_DAYS)]
    pub retention_period_days: Option<u32>,
}

/// Used by a user to specify the intended form of transport
/// to connect to a Portal node.
#[derive(Debug, Clone, Eq, PartialEq, ValueEnum)]
#[clap(rename_all = "snake_case")]
pub enum TransportType {
    IPC,
    HTTP,
}
