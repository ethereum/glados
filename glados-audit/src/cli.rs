use clap::Parser;
use std::path::PathBuf;

const DEFAULT_DB_URL: &str = "sqlite::memory:";

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = DEFAULT_DB_URL)]
    pub database_url: String,
    #[arg(short, long)]
    pub ipc_path: PathBuf,
    #[arg(short, long, default_value = "4", help = "number of auditing threads")]
    pub concurrency: u8,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_minimum_args() {
        const IPC_PATH: &str = "/path/to/ipc";
        let result = Args::parse_from(["test", "--ipc-path", IPC_PATH]);
        let expected = Args {
            database_url: DEFAULT_DB_URL.to_string(),
            ipc_path: PathBuf::from(IPC_PATH),
            concurrency: 4,
        };
        assert_eq!(result, expected);
    }
    #[test]
    fn test_custom_concurrency() {
        const IPC_PATH: &str = "/path/to/ipc";
        let result = Args::parse_from(["test", "--ipc-path", IPC_PATH, "--concurrency", "3"]);
        let expected = Args {
            database_url: DEFAULT_DB_URL.to_string(),
            ipc_path: PathBuf::from(IPC_PATH),
            concurrency: 3,
        };
        assert_eq!(result, expected);
    }
}
