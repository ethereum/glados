use std::str::FromStr;

use clap::{ArgAction, Parser, ValueEnum};
use entity::content_audit::{HistorySelectionStrategy, SelectionStrategy};

const DEFAULT_STATS_PERIOD: &str = "300";

#[derive(Parser, Debug, Eq, PartialEq)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub database_url: String,

    #[arg(long, default_value = "4", help = "The number of auditing threads")]
    pub concurrency: usize,

    #[arg(
        long,
        default_value = "100",
        help = "The maximum number of audits per second."
    )]
    pub max_audit_rate: usize,

    #[arg(
        long,
        action(ArgAction::Append),
        help = "Specific strategy to use. Strategy can be selected by name and weight (--strategy random:10) or by name only (--strategy random) which assumes weight 1. May be passed multiple times for multiple strategies (--strategy random --strategy sync:5)"
    )]
    pub strategy: Vec<StrategyWithWeight>,

    #[arg(long, default_value = DEFAULT_STATS_PERIOD, help = "stats recording period (seconds)")]
    pub stats_recording_period: u64,

    #[arg(long, action(ArgAction::Append))]
    pub portal_client: Vec<String>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct StrategyWithWeight {
    pub strategy: SelectionStrategy,
    pub weight: u8,
}

impl StrategyWithWeight {
    pub fn as_tuple(&self) -> (SelectionStrategy, u8) {
        (self.strategy.clone(), self.weight)
    }
}

impl FromStr for StrategyWithWeight {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split(':').collect::<Vec<_>>();

        match parts.len() {
            1 => {
                // assuming weight 1
                let strategy =
                    HistorySelectionStrategy::from_str(parts[0], /* ignore_case= */ true)?;
                Ok(Self {
                    strategy: SelectionStrategy::History(strategy),
                    weight: 1,
                })
            }
            2 => {
                let strategy =
                    HistorySelectionStrategy::from_str(parts[0], /* ignore_case= */ true)?;
                let weight: u8 = parts[1]
                    .parse()
                    .map_err(|_| format!("Invalid strategy weight: {}", parts[1]))?;
                Ok(Self {
                    strategy: SelectionStrategy::History(strategy),
                    weight,
                })
            }
            _ => Err(format!("Unknown strategy: {s}")),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const DATABASE_URL: &str = "postgres://localhost:5432";

    impl Default for Args {
        fn default() -> Self {
            Self {
                database_url: "".to_string(),
                concurrency: 4,
                max_audit_rate: 100,
                strategy: vec![],
                portal_client: vec!["ipc:////tmp/trin-jsonrpc.ipc".to_owned()],
                stats_recording_period: 300,
            }
        }
    }

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
            "--strategy",
            "random",
            "--portal-client",
            PORTAL_CLIENT_STRING,
            "--database-url",
            DATABASE_URL,
        ]);
        let expected = Args {
            database_url: DATABASE_URL.to_string(),
            concurrency: 4,
            strategy: vec![StrategyWithWeight {
                strategy: SelectionStrategy::History(HistorySelectionStrategy::Random),
                weight: 1,
            }],
            portal_client: vec![PORTAL_CLIENT_STRING.to_owned()],
            ..Default::default()
        };
        assert_eq!(result, expected);
    }

    /// Tests that a specific audit strategy can be used with weight.
    #[test]
    fn test_custom_strategy_with_weight() {
        const PORTAL_CLIENT_STRING: &str = "ipc:////path/to/ipc";
        let result = Args::parse_from([
            "test",
            "--strategy",
            "random:10",
            "--portal-client",
            PORTAL_CLIENT_STRING,
            "--database-url",
            DATABASE_URL,
        ]);
        let expected = Args {
            database_url: DATABASE_URL.to_string(),
            concurrency: 4,
            strategy: vec![StrategyWithWeight {
                strategy: SelectionStrategy::History(HistorySelectionStrategy::Random),
                weight: 10,
            }],
            portal_client: vec![PORTAL_CLIENT_STRING.to_owned()],
            ..Default::default()
        };
        assert_eq!(result, expected);
    }

    /// Tests that arbitrary combinations of audit strategies are permitted.
    #[test]
    fn test_multiple_custom_strategies() {
        const PORTAL_CLIENT_STRING: &str = "ipc:////path/to/ipc";
        let result = Args::parse_from([
            "test",
            "--portal-client",
            PORTAL_CLIENT_STRING,
            "--strategy",
            "random",
            "--strategy",
            "sync:2",
            "--database-url",
            DATABASE_URL,
        ]);
        let expected = Args {
            database_url: DATABASE_URL.to_string(),
            concurrency: 4,
            strategy: vec![
                StrategyWithWeight {
                    strategy: SelectionStrategy::History(HistorySelectionStrategy::Random),
                    weight: 1,
                },
                StrategyWithWeight {
                    strategy: SelectionStrategy::History(HistorySelectionStrategy::Sync),
                    weight: 2,
                },
            ],
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
