use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = "sqlite::memory:")]
    pub database_url: String,
    #[arg(short, long)]
    pub ipc_path: PathBuf,
}
