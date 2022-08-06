use std::{env, io};
use std::error::Error;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use texture_stacker::Config;

mod interop;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn Error>>;

macro_rules! log_error {
    ($fmt:literal) => {
        std::eprintln!(std::concat!("\x1b[31m[ERROR]\x1b[0m ", $fmt));
    };
    ($fmt:literal, $($arg:tt)*) => {
        std::eprintln!(std::concat!("\x1b[31m[ERROR]\x1b[0m ", $fmt), $($arg)*);
    };
}

fn main() {
    interop::enable_virtual_terminal_processing();
    setup_panic_handler();

    println!("Hello, world!");

    if let Err(err) = run() {
        log_error!("Critical error: {}", err);
    }

    prompt_for_string("Press enter to close this window...").unwrap();
}

fn run() -> std::result::Result<(), Box<dyn Error>> {
    let input_directory = get_input_directory()?;

    if !Path::new(&input_directory).is_dir() {
        log_error!("The specified input directory is not valid.");
        exit_blocking(1);
    }

    let output_texture_name = get_output_texture_name()?;
    let keep_mask_alpha = get_keep_mask_alpha()?;

    // Load config file.
    let mut config: Config = match texture_stacker::read_config_file() {
        Ok(config) => config.into(),
        Err(err) => {
            log_error!("Error loading config file \"{}\", using defaults.", err);
            Config::default()
        }
    };

    // Apply options from user.
    config.output_texture_name = output_texture_name;
    config.keep_mask_alpha = keep_mask_alpha;

    // Validate settings.
    if config.suffixes.is_empty() {
        log_error!("No suffixes specified in config.");
        exit_blocking(1);
    }

    config.input_directory = input_directory;

    let start_time = Instant::now();
    texture_stacker::run(&config)?;
    println!("Finished in {} s", start_time.elapsed().as_secs_f32());

    Ok(())
}

fn get_input_directory() -> Result<String> {
    // Use program argument if specified, otherwise prompt.
    match env::args().nth(1) {
        None =>
            Ok(prompt_for_string("Input directory? ")?
                .trim_matches('"')
                .to_owned()),
        Some(arg) => Ok(arg)
    }
}

fn get_output_texture_name() -> Result<String> {
    Ok(prompt_for_string("Output texture name? ")?
        // Make sure it does not contain path separators.
        .trim_matches(&['/', '\\'] as &[char])
        .replace('/', "_")
        .replace('\\', "_"))
}

fn get_keep_mask_alpha() -> Result<bool> {
    let response = prompt_for_string("Keep alpha channel? (Y/n) ")?;
    match response.trim() {
        "Y" | "y" => Ok(true),
        _ => Ok(false),
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
