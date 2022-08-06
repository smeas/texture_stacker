mod interop;

use std::error::Error;
use std::{env, io};
use std::io::Write;
use texture_stacker::TextureStacker;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() {
    interop::enable_virtual_terminal_processing();
    setup_panic_handler();

    println!("Hello, world!");

    let argv: Vec<String> = env::args().collect();
    let input_directory = {
        if argv.len() > 1 {
            argv[1].clone()
        } else {
            prompt_for_string("Input directory? ")
                .unwrap()
                .trim_matches('"')
                .to_owned()
        }
    };

    let mut stacker = TextureStacker::new();
    stacker.load_config_file().unwrap();
    stacker.run_on_directory(&input_directory).unwrap();

    prompt_for_string("Press enter to close this window...").unwrap();
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

// fn exit_blocking(code: i32) -> ! {
//     ask_to_close_window();
//     std::process::exit(code);
// }

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
