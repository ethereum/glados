use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    // Connection to a database where content keys will be injected
    #[arg(short, long)]
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

    FollowBeacon {
        // Beacon node base URL
        #[arg(short, long)]
        beacon_base_url: String,
    },

    FollowBeaconPandaops {},

    /// Imports blocks from a remote provider
    BulkDownloadBlockData {
        #[arg(
            short,
            long,
            help = "The block number with which the download will begin"
        )]
        start_block_number: u64,
        #[arg(
            short,
            long,
            help = "The block number (exclusive) with which the download will end"
        )]
        end_block_number: u64,
        #[arg(short, long)]
        provider_url: String,
        // 100 is chosen because it is Postgres' default max connections
        #[arg(short, long, default_value = "100")]
        concurrency: u32,
    },

    /// Follows the head of the chain and stores state roots for each block
    FollowHeadState {
        // HTTP web3 provider
        #[arg(short, long)]
        provider_url: String,
    },

    /// Takes a range and populates the state roots table with the state roots for that range
    PopulateStateRootsRange {
        #[arg(
            short,
            long,
            help = "The block number with which the download will begin"
        )]
        start_block_number: u64,
        #[arg(
            short,
            long,
            help = "The block number (exclusive) with which the download will end"
        )]
        end_block_number: u64,
        #[arg(short, long)]
        provider_url: String,
        // 100 is chosen because it is Postgres' default max connections
        #[arg(short, long, default_value = "100")]
        concurrency: u32,
    },
    Seed {
        #[arg(short, long, help = "The name of the table to seed")]
        table_name: String,
    },
}
