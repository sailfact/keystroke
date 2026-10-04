//! Saved progress and preferences, stored as JSON in the user's data directory.

use std::fs;
use std::io;
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::progress::Profile;

const FILE_NAME: &str = "progress.json";

/// Saves somewhere else instead, e.g. to try the game without touching your
/// real progress.
const DIR_OVERRIDE: &str = "KEYSTROKE_DATA_DIR";

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load());
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveData {
    pub version: u32,
    pub sound_on: bool,
    pub profile: Profile,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: 2,
            sound_on: true,
            profile: Profile::default(),
        }
    }
}

impl SaveData {
    pub fn from_json(text: &str) -> serde_json::Result<Self> {
        serde_json::from_str(text)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("save data always serializes")
    }
}

pub fn save_path() -> Option<PathBuf> {
    match std::env::var_os(DIR_OVERRIDE) {
        Some(dir) => Some(PathBuf::from(dir).join(FILE_NAME)),
        None => dirs::data_dir().map(|dir| dir.join("keystroke").join(FILE_NAME)),
    }
}

fn load() -> SaveData {
    let Some(path) = save_path() else {
        warn!("No data directory found; progress won't be saved");
        return SaveData::default();
    };
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return SaveData::default(),
        Err(err) => {
            warn!("Couldn't read {}: {err}", path.display());
            return SaveData::default();
        }
    };
    match SaveData::from_json(&text) {
        Ok(data) => data,
        Err(err) => {
            let backup = path.with_extension("json.bak");
            warn!(
                "{} is unreadable ({err}); moving it to {} and starting fresh",
                path.display(),
                backup.display()
            );
            if let Err(err) = fs::rename(&path, &backup) {
                warn!("Couldn't move it: {err}");
            }
            SaveData::default()
        }
    }
}

/// Writes progress to disk. Failures are logged rather than returned: a
/// failed save shouldn't interrupt practice.
pub fn persist(data: &SaveData) {
    if let Err(err) = write(data) {
        warn!("Couldn't save progress: {err}");
    }
}

fn write(data: &SaveData) -> io::Result<()> {
    let path = save_path().ok_or_else(|| io::Error::other("no data directory"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    // Write a temporary file and rename it over the old one, so a crash
    // can't leave a half-written save behind.
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, data.to_json())?;
    fs::rename(&temp, &path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let mut data = SaveData {
            sound_on: false,
            ..SaveData::default()
        };
        data.profile.unlocked = 9;
        data.profile.keys[0].add_sample(250.0);
        data.profile.best_wpm = 41.5;
        assert_eq!(SaveData::from_json(&data.to_json()).unwrap(), data);
    }

    #[test]
    fn corrupt_json_is_an_error() {
        assert!(SaveData::from_json("{ not json").is_err());
    }

    #[test]
    fn missing_fields_use_defaults() {
        assert_eq!(SaveData::from_json("{}").unwrap(), SaveData::default());
        let data = SaveData::from_json(r#"{"profile": {"unlocked": 8}}"#).unwrap();
        assert_eq!(data.profile.unlocked, 8);
        assert_eq!(data.profile.keys, Profile::default().keys);
    }
}
