use clap::{ArgAction, Parser, ValueEnum};
use entity::content_audit::SelectionStrategy;
use std::path::PathBuf;
use url::Url;

const DEFAULT_DB_URL: &str = "sqlite::memory:";

#[derive(Parser, Default, Debug, Eq, PartialEq)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Database connection URL, such as SQLite or PostgreSQL.
    #[arg(short, long, default_value = DEFAULT_DB_URL)]
    pub database_url: String,

    /// IPC Path for connection to Portal node.
    #[arg(short, long, requires = "transport")]
    pub ipc_path: Option<PathBuf>,

    /// HTTP URL for connection to Portal node.
    #[arg(short = 'u', long, requires = "transport")]
    pub http_url: Option<Url>,

    /// Portal node connection mode.
    #[arg(short, long)]
    pub transport: TransportType,

    #[arg(short, long, default_value = "4", help = "number of auditing threads")]
    pub concurrency: u8,

    /// Specific strategy to use. Default is to use all available strategies.
    /// May be passed multiple times for multiple strategies
    /// (--strategy latest --strategy random).
    /// Duplicates are permitted (--strategy random --strategy random).
    #[arg(short, long, action(ArgAction::Append), value_enum, default_value = None)]
    pub strategy: Option<Vec<SelectionStrategy>>,

    /// Non-portal Ethereum execution node for validating data against.
    #[arg(long, default_value = "http")]
    pub trusted_provider: TrustedProvider,

    /// HTTP URL for connection to non-portal Ethereum execution node.
    #[arg(long, requires = "trusted_provider")]
    pub provider_http_url: Option<Url>,

    /// Pandaops URL for connection to non-portal Ethereum execution node.
    #[arg(long, requires = "trusted_provider")]
    pub provider_pandaops: Option<String>,
}

#[cfg(test)]
mod test {
    use super::*;

    /// Tests that the defaults are correct when the minimum required flags are passed.
    #[test]
    fn test_minimum_args() {
        const IPC_PATH: &str = "/path/to/ipc";
        let result = Args::parse_from(["test", "--transport", "ipc", "--ipc-path", IPC_PATH]);
        let expected = Args {
            database_url: DEFAULT_DB_URL.to_string(),
            ipc_path: Some(PathBuf::from(IPC_PATH)),
            concurrency: 4,
            http_url: None,
            transport: TransportType::IPC,
            ..Default::default()
        };
        assert_eq!(result, expected);
    }
    #[test]
    fn test_custom_concurrency() {
        const IPC_PATH: &str = "/path/to/ipc";
        let result = Args::parse_from([
            "test",
            "--transport",
            "ipc",
            "--ipc-path",
            IPC_PATH,
            "--concurrency",
            "3",
        ]);
        let expected = Args {
            database_url: DEFAULT_DB_URL.to_string(),
            ipc_path: Some(PathBuf::from(IPC_PATH)),
            concurrency: 3,
            http_url: None,
            transport: TransportType::IPC,
            ..Default::default()
        };
        assert_eq!(result, expected);
    }

    /// Tests that a specific audit strategy can be used without other strategies.
    #[test]
    fn test_custom_strategy() {
        const IPC_PATH: &str = "/path/to/ipc";
        let result = Args::parse_from([
            "test",
            "--transport",
            "ipc",
            "--ipc-path",
            IPC_PATH,
            "--strategy",
            "latest",
        ]);
        let expected = Args {
            transport: TransportType::IPC,
            database_url: DEFAULT_DB_URL.to_string(),
            http_url: None,
            ipc_path: Some(PathBuf::from(IPC_PATH)),
            concurrency: 4,
            strategy: Some(vec![SelectionStrategy::Latest]),
            ..Default::default()
        };
        assert_eq!(result, expected);
    }

    /// Tests that arbitrary combinations of audit strategies are permitted.
    /// This case shows 1 latest and 2 random, which doubles the rate of random audits.
    #[test]
    fn test_multiple_custom_strategies() {
        const IPC_PATH: &str = "/path/to/ipc";
        let result = Args::parse_from([
            "test",
            "--transport",
            "ipc",
            "--ipc-path",
            IPC_PATH,
            "--strategy",
            "random",
            "--strategy",
            "latest",
            "--strategy",
            "random", // Duplicate is permitted
        ]);
        let expected = Args {
            transport: TransportType::IPC,
            database_url: DEFAULT_DB_URL.to_string(),
            http_url: None,
            ipc_path: Some(PathBuf::from(IPC_PATH)),
            concurrency: 4,
            strategy: Some(vec![
                SelectionStrategy::Random,
                SelectionStrategy::Latest,
                SelectionStrategy::Random,
            ]),
            ..Default::default()
        };
        assert_eq!(result, expected);
    }
}

/// Used by a user to specify the intended form of transport
/// to connect to a Portal node.
#[derive(Debug, Default, Clone, Eq, PartialEq, ValueEnum)]
#[clap(rename_all = "snake_case")]
pub enum TransportType {
    #[default]
    HTTP,
    IPC,
}

/// Used by a user to specify the intended form of transport
/// to connect to a Portal node.
#[derive(Debug, Default, Clone, Eq, PartialEq, ValueEnum)]
#[clap(rename_all = "snake_case")]
pub enum TrustedProvider {
    #[default]
    /// An HTTP-based provider.
    HTTP,
    /// A Trin operations provider.
    Pandaops,
}
