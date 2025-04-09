#![feature(exit_status_error, cfg_match)]

use std::{collections::BTreeMap, io::Write, os::unix::ffi::OsStrExt, path::Path};

use clap::{Parser, Subcommand};
use eyre::{ensure, Result};
use serde::{de::Error as _, Deserialize, Serialize};
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
    TransferAssignments {
        from: String,
        to: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(try_from="RawSettings")]
#[serde(into="RawSettings")]
struct Settings {
    profile_keys: Vec<String>,
    profiles: BTreeMap<String, Profile>,
    ever_connected_devices: EverConnectedDevices,
    migration_report: MigrationReport,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

impl TryFrom<RawSettings> for Settings {
    type Error = serde_json::Error;

    fn try_from(mut raw: RawSettings) -> std::result::Result<Self, Self::Error> {
        let mut profiles = BTreeMap::new();
        for profile_name in &raw.profile_keys {
            let profile = raw.rest.remove(profile_name)
                .ok_or_else(|| serde_json::Error::custom(format!("missing profile: {profile_name}")))?;
            let profile: Profile = serde_json::from_value(profile)?;
            profiles.insert(profile_name.clone(), profile);
        }
        Ok(Settings {
            profile_keys: raw.profile_keys,
            profiles,
            ever_connected_devices: raw.ever_connected_devices,
            migration_report: raw.migration_report,
            rest: raw.rest,
        })
    }
}

impl Into<RawSettings> for Settings {
    fn into(mut self) -> RawSettings {
        for (profile_name, profile) in self.profiles {
            self.rest.insert(profile_name, serde_json::to_value(profile).unwrap());
        }
        RawSettings {
            profile_keys: self.profile_keys,
            ever_connected_devices: self.ever_connected_devices,
            migration_report: self.migration_report,
            rest: self.rest,
        }
    }
}

#[derive(Deserialize, Serialize)]
struct RawSettings {
    profile_keys: Vec<String>,
    ever_connected_devices: EverConnectedDevices,
    migration_report: MigrationReport,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
struct EverConnectedDevices {
    devices: Vec<ConnectedDevice>,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
struct ConnectedDevice {
    #[serde(rename="connectionType")]
    connection_type: Option<String>,
    #[serde(rename="deviceModel")]
    device_model: String,
    #[serde(rename="deviceType")]
    device_type: String,
    #[serde(rename="slotPrefix")]
    slot_prefix: String,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
struct MigrationReport {
    devices: Vec<MigrationDevice>,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
struct MigrationDevice {
    #[serde(rename="deviceName")]
    device_name: String,
    #[serde(rename="modelId")]
    model_id: String,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Profile {
    assignments: Vec<Assignment>,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
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
            backup_database(&options.db, &db)?;

            let new_settings = edit::edit(&settings)?;
            if new_settings.as_bytes() == settings {
                return Ok(());
            }

            save_settings(&db, &new_settings)?;
        }
        Command::TransferAssignments { from, to, dry_run } => {
            let mut settings: Settings = serde_json::from_slice(&settings)?;
            if !dry_run {
                backup_database(&options.db, &db)?;
            }

            for profile in settings.profiles.values_mut() {
                // Gather and clone source assignments
                let mut new_assignments: Vec<Assignment> = profile.assignments.iter()
                    // Get only assignments for source device, leave slot suffix only
                    .filter_map(|a| {
                        let (device, button) = a.slot_id.split_once('_')?;
                        (device == from).then(|| Assignment { slot_id: format!("{}_{}", to, button), ..a.clone()})
                    })
                    .collect();
                // Remove all existing assignments for target device.
                profile.assignments.retain(|a| a.slot_id.split_once('_').is_some_and(|(device, _)| device != to));
                // Append new assignemnts.
                profile.assignments.append(&mut new_assignments);
            }

            let settings = serde_json::to_string_pretty(&settings)?;
            if dry_run {
                println!("{}", settings);
            } else {
                save_settings(&db, &settings)?;

                restart_logi_agent()?;
            }
        }
    }

    Ok(())
}

fn restart_logi_agent() -> Result<(), eyre::Error> {
    cfg_match! {
        target_os="macos" => {
            let uid = unsafe { libc::getuid() };
            std::process::Command::new("/bin/launchctl")
                .args(["kill", "SIGKILL"])
                .arg(format!("gui/{uid}/com.logi.cp-dev-mgr"))
                .status()?
                .exit_ok()?;
        }
        _ => {
        }
    }
    Ok(())
}

fn backup_database(db_path: &Path, db: &rusqlite::Connection) -> Result<(), eyre::Error> {
    db.execute(
        "VACUUM INTO concat(?1, '.', strftime('%Y-%m-%d_%H-%M-%S', 'now', 'localtime'))",
        [db_path.as_os_str().as_bytes()]
    )?;
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
    db.execute("UPDATE data SET file=?1 WHERE _id=1", [settings.as_bytes()])?;
    Ok(())
}
