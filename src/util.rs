macro_rules! log_info {
    ($fmt:literal) => {
        std::println!($fmt);
    };
    ($fmt:literal, $($arg:tt)*) => {
        std::println!($fmt, $($arg)*);
    };
}

macro_rules! log_warn {
    ($fmt:literal) => {
        std::eprintln!(std::concat!("\x1b[33m[WARN]\x1b[0m ", $fmt));
    };
    ($fmt:literal, $($arg:tt)*) => {
        std::eprintln!(std::concat!("\x1b[33m[WARN]\x1b[0m ", $fmt), $($arg)*);
    };
}

macro_rules! log_error {
    ($fmt:literal) => {
        std::eprintln!(std::concat!("\x1b[31m[ERROR]\x1b[0m ", $fmt));
    };
    ($fmt:literal, $($arg:tt)*) => {
        std::eprintln!(std::concat!("\x1b[31m[ERROR]\x1b[0m ", $fmt), $($arg)*);
    };
}

pub(crate) use log_error;
pub(crate) use log_info;
pub(crate) use log_warn;
