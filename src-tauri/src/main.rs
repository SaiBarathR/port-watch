// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// A windows_subsystem = "windows" binary has no console, so println!/eprintln!
// from the CLI subcommands would vanish. Attach to the invoking shell's
// console before the first print.
#[cfg(windows)]
fn attach_parent_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_cli = args.len() >= 2 && (args[1] == "install-cli" || args[1] == "check");

    #[cfg(windows)]
    if is_cli {
        attach_parent_console();
    }

    if is_cli {
        if args[1] == "install-cli" {
            port_watch_lib::cli_install::run_install_cli();
        } else {
            port_watch_lib::cli::run_check(&args[2..]);
        }
        return;
    }

    port_watch_lib::run();
}
