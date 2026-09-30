// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use echo_lib::CliArgs;

fn main() {
    let cli_args = CliArgs::parse();

    // Headless CLI mode: the release GUI is a Windows-subsystem app with no
    // console, so attach to the launching terminal's console to make
    // stdout/stderr visible. No-op (harmless) if there is no parent console.
    #[cfg(target_os = "windows")]
    {
        if cli_args.transcribe_file.is_some() {
            use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
            unsafe {
                let _ = AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // DMABUF renderer causes crashes on various GPU/display server configurations
        // See: https://github.com/tauri-apps/tauri/issues/9394
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    echo_lib::run(cli_args)
}
