use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use url::Url;

const DEFAULT_DB_URL: &str = "sqlite::memory:";

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = DEFAULT_DB_URL)]
    pub database_url: String,
    #[arg(short, long, requires = "transport")]
    pub ipc_path: Option<PathBuf>,
    #[arg(short = 'u', long, requires = "transport")]
    pub http_url: Option<Url>,
    #[arg(short, long)]
    pub transport: TransportType,
    #[arg(short, long, default_value = "4", help = "number of auditing threads")]
    pub concurrency: u8,
}

#[cfg(test)]
mod test {
    use super::*;

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
