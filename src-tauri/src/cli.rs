use std::process;

use crate::scanner::{scan_listening_ports, PortProcess};

pub fn run_check(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: port-watch check <port> [--udp]");
        process::exit(2);
    }

    let port: u16 = match args[0].parse() {
        Ok(port) => port,
        Err(_) => {
            eprintln!("Invalid port: {}", args[0]);
            process::exit(2);
        }
    };

    let include_udp = args.get(1).map(|v| v == "--udp").unwrap_or(false);
    let processes = match scan_listening_ports(include_udp) {
        Ok(processes) => processes,
        Err(err) => {
            eprintln!("{err}");
            process::exit(2);
        }
    };

    let owners: Vec<&PortProcess> = processes
        .iter()
        .filter(|process| process.ports.iter().any(|binding| binding.port == port))
        .collect();

    if owners.is_empty() {
        process::exit(0);
    }

    println!(
        "{}",
        serde_json::to_string(&owners).unwrap_or_else(|_| "[]".into())
    );
    process::exit(1);
}
