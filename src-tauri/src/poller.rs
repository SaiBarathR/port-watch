use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::scanner::{scan_listening_ports, PortProcess};

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortsUpdatedPayload {
    pub processes: Vec<PortProcess>,
    pub error: Option<String>,
}

#[derive(Debug)]
struct PollerInner {
    last_result: Vec<PortProcess>,
    last_error: Option<String>,
    last_scan_at: Option<Instant>,
    in_flight: bool,
    include_udp: bool,
    interval_ms: u64,
    refresh_paused: bool,
    generation: u64,
}

impl Default for PollerInner {
    fn default() -> Self {
        Self {
            last_result: Vec::new(),
            last_error: None,
            last_scan_at: None,
            in_flight: false,
            include_udp: false,
            interval_ms: 3000,
            refresh_paused: false,
            generation: 0,
        }
    }
}

pub struct PortPoller {
    inner: Mutex<PollerInner>,
}

impl PortPoller {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(PollerInner::default()),
        }
    }
}

fn read_cache(app: &AppHandle) -> Result<PortsUpdatedPayload, String> {
    let poller = app.state::<PortPoller>();
    let inner = poller
        .inner
        .lock()
        .map_err(|_| "Port poller lock poisoned".to_string())?;

    Ok(PortsUpdatedPayload {
        processes: inner.last_result.clone(),
        error: inner.last_error.clone(),
    })
}

#[tauri::command]
pub async fn get_listening_ports(app: AppHandle) -> Result<PortsUpdatedPayload, String> {
    const MAX_WAIT: Duration = Duration::from_secs(8);
    let started = Instant::now();
    let mut triggered_scan = false;

    loop {
        let snapshot = read_cache(&app)?;
        if !snapshot.processes.is_empty() || snapshot.error.is_some() {
            return Ok(snapshot);
        }

        let in_flight = {
            let poller = app.state::<PortPoller>();
            let inner = poller
                .inner
                .lock()
                .map_err(|_| "Port poller lock poisoned".to_string())?;
            inner.in_flight
        };

        if !in_flight && !triggered_scan {
            triggered_scan = true;
            spawn_scan(app.clone());
        }

        if started.elapsed() >= MAX_WAIT {
            return Ok(snapshot);
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tauri::command]
pub fn set_scan_settings(
    app: AppHandle,
    interval_ms: u64,
    include_udp: bool,
) -> Result<(), String> {
    let poller = app.state::<PortPoller>();
    let mut inner = poller
        .inner
        .lock()
        .map_err(|_| "Port poller lock poisoned".to_string())?;

    let settings_changed = inner.interval_ms != interval_ms || inner.include_udp != include_udp;

    inner.interval_ms = interval_ms;
    inner.include_udp = include_udp;

    if settings_changed {
        inner.generation = inner.generation.wrapping_add(1);
        let generation = inner.generation;
        drop(inner);
        spawn_poller_loop(app.clone(), generation);
        spawn_scan(app);
    }

    Ok(())
}

#[tauri::command]
pub fn set_refresh_paused(app: AppHandle, paused: bool) -> Result<(), String> {
    let poller = app.state::<PortPoller>();
    let mut inner = poller
        .inner
        .lock()
        .map_err(|_| "Port poller lock poisoned".to_string())?;
    inner.refresh_paused = paused;
    Ok(())
}

#[tauri::command]
pub fn trigger_port_scan(app: AppHandle) -> Result<(), String> {
    spawn_scan(app);
    Ok(())
}

pub fn start_poller(app: AppHandle) {
    spawn_scan(app.clone());
    let generation = {
        let poller = app.state::<PortPoller>();
        let inner = poller.inner.lock().expect("poller lock");
        inner.generation
    };
    spawn_poller_loop(app, generation);
}

fn spawn_poller_loop(app: AppHandle, generation: u64) {
    tauri::async_runtime::spawn(async move {
        loop {
            let (interval_ms, current_generation, paused) = {
                let poller = app.state::<PortPoller>();
                let inner = poller.inner.lock().expect("poller lock");
                (
                    inner.interval_ms,
                    inner.generation,
                    inner.refresh_paused,
                )
            };

            if current_generation != generation {
                break;
            }

            if interval_ms == 0 || paused {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }

            let scan_started = Instant::now();
            run_scan(&app).await;

            if {
                let poller = app.state::<PortPoller>();
                let inner = poller.inner.lock().expect("poller lock");
                inner.generation != generation
            } {
                break;
            }

            let elapsed = scan_started.elapsed();
            let wait = Duration::from_millis(interval_ms)
                .saturating_sub(elapsed)
                .max(Duration::from_millis(100));

            tokio::time::sleep(wait).await;
        }
    });
}

fn spawn_scan(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        run_scan(&app).await;
    });
}

async fn run_scan(app: &AppHandle) {
    let include_udp = {
        let poller = app.state::<PortPoller>();
        let mut inner = match poller.inner.lock() {
            Ok(inner) => inner,
            Err(_) => return,
        };

        if inner.in_flight {
            return;
        }

        inner.in_flight = true;
        inner.include_udp
    };

    let scan_result =
        tauri::async_runtime::spawn_blocking(move || scan_listening_ports(include_udp)).await;

    let (processes, error) = match scan_result {
        Ok(Ok(processes)) => (processes, None),
        Ok(Err(err)) => (Vec::new(), Some(err)),
        Err(err) => (Vec::new(), Some(format!("Scan task failed: {err}"))),
    };

    let payload = {
        let poller = app.state::<PortPoller>();
        let mut inner = match poller.inner.lock() {
            Ok(inner) => inner,
            Err(_) => return,
        };

        inner.in_flight = false;
        inner.last_scan_at = Some(Instant::now());
        inner.last_result = processes.clone();
        inner.last_error = error.clone();

        PortsUpdatedPayload {
            processes,
            error,
        }
    };

    let _ = app.emit("ports-updated", &payload);
}
