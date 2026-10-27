#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod presentation;
#[cfg(windows)]
mod windows_app;

#[cfg(windows)]
fn main() {
    let smoke = std::env::args().any(|arg| arg == "--smoke");
    if let Err(error) = windows_app::run(smoke) {
        if !smoke {
            windows_app::show_error(&error);
        }
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("RiichiScreenHelper requires Windows 10 or later");
    std::process::exit(1);
}
