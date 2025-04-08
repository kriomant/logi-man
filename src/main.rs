use std::{collections::BTreeMap, io::Write, os::unix::ffi::OsStrExt};

use clap::{Parser, Subcommand};
use eyre::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Parser)]
struct Options {
    /// Path to LogiOptions settings database
    db: std::path::PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    ShowSettings,
    ListDevices,
    EditSettings,
}

#[derive(Deserialize, Serialize)]
struct Settings {
    profile_keys: Vec<String>,
    ever_connected_devices: EverConnectedDevices,
    migration_report: MigrationReport,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct EverConnectedDevices {
    devices: Vec<ConnectedDevice>,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct ConnectedDevice {
    #[serde(rename="connectionType")]
    connection_type: String,
    #[serde(rename="deviceModel")]
    device_model: String,
    #[serde(rename="deviceType")]
    device_type: String,
    #[serde(rename="slotPrefix")]
    slot_prefix: String,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct MigrationReport {
    devices: Vec<MigrationDevice>,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct MigrationDevice {
    #[serde(rename="deviceName")]
    device_name: String,
    #[serde(rename="modelId")]
    model_id: String,

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

    let db = rusqlite::Connection::open(&options.db)?;
    let settings = load_settings(&db)?;

    match options.command {
        Command::ShowSettings => {
            std::io::stdout().write_all(&settings)?
        }
        Command::ListDevices => {
            let settings: Settings = serde_json::from_slice(&settings)?;

            // Get human-readable model names. I have no idea where LogiOptions application
            // gets them, I suppose they are hardcoded into binary. But some model names
            // are in migration settings. Load them and use.
            let model_names: BTreeMap<&str, &str> = settings.migration_report.devices.iter()
                .map(|device| (device.model_id.as_str(), device.device_name.as_str()))
                .collect();

            let devices: BTreeMap<&str, &ConnectedDevice> = settings.ever_connected_devices.devices.iter()
                // There are some virtual devices in list, skip them.
                .filter(|device| device.device_type == "MOUSE")
                // Sometimes same device is listed several times. Deduplicate records.
                .map(|device| (device.slot_prefix.as_str(), device))
                .collect();

            for device in devices.values() {
                let model_name: &str = model_names.get(device.device_model.as_str()).cloned()
                    // Sometimes model ID in migration settings looks like '6b023',
                    // but device model in device list is '6b023_ext2'.
                    // So try to use first part before '_' to find model name.
                    .or_else(|| {
                        device.device_model.split_once('_')
                            .and_then(|(prefix, _)| model_names.get(prefix).cloned())
                    })
                    // No model name found, use model id.
                    .unwrap_or(device.device_model.as_str());
                println!("{}: {}", device.slot_prefix, model_name);
            }
        }
        Command::EditSettings => {
            // Backup current settings
            db.execute(
                "VACUUM INTO concat(?1, '.', strftime('%Y-%m-%d_%H-%M-%S', 'now', 'localtime'))",
                [options.db.as_os_str().as_bytes()]
            )?;

            let new_settings = edit::edit(&settings)?;
            if new_settings.as_bytes() == settings {
                return Ok(());
            }

            save_settings(&db, &new_settings)?;
        }
    }

    Ok(())
}

fn load_settings(db: &rusqlite::Connection) -> Result<Vec<u8>> {
    let number_of_rows: u32 = db.query_row("SELECT COUNT(*) FROM data", [], |row| row.get(0))?;
    ensure!(number_of_rows == 1, "database is expected to contain single row only, but it contains {} row(s)", number_of_rows);
    let (id, settings): (u32, Vec<u8>) = db.query_row("SELECT _id, file FROM data", [], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    ensure!(id == 1, "settings are expected to have id==1, got {}", id);
    Ok(settings)
}

fn save_settings(db: &rusqlite::Connection, settings: &str) -> Result<()> {
    db.execute("UPDATE data SET file=?1 WHERE _id=1", [settings])?;
    Ok(())
}
