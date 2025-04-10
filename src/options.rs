use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct Options {
    #[command(flatten)]
    pub common: CommonOptions,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Parser)]
pub struct CommonOptions {
    /// Path to LogiOptions settings database
    pub db: Option<std::path::PathBuf>,
}

impl Options {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}

#[derive(Clone, Parser)]
pub struct TransferAssignments{
    pub from: String,
    pub to: String,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Subcommand)]
pub enum Command {
    ShowSettings,
    ListDevices,
    EditSettings,
    TransferAssignments(TransferAssignments),
}
