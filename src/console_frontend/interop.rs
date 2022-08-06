#[cfg(windows)]
mod win {
    use std::os::raw::c_int;

    #[link(name = "kernel32")]
    extern "C" {
        fn GetStdHandle(nStdHandle: u32) -> usize;
        fn GetConsoleMode(hConsoleHandle: usize, lpMode: *mut u32) -> c_int;
        fn SetConsoleMode(hConsoleHandle: usize, dwMode: u32) -> c_int;
    }

    const INVALID_HANDLE: usize = -1isize as usize;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x4;

    pub fn enable_virtual_terminal_processing() -> bool {
        unsafe {
            let std_out = GetStdHandle(STD_OUTPUT_HANDLE);
            if std_out == INVALID_HANDLE {
                return false;
            }

            let mut console_mode = 0u32;
            if GetConsoleMode(std_out, &mut console_mode as *mut u32) == 0 {
                return false;
            }

            console_mode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            if SetConsoleMode(std_out, console_mode) == 0 {
                return false;
            }
        }

        return true;
    }
}

#[cfg(windows)]
pub fn enable_virtual_terminal_processing() -> bool {
    win::enable_virtual_terminal_processing()
}

#[cfg(not(windows))]
pub fn enable_virtual_terminal_processing() -> bool {
    true
}
