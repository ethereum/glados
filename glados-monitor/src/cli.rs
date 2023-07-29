use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    // Connection to a database where content keys will be injected
    #[arg(short, long, default_value = "sqlite::memory:")]
    pub database_url: String,

    #[arg(short, long, default_value = "false")]
    pub migrate: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    FollowHead {
        // HTTP web3 provider
        #[arg(short, long)]
        provider_url: String,
    },

    FollowHeadPandaops {
        // Pandaops web3 provider
        #[arg(short, long)]
        provider_url: String,
    },

    /// does testing things
    ImportPreMergeAccumulators {
        /// lists test values
        #[arg(short, long)]
        path: PathBuf,
    },

    /// Imports blocks from a remote provider
    BulkDownloadBlockData {
        #[arg(short, long)]
        beginning: u64,
        #[arg(short, long)]
        end: u64,
        #[arg(short, long)]
        provider_url: String,
        #[arg(short, long, default_value = "5")]
        concurrency: u32,
    },
}
