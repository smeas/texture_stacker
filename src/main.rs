mod interop;
mod processing;
mod util;

use processing::{combine_texture_sets, Config, InputTextureSet};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Instant,
};
use util::{log_error, log_info};

pub(crate) type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    suffixes: Vec<String>,

    #[serde(default)]
    keep_mask_alpha: bool,

    // Debug options
    #[serde(default)]
    output_masks: bool,

    input_directory: Option<String>,
    output_texture_name: Option<String>,
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
            keep_mask_alpha: false,
            output_masks: false,
            input_directory: None,
            output_texture_name: None,
        }
    }
}

fn prompt_for_string(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_owned())
}

fn ask_to_close_window() {
    let _ = prompt_for_string("Press enter to close this window...");
}

fn exit_blocking(code: i32) -> ! {
    ask_to_close_window();
    std::process::exit(code);
}

fn setup_panic_handler() {
    use std::panic;

    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        print!("\x1b[31m[ERROR]\x1b[0m ");
        let _ = io::stdout().flush();
        default_hook(info);
        ask_to_close_window();
    }));
}

fn get_config() -> Result<ConfigFile> {
    let mut path = PathBuf::new();
    path.push(env::current_exe()?);
    path.pop();
    path.push("config.toml");

    if path.is_file() {
        let raw = fs::read_to_string(path)?;
        Ok(toml::from_str(&raw)?)
    } else {
        log_info!("Config file not found, using default configuration.");
        Ok(Default::default())
    }
}

fn suffix_from_filename(filename: &str) -> Option<&str> {
    if let Some(stem) = Path::new(filename).file_stem() {
        let stem = stem.to_str().unwrap();
        if let Some(pos) = stem.rfind('_') {
            return Some(&stem[pos..]);
        }
    }

    None
}

fn collect_and_group_files_by_name<P: AsRef<Path>>(
    directory: &P,
) -> Result<BTreeMap<String, Vec<String>>> {
    let directory = directory.as_ref();
    assert!(directory.is_dir()); // TODO

    // Here we store a mapping of: texture name -> list of textures with that name.
    // Using a BTreeMap instead of a HashMap here to have the items be sorted by key. This helps
    // make sure we get a consistent result when processing the textures later.
    let mut map = BTreeMap::<String, Vec<String>>::new();

    for entry in directory.read_dir()? {
        let entry = entry?; // TODO
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

fn main() {
    interop::enable_virtual_terminal_processing();
    setup_panic_handler();

    let argv: Vec<String> = env::args().collect();
    let config_file: ConfigFile = get_config().expect("failed to read config file");

    if config_file.suffixes.len() == 0 {
        log_error!("No suffixes specified in config.");
        exit_blocking(1);
    }

    // input_directory = config > args > prompt
    let input_directory = config_file.input_directory.unwrap_or_else(|| {
        if argv.len() > 1 {
            argv[1].clone()
        } else {
            prompt_for_string("Input directory? ").unwrap()
        }
    });

    if !Path::new(&input_directory).is_dir() {
        log_error!("The specified input directory is not valid.");
        exit_blocking(1);
    }

    // output_texture_name = config > prompt
    // Can be a relative path, so has to be unpacked appropriately.
    let mut output_texture_name = config_file
        .output_texture_name
        .unwrap_or_else(|| {
            prompt_for_string("Output texture name (relative to input directory)? ").unwrap()
        })
        // Make sure it does not start with a slash, as that could cause paths to be overwritten by an absolute later on.
        .trim_matches(&['/', '\\'] as &[char])
        .to_owned();

    let output_directory = {
        let mut output_directory_path = PathBuf::new();
        output_directory_path.push(&input_directory);

        let empty_path = Path::new("");
        let output_texture_path = Path::new(&output_texture_name);
        let parent_dir = output_texture_path.parent().unwrap_or(empty_path);
        if parent_dir != empty_path {
            output_directory_path.push(&parent_dir);
            fs::create_dir_all(&output_directory_path).unwrap();

            output_texture_name = output_texture_path
                .file_name()
                .map(|s| s.to_str().unwrap())
                .unwrap_or("")
                .to_owned();
        }

        output_directory_path.to_str().unwrap().to_owned()
    };

    let config = Config {
        suffixes: config_file.suffixes,
        keep_mask_alpha: config_file.keep_mask_alpha,
        output_masks: config_file.output_masks,
        output_directory,
        output_texture_name,
    };

    let mut texture_sets =
        gather_texture_sets_from_directory(&input_directory, &config.suffixes).unwrap();

    // Remove invalid texture sets from the list.
    texture_sets.retain(|set| {
        // Make sure the first texture type is given as this will be used for the mask.
        let valid = set.textures.len() > 0 && set.textures[0].is_some();
        if !valid {
            log_error!(
                "Unable to compute mask for texture set '{}' because the first texture type '{}' is missing. This texture set will be skipped.",
                set.name,
                config.suffixes[0]);
        }

        valid
    });

    let start_time = Instant::now();

    combine_texture_sets(&texture_sets, &config).unwrap();

    log_info!("Finished in {} s", start_time.elapsed().as_secs_f32());

    prompt_for_string("Press enter to close this window...").unwrap();
}
