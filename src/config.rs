use std::{env, fs};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::Result;

const DEFAULT_CONFIG_FILE_NAME: &str = "texture_stacker.toml";

#[derive(Debug, Clone)]
pub struct Config {
    pub keep_mask_alpha: bool,
    pub output_masks: bool,
    pub suffixes: Vec<String>,
    pub output_texture_name: String,
    pub input_directory: String,
    pub output_directory: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            keep_mask_alpha: false,
            output_masks: false,
            suffixes: vec![
                "_D".to_owned(),
                "_N".to_owned(),
                "_E".to_owned(),
                "_M".to_owned(),
            ],
            output_texture_name: "T_Combined".to_owned(),
            input_directory: String::new(),
            output_directory: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub suffixes: Vec<String>,

    #[serde(default)]
    pub output_masks: bool,

    #[serde(default)]
    pub keep_mask_alpha: bool,

    pub output_texture_name: Option<String>,
    pub input_directory: Option<String>,
}

impl Into<Config> for ConfigFile {
    fn into(self) -> Config {
        let mut config = Config {
            suffixes: self.suffixes,
            output_masks: self.output_masks,
            keep_mask_alpha: self.keep_mask_alpha,
            ..Config::default()
        };

        if let Some(output_texture_name) = self.output_texture_name {
            config.output_texture_name = output_texture_name;
        }

        if let Some(input_directory) = self.input_directory {
            config.input_directory = input_directory;
        }

        config
    }
}

impl From<Config> for ConfigFile {
    fn from(config: Config) -> Self {
        Self {
            suffixes: config.suffixes,
            output_masks: config.output_masks,
            keep_mask_alpha: config.keep_mask_alpha,
            output_texture_name: Some(config.output_texture_name),
            input_directory: Some(config.input_directory),
        }
    }
}

pub fn read_config_file() -> Result<ConfigFile> {
    read_config_from_path(get_default_config_path()?)
}

pub fn write_config_file(config: &ConfigFile) -> Result<()> {
    write_config_to_path(get_default_config_path()?, config)
}

pub fn read_config_from_path(path: impl AsRef<Path>) -> Result<ConfigFile> {
    let path = path.as_ref();
    if path.is_file() {
        let raw = fs::read_to_string(path)?;
        let config_file: ConfigFile = toml::from_str(&raw)?;
        Ok(config_file)
    } else {
        Err("Config file not found".into())
    }
}

pub fn write_config_to_path(path: impl AsRef<Path>, config: &ConfigFile) -> Result<()> {
    let raw = toml::to_string(config)?;
    fs::write(path, raw)?;
    Ok(())
}

fn get_default_config_path() -> Result<PathBuf> {
    let mut path = PathBuf::new();
    path.push(env::current_exe()?);
    path.pop();
    path.push(DEFAULT_CONFIG_FILE_NAME);
    Ok(path)
}