use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Bundle(BundleArgs),
}

#[derive(Args)]
pub struct BundleArgs {
    #[arg(short = 'i', long)]
    pub install: bool,
}
