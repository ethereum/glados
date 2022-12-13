use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub provider_url: String,
    #[arg(short, long, default_value = "sqlite::memory:")]
    pub database_url: String,
}
