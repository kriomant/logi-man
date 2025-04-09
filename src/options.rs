use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct Options {
    /// Path to LogiOptions settings database
    pub db: std::path::PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

impl Options {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}

#[derive(Clone, Subcommand)]
pub enum Command {
    ShowSettings,
    ListDevices,
    EditSettings,
    TransferAssignments {
        from: String,
        to: String,
        #[arg(long)]
        dry_run: bool,
    },
}