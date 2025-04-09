use std::collections::BTreeMap;

use serde::{de::Error as _, Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Deserialize, Serialize)]
#[serde(try_from="RawSettings")]
#[serde(into="RawSettings")]
pub struct Settings {
    pub profile_keys: Vec<String>,
    pub profiles: BTreeMap<String, Profile>,
    pub ever_connected_devices: EverConnectedDevices,
    pub migration_report: MigrationReport,

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
    pub profile_keys: Vec<String>,
    pub ever_connected_devices: EverConnectedDevices,
    pub migration_report: MigrationReport,

    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct EverConnectedDevices {
    pub devices: Vec<ConnectedDevice>,

    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ConnectedDevice {
    #[serde(rename="connectionType")]
    pub connection_type: Option<String>,
    #[serde(rename="deviceModel")]
    pub device_model: String,
    #[serde(rename="deviceType")]
    pub device_type: String,
    #[serde(rename="slotPrefix")]
    pub slot_prefix: String,

    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct MigrationReport {
    pub devices: Vec<MigrationDevice>,

    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct MigrationDevice {
    #[serde(rename="deviceName")]
    pub device_name: String,
    #[serde(rename="modelId")]
    pub model_id: String,

    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Profile {
    pub assignments: Vec<Assignment>,

    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Assignment {
    #[serde(rename="slotId")]
    pub slot_id: String,

    #[serde(flatten)]
    pub rest: Map<String, Value>,
}
