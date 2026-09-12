#![cfg_attr(windows, windows_subsystem = "windows")]

use nioh3_onefile_launcher::{launch, log_launch_failure};

fn main() {
    match launch(std::env::args_os().skip(1)) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            let log = log_launch_failure(&error);
            let message = format!(
                "Nioh 3 Studio could not start.\n\n{error}\n\nPlease download the complete portable EXE again if its contents are damaged.\n\nLauncher log: {}",
                log.map(|p| p.display().to_string()).unwrap_or_else(|| "unavailable".into())
            );
            #[cfg(windows)]
            {
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    MessageBoxW, MB_ICONERROR, MB_OK,
                };
                let text: Vec<u16> = message.encode_utf16().chain([0]).collect();
                let title: Vec<u16> = "Nioh 3 Studio".encode_utf16().chain([0]).collect();
                unsafe {
                    MessageBoxW(
                        std::ptr::null_mut(),
                        text.as_ptr(),
                        title.as_ptr(),
                        MB_OK | MB_ICONERROR,
                    );
                }
            }
            #[cfg(not(windows))]
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}
