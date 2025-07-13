use std::vec;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use entity::content_audit::HistorySelectionStrategy;

const DEFAULT_STATS_PERIOD: &str = "300";

#[derive(Parser, Debug, Eq, PartialEq)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub database_url: String,

    #[arg(short, long, default_value = "4", help = "number of auditing threads")]
    pub concurrency: u8,

    #[arg(
        long = "history",
        default_missing_value("true"),
        default_value("true"),
        num_args(0..=1),
        require_equals(true),
        action = ArgAction::Set,
        help = "Run audits for history subnetwork. Default is true."
    )]
    pub history: bool,

    #[arg(
        long,
        action(ArgAction::Append),
        value_enum,
        default_value = None,
        requires_if("false", "history"),
        help = "Specific strategy to use. Default is to use all available strategies. May be passed multiple times for multiple strategies (--strategy latest --strategy random). Duplicates are permitted (--strategy random --strategy random)."
    )]
    pub history_strategy: Option<Vec<HistorySelectionStrategy>>,

    #[arg(
        short,
        long,
        default_value = "1",
        help = "relative weight of the 'latest' strategy"
    )]
    pub latest_strategy_weight: u8,
    #[arg(
        short,
        long,
        default_value = "1",
        help = "relative weight of the 'failed' strategy"
    )]
    pub failed_strategy_weight: u8,
    #[arg(
        short,
        long,
        default_value = "1",
        help = "relative weight of the 'select oldest unaudited' strategy"
    )]
    pub oldest_strategy_weight: u8,
    #[arg(
        short,
        long,
        default_value = "1",
        help = "relative weight of the 'random' strategy"
    )]
    pub random_strategy_weight: u8,
    #[arg(
        long,
        default_value = "1",
        help = "relative weight of the 'four_fours' strategy"
    )]
    pub four_fours_strategy_weight: u8,

    #[arg(long, default_value = DEFAULT_STATS_PERIOD, help = "stats recording period (seconds)")]
    pub stats_recording_period: u64,

    #[arg(long, action(ArgAction::Append))]
    pub portal_client: Vec<String>,

    #[command(subcommand)]
    pub subcommand: Option<Command>,
}

#[derive(Subcommand, Debug, Eq, PartialEq, Clone)]
pub enum Command {
    /// Run a single audit for a specific, previously audited content key.
    Audit {
        content_key: String,
        portal_client: String,
        database_url: String,
    },
}

impl Default for Args {
    fn default() -> Self {
        Self {
            database_url: "".to_string(),
            concurrency: 4,
            latest_strategy_weight: 1,
            failed_strategy_weight: 1,
            oldest_strategy_weight: 1,
            random_strategy_weight: 1,
            four_fours_strategy_weight: 1,
            history: true,
            history_strategy: None,
            portal_client: vec!["ipc:////tmp/trin-jsonrpc.ipc".to_owned()],
            subcommand: None,
            stats_recording_period: 300,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const DATABASE_URL: &str = "postgres://localhost:5432";

    /// Tests that the defaults are correct when the minimum required flags are passed.
    #[test]
    fn test_minimum_args() {
        const PORTAL_CLIENT_STRING: &str = "ipc:////path/to/ipc";
        let result = Args::parse_from([
            "test",
            "--portal-client",
            PORTAL_CLIENT_STRING,
            "--database-url",
            DATABASE_URL,
        ]);
        let expected = Args {
            database_url: DATABASE_URL.to_string(),
            portal_client: vec![PORTAL_CLIENT_STRING.to_owned()],
            ..Default::default()
        };
        assert_eq!(result, expected);
    }
    #[test]
    fn test_custom_concurrency() {
        const PORTAL_CLIENT_STRING: &str = "ipc:////path/to/ipc";
        let result = Args::parse_from([
            "test",
            "--concurrency",
            "3",
            "--portal-client",
            PORTAL_CLIENT_STRING,
            "--database-url",
            DATABASE_URL,
        ]);
        let expected = Args {
            database_url: DATABASE_URL.to_string(),
            concurrency: 3,
            history_strategy: None,
            portal_client: vec![PORTAL_CLIENT_STRING.to_owned()],
            ..Default::default()
        };
        assert_eq!(result, expected);
    }

    /// Tests that a specific audit strategy can be used without other strategies.
    #[test]
    fn test_custom_strategy() {
        const PORTAL_CLIENT_STRING: &str = "ipc:////path/to/ipc";
        let result = Args::parse_from([
            "test",
            "--history-strategy",
            "latest",
            "--portal-client",
            PORTAL_CLIENT_STRING,
            "--database-url",
            DATABASE_URL,
        ]);
        let expected = Args {
            database_url: DATABASE_URL.to_string(),
            concurrency: 4,
            history_strategy: Some(vec![HistorySelectionStrategy::Latest]),
            portal_client: vec![PORTAL_CLIENT_STRING.to_owned()],
            ..Default::default()
        };
        assert_eq!(result, expected);
    }

    /// Tests that arbitrary combinations of audit strategies are permitted.
    /// This case shows 1 latest and 2 random, which doubles the rate of random audits.
    #[test]
    fn test_multiple_custom_strategies() {
        const PORTAL_CLIENT_STRING: &str = "ipc:////path/to/ipc";
        let result = Args::parse_from([
            "test",
            "--portal-client",
            PORTAL_CLIENT_STRING,
            "--history-strategy",
            "random",
            "--history-strategy",
            "latest",
            "--history-strategy",
            "random", // Duplicate is permitted
            "--database-url",
            DATABASE_URL,
        ]);
        let expected = Args {
            database_url: DATABASE_URL.to_string(),
            concurrency: 4,
            history_strategy: Some(vec![
                HistorySelectionStrategy::Random,
                HistorySelectionStrategy::Latest,
                HistorySelectionStrategy::Random,
            ]),
            portal_client: vec![PORTAL_CLIENT_STRING.to_owned()],
            ..Default::default()
        };
        assert_eq!(result, expected);
    }
}

/// Used by a user to specify the intended form of transport
/// to connect to a Portal node.
#[derive(Debug, Clone, Eq, PartialEq, ValueEnum)]
#[clap(rename_all = "snake_case")]
pub enum TransportType {
    IPC,
    HTTP,
}
