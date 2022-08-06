use std::{env, fs};
use std::path::PathBuf;
use serde::Deserialize;
use crate::Result;
use crate::util::log_info;

pub(crate) fn load_config_file() -> Result<ConfigFile> {
    let mut path = PathBuf::new();
    path.push(env::current_exe()?);
    path.pop();
    path.push("../config.toml");

    if path.is_file() {
        let raw = fs::read_to_string(path)?;
        Ok(toml::from_str(&raw)?)
    } else {
        log_info!("Config file not found, using default configuration.");
        Ok(Default::default())
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConfigFile {
    #[serde(default)]
    pub suffixes: Vec<String>,

    // Debug options
    #[serde(default)]
    pub output_masks: bool,

    pub output_texture_name: Option<String>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            suffixes: vec![
                "_D".to_owned(),
                "_N".to_owned(),
                "_E".to_owned(),
                "_M".to_owned(),
            ],
            output_masks: false,
            output_texture_name: None,
        }
    }
}