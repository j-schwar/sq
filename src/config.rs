use std::{
    collections::HashMap,
    env,
    fs::{self, OpenOptions},
    io::{Read, Seek, Write},
    path::PathBuf,
};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::db::DriverConfig;

/// Configuration options for a single connection profile.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub driver: DriverConfig,

    #[serde(skip)]
    name: String,
}

impl Profile {
    /// Gets the path to the schema file for this profile, if it can be determined.
    pub fn schema_path(&self) -> Option<PathBuf> {
        let path = config_dir().ok()?;
        Some(path.join(format!("{}.schema.json", self.name)))
    }
}

/// Configuration options for the whole application.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub profiles: HashMap<String, Profile>,
}

/// Returns the path to the directory where the configuration file is stored, creating it if
/// necessary.
#[tracing::instrument(err)]
pub fn config_dir() -> anyhow::Result<PathBuf> {
    let path = if let Ok(home) = env::var("SQ_HOME") {
        PathBuf::from(home)
    } else {
        let mut path = dirs::config_dir()
            .ok_or_else(|| anyhow!("Unable to determine config directory: SQ_HOME not set"))?;

        path.push("sq");
        path
    };

    fs::create_dir_all(&path)?;
    Ok(path)
}

/// Loads the configuration from the configuration file, creating it with default values if it
/// does not exist.
#[tracing::instrument(err)]
pub fn load() -> anyhow::Result<Config> {
    let mut path = config_dir()?;
    path.push("config.json");

    let mut options = OpenOptions::new();
    options.read(true);
    options.write(true);
    options.create(true);

    tracing::info!("Loading configuration from {}", path.to_string_lossy());
    let mut file = options.open(&path)?;

    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    if buf.is_empty() || buf.iter().all(u8::is_ascii_whitespace) {
        tracing::info!("File is empty, loading default configuration");
        file.set_len(0)?;
        file.rewind()?;
        serde_json::to_writer_pretty(&mut file, &Config::default())?;
        file.flush()?
    }

    file.rewind()?;
    let mut config: Config = serde_json::from_reader(file)?;

    // Ensure that profile names are set
    for (name, profile) in config.profiles.iter_mut() {
        profile.name = name.clone();
    }

    Ok(config)
}
