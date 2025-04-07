use std::path::Path;

use clap::Parser;
use eyre::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Parser)]
struct Options {
    /// Path to LogiOptions settings database
    db: std::path::PathBuf,
}

#[derive(Deserialize, Serialize)]
struct Settings {
    profile_keys: Vec<String>,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct Profile {
    assignments: Vec<Assignment>,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct Assignment {
    #[serde(rename="slotId")]
    slot_id: String,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

fn main() -> Result<()> {
    let options = Options::parse();

    let settings = load_settings(&options.db)?;
    let settings: Settings = serde_json::from_slice(&settings)?;

    serde_json::to_writer_pretty(std::io::stdout(), &settings)?;

    Ok(())
}

fn load_settings(db: &Path) -> Result<Vec<u8>, eyre::Error> {
    let db = rusqlite::Connection::open(db)?;
    let number_of_rows: u32 = db.query_row("SELECT COUNT(*) FROM data", [], |row| row.get(0))?;
    ensure!(number_of_rows == 1, "database is expected to contain single row only, but it contains {} row(s)", number_of_rows);
    let (id, settings): (u32, Vec<u8>) = db.query_row("SELECT _id, file FROM data", [], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    ensure!(id == 1, "settings are expected to have id==1, got {}", id);
    Ok(settings)
}
