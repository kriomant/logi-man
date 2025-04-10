#![feature(exit_status_error, cfg_match)]

use std::{collections::BTreeMap, io::Write, os::unix::ffi::OsStrExt, path::Path};

use directories_next::BaseDirs;
use eyre::{ensure, OptionExt, Result};

mod options;
mod models;

use options::{Command, Options, TransferAssignments};
use models::{Assignment, ConnectedDevice, Settings};

fn main() -> Result<()> {
    let options = Options::parse();

    // Autodetect database path if needed.
    let db_path = match options.common.db {
        Some(path) => path,
        None => {
            let dirs = BaseDirs::new().ok_or_eyre("can't get user directory path")?;
            dirs.data_local_dir().join("LogiOptionsPlus/settings.db")
        }
    };

    let db = rusqlite::Connection::open(&db_path)?;
    let settings = load_settings(&db)?;

    match options.command.clone() {
        Command::ShowSettings => show_settings(settings),
        Command::ListDevices => list_devices(settings),
        Command::EditSettings => edit_settings(&db_path, db, settings),
        Command::TransferAssignments(opts) => transfer_assignments(&db_path, opts, db, settings)
    }
}

fn show_settings(settings: Vec<u8>) -> Result<()> {
    std::io::stdout().write_all(&settings)?;
    Ok(())
}

fn list_devices(settings: Vec<u8>) -> Result<()> {
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

    Ok(())
}

fn edit_settings(db_path: &Path, db: rusqlite::Connection, settings: Vec<u8>) -> Result<()> {
    backup_database(db_path, &db)?;

    let new_settings = edit::edit(&settings)?;
    if new_settings.as_bytes() == settings {
        return Ok(());
    }

    save_settings(&db, &new_settings)?;
    Ok(())
}

fn transfer_assignments(db_path: &Path, opts: TransferAssignments, db: rusqlite::Connection, settings: Vec<u8>) -> Result<()> {
    let mut settings: Settings = serde_json::from_slice(&settings)?;
    if !opts.dry_run {
        backup_database(db_path, &db)?;
    }

    for profile in settings.profiles.values_mut() {
        // Gather and clone source assignments
        let mut new_assignments: Vec<Assignment> = profile.assignments.iter()
            // Get only assignments for source device, leave slot suffix only
            .filter_map(|a| {
                let (device, button) = a.slot_id.split_once('_')?;
                (device == opts.from).then(|| Assignment { slot_id: format!("{}_{}", opts.to, button), ..a.clone()})
            })
            .collect();
        // Remove all existing assignments for target device.
        profile.assignments.retain(|a| a.slot_id.split_once('_').is_some_and(|(device, _)| device != opts.to));
        // Append new assignemnts.
        profile.assignments.append(&mut new_assignments);
    }

    let settings = serde_json::to_string_pretty(&settings)?;
    if opts.dry_run {
        println!("{}", settings);
    } else {
        save_settings(&db, &settings)?;

        restart_logi_agent()?;
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
