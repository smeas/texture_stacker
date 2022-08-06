use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::processing::{combine_texture_sets, InputTextureSet};
use crate::util::{log_warn, suffix_from_filename};

mod processing;
mod util;
mod config;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn Error>>;

const COMBINED_DIRECTORY_NAME: &str = "Combined";

/// Main API
pub struct TextureStacker {
    // Options
    pub keep_mask_alpha: bool,
    pub output_masks: bool,
    pub suffixes: Vec<String>,
    pub output_texture_name: String,
}

impl TextureStacker {
    /// Initialize API and load config file.
    pub fn new() -> Self {
        Self {
            keep_mask_alpha: false,
            output_masks: false,
            suffixes: vec![
                "_D".to_owned(),
                "_N".to_owned(),
                "_E".to_owned(),
                "_M".to_owned(),
            ],
            output_texture_name: "Output".to_owned(),
        }
    }

    pub fn load_config_file(&mut self) -> Result<()> {
        let conf = config::load_config_file()?;
        self.output_masks = conf.output_masks;
        self.suffixes = conf.suffixes;
        if let Some(name) = conf.output_texture_name {
            self.output_texture_name = name;
        }

        Ok(())
    }

    pub fn run_on_directory(&mut self, input_directory: impl AsRef<Path>) -> Result<()> {
        if self.suffixes.is_empty() {
            return Err("No suffixes specified.".into());
        }

        let input_directory = input_directory.as_ref();
        if !input_directory.is_dir() {
            return Err("The specified input directory is not valid.".into());
        }

        // Output will be in a subdirectory to the input dir.
        let mut output_directory = PathBuf::new();
        output_directory.push(&input_directory);
        output_directory.push(COMBINED_DIRECTORY_NAME);

        if !output_directory.is_dir() {
            fs::create_dir(&output_directory)?;
        }

        // Gather input sets from the input directory.
        let mut inputs =
            gather_texture_sets_from_directory(&input_directory, &self.suffixes)?;

        // Remove invalid texture sets from the list.
        inputs.retain(|set| {
            // Make sure the first texture type is given as this will be used for the mask.
            let valid = set.textures.len() > 0 && set.textures[0].is_some();
            if !valid {
                log_warn!(
                "Unable to compute mask for texture set '{}' because the first texture type '{}' is missing. This texture set will be skipped.",
                set.name,
                &self.suffixes[0]);
            }

            valid
        });

        // Process all input files.
        let config = Config {
            keep_mask_alpha: self.keep_mask_alpha,
            output_masks: self.output_masks,
            suffixes: self.suffixes.clone(),
            output_texture_name: PathBuf::from(&self.output_texture_name),
            output_directory: output_directory.clone(),
        };

        combine_texture_sets(&inputs, &config)?;

        // Open the destination directory when completed.
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("explorer")
                .arg(&output_directory)
                .output();
        };

        Ok(())
    }
}

fn collect_and_group_files_by_name<P: AsRef<Path>>(
    directory: &P,
) -> Result<BTreeMap<String, Vec<String>>> {
    let directory = directory.as_ref();
    assert!(directory.is_dir());

    // Here we store a mapping of: texture name -> list of textures with that name.
    // Using a BTreeMap instead of a HashMap here to have the items be sorted by key. This helps
    // make sure we get a consistent result when processing the textures later.
    let mut map = BTreeMap::<String, Vec<String>>::new();

    for entry in directory.read_dir()? {
        let entry = entry?;
        let path = entry.path();

        if path.extension() != Some("png".as_ref()) {
            continue;
        }

        if let Some(stem) = path.file_stem() {
            let stem = stem.to_string_lossy();

            if let Some(pos) = stem.rfind('_') {
                let pre = &stem[..pos];

                match map.get_mut(pre) {
                    Some(vec) => {
                        vec.push(path.to_string_lossy().into_owned());
                    }
                    None => {
                        let mut vec = Vec::new();
                        vec.push(path.to_string_lossy().to_string());
                        map.insert(pre.to_owned(), vec);
                    }
                }
            }
        }
    }

    Ok(map)
}

fn gather_texture_sets_from_directory<P, S>(
    path: &P,
    suffixes: &[S],
) -> Result<Vec<InputTextureSet>>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
{
    let files = collect_and_group_files_by_name(path)?;
    let mut output: Vec<InputTextureSet> = Vec::new();

    for (name, textures) in &files {
        let mut texture_set = InputTextureSet {
            name: name.clone(),
            textures: vec![None; suffixes.len()],
        };

        for (i, suffix) in suffixes.iter().enumerate() {
            if let Some(file) = textures
                .iter()
                .find(|filename| suffix_from_filename(filename) == Some(suffix.as_ref()))
            {
                texture_set.textures[i] = Some(file.clone());
            }
        }

        output.push(texture_set);
    }

    Ok(output)
}
