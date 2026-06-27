// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "check" {
        port_watch_lib::cli::run_check(&args[2..]);
        return;
    }

    port_watch_lib::run();
}
