use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub use crate::config::*;
use crate::processing::{combine_texture_sets, InputTextureSet, ProcessConfig};
use crate::util::{log_warn, suffix_from_filename};

mod processing;
mod util;
mod config;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn Error>>;

const DEFAULT_OUTPUT_DIRECTORY_NAME: &str = "Combined";

pub fn run(config: &Config, progress_handler: Option<Box<dyn Fn(f32)>>) -> Result<()> {
    if config.suffixes.is_empty() {
        return Err("No suffixes specified.".into());
    }

    let input_directory = PathBuf::from(&config.input_directory);
    if !input_directory.is_dir() {
        return Err("The specified input directory is not valid.".into());
    }

    let output_directory = match &config.output_directory {
        None => {
            // Output will be in a subdirectory to the input dir.
            let mut buf = PathBuf::new();
            buf.push(&input_directory);
            buf.push(DEFAULT_OUTPUT_DIRECTORY_NAME);
            buf
        }
        Some(path) => PathBuf::from(path),
    };

    if !output_directory.is_dir() {
        fs::create_dir(&output_directory)?;
    }

    // Gather input sets from the input directory.
    let mut inputs =
        gather_texture_sets_from_directory(&input_directory, &config.suffixes)?;

    // Remove invalid texture sets from the list.
    inputs.retain(|set| {
        // Make sure the first texture type is given as this will be used for the mask.
        let valid = set.textures.len() > 0 && set.textures[0].is_some();
        if !valid {
            log_warn!(
                "Unable to compute mask for texture set '{}' because the first texture type '{}' is missing. This texture set will be skipped.",
                set.name,
                &config.suffixes[0]);
        }

        valid
    });

    // Process all input files.
    let config = ProcessConfig {
        keep_mask_alpha: config.keep_mask_alpha,
        output_masks: config.output_masks,
        suffixes: config.suffixes.clone(),
        output_texture_name: PathBuf::from(&config.output_texture_name),
        output_directory: output_directory.clone(),
        progress_handler,
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
