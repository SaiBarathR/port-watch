//! One background task scans for listening ports and publishes what it finds.
//! Everything else talks to it through channels: settings and window
//! visibility arrive on a watch channel, "scan now" requests on a `Notify`,
//! and results leave on another watch channel.
//!
//! `Notify` keeps one permit, so any number of requests made while a scan is
//! running lead to exactly one more scan after it: a request is never dropped.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{watch, Notify};
use tokio::time::Instant;

use crate::scanner::{scan_listening_ports, PortProcess};

// With the window hidden nobody is watching the table, so periodic scans slow
// to this unless a watched port still wants prompt alerts.
const HIDDEN_INTERVAL: Duration = Duration::from_secs(15);
// A scan that overruns its interval is still followed by a pause.
const MIN_GAP: Duration = Duration::from_millis(100);
// How long a caller waits for a scan before giving up on it.
const SCAN_WAIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq)]
struct Config {
    interval_ms: u64,
    include_udp: bool,
    paused: bool,
    watch_while_hidden: bool,
    window_visible: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval_ms: 3000,
            include_udp: false,
            paused: false,
            watch_while_hidden: false,
            window_visible: true,
        }
    }
}

impl Config {
    /// Time between periodic scans; None while they are off or paused.
    fn interval(&self) -> Option<Duration> {
        if self.paused || self.interval_ms == 0 {
            return None;
        }

        let interval = Duration::from_millis(self.interval_ms);
        Some(if self.window_visible || self.watch_while_hidden {
            interval
        } else {
            interval.max(HIDDEN_INTERVAL)
        })
    }

    /// Whether the change from `previous` should be followed by a scan right
    /// away. Pausing and hiding the window only change the pace.
    fn calls_for_scan(&self, previous: &Config) -> bool {
        self.include_udp != previous.include_udp
            || self.interval_ms != previous.interval_ms
            || (previous.paused && !self.paused)
            || (!previous.window_visible && self.window_visible)
    }
}

/// The latest scan result. A failed scan keeps the processes of the last
/// good one and adds the error.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub processes: Arc<Vec<PortProcess>>,
    pub error: Option<String>,
    /// False until the first scan has finished.
    pub scanned: bool,
    /// Goes up by one each time the result changes. A reader that has seen
    /// revision N can tell a late copy of N-1 from news.
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct Progress {
    started: u64,
    completed: u64,
}

pub type ScanFuture = Pin<Box<dyn Future<Output = Result<Vec<PortProcess>, String>> + Send>>;

pub struct PortPoller {
    config: watch::Sender<Config>,
    snapshot: watch::Sender<Snapshot>,
    progress: watch::Sender<Progress>,
    kick: Notify,
}

impl PortPoller {
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    fn with_config(config: Config) -> Self {
        Self {
            config: watch::Sender::new(config),
            snapshot: watch::Sender::new(Snapshot {
                processes: Arc::new(Vec::new()),
                error: None,
                scanned: false,
                revision: 0,
            }),
            progress: watch::Sender::new(Progress::default()),
            kick: Notify::new(),
        }
    }

    pub fn processes(&self) -> Arc<Vec<PortProcess>> {
        self.snapshot.borrow().processes.clone()
    }

    pub fn find_by_pid(&self, pid: u32) -> Option<PortProcess> {
        self.snapshot
            .borrow()
            .processes
            .iter()
            .find(|process| process.pid == pid)
            .cloned()
    }

    /// Asks for a scan without waiting for it.
    pub fn request_scan(&self) {
        self.kick.notify_one();
    }

    /// Asks for a scan, waits for one that started after the request, and
    /// returns what there is then, so the caller sees the effect of whatever
    /// it just did. A scan already in flight does not count.
    pub async fn scan_now(&self) -> Result<Snapshot, String> {
        let mut progress = self.progress.subscribe();
        let target = progress.borrow().started + 1;
        self.request_scan();

        let waited = tokio::time::timeout(
            SCAN_WAIT,
            progress.wait_for(|progress| progress.completed >= target),
        )
        .await;
        match waited {
            Ok(Ok(_)) => Ok(self.snapshot.borrow().clone()),
            _ => Err("Scan timed out before completing".into()),
        }
    }

    /// The first finished scan, or what there is with an error if none
    /// finishes in time.
    pub async fn first_scan(&self) -> Snapshot {
        let mut snapshot = self.snapshot.subscribe();
        let waited =
            tokio::time::timeout(SCAN_WAIT, snapshot.wait_for(|snapshot| snapshot.scanned)).await;
        if let Ok(Ok(snapshot)) = waited {
            return snapshot.clone();
        }

        let mut snapshot = self.snapshot.borrow().clone();
        snapshot
            .error
            .get_or_insert_with(|| "Scan timed out before completing".into());
        snapshot
    }

    pub fn set_scan_settings(&self, interval_ms: u64, include_udp: bool, watch_while_hidden: bool) {
        self.update_config(|config| {
            config.interval_ms = interval_ms;
            config.include_udp = include_udp;
            config.watch_while_hidden = watch_while_hidden;
        });
    }

    pub fn set_paused(&self, paused: bool) {
        self.update_config(|config| config.paused = paused);
    }

    pub fn set_window_visible(&self, visible: bool) {
        self.update_config(|config| config.window_visible = visible);
    }

    fn update_config(&self, change: impl FnOnce(&mut Config)) {
        self.config.send_if_modified(|config| {
            let previous = config.clone();
            change(config);
            *config != previous
        });
    }

    /// The scan loop. It scans once at once, then whenever the interval
    /// elapses, a scan is requested, or a setting changes in a way that calls
    /// for one. `after_scan` runs after every scan and is told whether the
    /// result differs from the one before it.
    pub async fn run(
        &self,
        scan: impl Fn(bool) -> ScanFuture,
        after_scan: impl Fn(&Snapshot, bool),
    ) {
        let mut config_rx = self.config.subscribe();
        let mut config = config_rx.borrow_and_update().clone();

        loop {
            let started = Instant::now();
            self.progress.send_modify(|progress| progress.started += 1);
            let result = scan(config.include_udp).await;
            self.publish(result, &after_scan);
            self.progress
                .send_modify(|progress| progress.completed += 1);

            // Wait for a reason to scan again.
            loop {
                let tick = async {
                    match config.interval() {
                        Some(interval) => {
                            let due = (started + interval).max(Instant::now() + MIN_GAP);
                            tokio::time::sleep_until(due).await;
                        }
                        None => std::future::pending().await,
                    }
                };

                tokio::select! {
                    _ = tick => break,
                    _ = self.kick.notified() => break,
                    changed = config_rx.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        let next = config_rx.borrow_and_update().clone();
                        let scan_now = next.calls_for_scan(&config);
                        config = next;
                        if scan_now {
                            break;
                        }
                    }
                }
            }
        }
    }

    fn publish(
        &self,
        result: Result<Vec<PortProcess>, String>,
        after_scan: &impl Fn(&Snapshot, bool),
    ) {
        let previous = self.snapshot.borrow().clone();
        let (processes, error) = match result {
            // The same allocation when nothing changed.
            Ok(processes) if *previous.processes == processes => (previous.processes.clone(), None),
            Ok(processes) => (Arc::new(processes), None),
            Err(error) => (previous.processes.clone(), Some(error)),
        };

        let changed = !previous.scanned
            || error != previous.error
            || !Arc::ptr_eq(&processes, &previous.processes);
        let next = Snapshot {
            processes,
            error,
            scanned: true,
            revision: previous.revision + u64::from(changed),
        };
        if changed {
            self.snapshot.send_replace(next.clone());
        }
        after_scan(&next, changed);
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortsUpdatedPayload {
    pub processes: Vec<PortProcess>,
    pub error: Option<String>,
    pub revision: u64,
}

// The same shape, borrowed, so an event does not copy the list.
#[derive(Clone, serde::Serialize)]
struct PortsUpdatedEvent<'a> {
    processes: &'a [PortProcess],
    error: Option<&'a str>,
    revision: u64,
}

pub fn start_poller(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let poller = app.state::<PortPoller>();
        poller
            .run(
                |include_udp| {
                    Box::pin(async move {
                        tauri::async_runtime::spawn_blocking(move || {
                            scan_listening_ports(include_udp)
                        })
                        .await
                        .unwrap_or_else(|err| Err(format!("Scan task failed: {err}")))
                    })
                },
                |snapshot, changed| {
                    // A scan that found nothing new is not announced.
                    if changed {
                        let _ = app.emit(
                            "ports-updated",
                            PortsUpdatedEvent {
                                processes: &snapshot.processes,
                                error: snapshot.error.as_deref(),
                                revision: snapshot.revision,
                            },
                        );
                    }
                    // Every scan, though: the tray compares for itself, and
                    // retries a menu it failed to apply last time.
                    crate::tray::rebuild_tray_menu(&app);
                },
            )
            .await;
    });
}

impl From<Snapshot> for PortsUpdatedPayload {
    fn from(snapshot: Snapshot) -> Self {
        Self {
            processes: snapshot.processes.to_vec(),
            error: snapshot.error,
            revision: snapshot.revision,
        }
    }
}

#[tauri::command]
pub async fn get_listening_ports(app: AppHandle) -> Result<PortsUpdatedPayload, String> {
    Ok(app.state::<PortPoller>().first_scan().await.into())
}

#[tauri::command]
pub fn set_refresh_paused(app: AppHandle, paused: bool) -> Result<(), String> {
    app.state::<PortPoller>().set_paused(paused);
    Ok(())
}

// Resolves with the result once a scan that started after the call has
// finished. A scan that found nothing new sends no event, so this is how the
// caller learns it is done. The revision lets it drop a reply that arrives
// after a newer event.
#[tauri::command]
pub async fn trigger_port_scan(app: AppHandle) -> Result<PortsUpdatedPayload, String> {
    Ok(app.state::<PortPoller>().scan_now().await?.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::SystemKind;
    use crate::scanner::PortBinding;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use tokio::sync::Semaphore;

    fn listener(pid: u32, port: u16) -> PortProcess {
        PortProcess {
            id: String::new(),
            pid,
            name: "node".into(),
            user: "dev".into(),
            ports: vec![PortBinding {
                address: "*".into(),
                port,
                protocol: "TCP".into(),
            }],
            executable_path: "/usr/local/bin/node".into(),
            script_path: None,
            command_line: "node server.js".into(),
            working_directory: "/Users/dev/app".into(),
            project_root: "/Users/dev/app".into(),
            system_kind: SystemKind::User,
            is_system_service: false,
            started_at: 1_790_000_000,
            delete_blocked: None,
        }
    }

    type ScanResult = Result<Vec<PortProcess>, String>;

    // A scanner the test scripts: it records every call, hands out queued
    // results (repeating the last one), and can be made to hold each scan
    // until the test lets it finish.
    #[derive(Default)]
    struct FakeScanner {
        calls: Mutex<Vec<bool>>,
        results: Mutex<VecDeque<ScanResult>>,
        last: Mutex<Option<ScanResult>>,
        gate: Option<Semaphore>,
    }

    impl FakeScanner {
        fn returning(results: impl IntoIterator<Item = ScanResult>) -> Self {
            Self {
                results: Mutex::new(results.into_iter().collect()),
                ..Self::default()
            }
        }

        fn gated(mut self) -> Self {
            self.gate = Some(Semaphore::new(0));
            self
        }

        fn finish_one_scan(&self) {
            self.gate.as_ref().unwrap().add_permits(1);
        }

        fn calls(&self) -> Vec<bool> {
            self.calls.lock().unwrap().clone()
        }

        fn scan(self: &Arc<Self>, include_udp: bool) -> ScanFuture {
            let scanner = self.clone();
            Box::pin(async move {
                scanner.calls.lock().unwrap().push(include_udp);
                if let Some(gate) = &scanner.gate {
                    gate.acquire().await.unwrap().forget();
                }
                let next = scanner.results.lock().unwrap().pop_front();
                let mut last = scanner.last.lock().unwrap();
                if let Some(next) = next {
                    *last = Some(next);
                }
                last.clone().unwrap_or_else(|| Ok(Vec::new()))
            })
        }
    }

    struct Running {
        poller: Arc<PortPoller>,
        scanner: Arc<FakeScanner>,
        published: Arc<Mutex<Vec<Snapshot>>>,
        reported: Arc<Mutex<usize>>,
    }

    impl Running {
        fn start(config: Config, scanner: FakeScanner) -> Self {
            let poller = Arc::new(PortPoller::with_config(config));
            let scanner = Arc::new(scanner);
            let published = Arc::new(Mutex::new(Vec::new()));
            let reported = Arc::new(Mutex::new(0));

            tokio::spawn({
                let poller = poller.clone();
                let scanner = scanner.clone();
                let published = published.clone();
                let reported = reported.clone();
                async move {
                    poller
                        .run(
                            |include_udp| scanner.scan(include_udp),
                            |snapshot, changed| {
                                *reported.lock().unwrap() += 1;
                                if changed {
                                    published.lock().unwrap().push(snapshot.clone());
                                }
                            },
                        )
                        .await;
                }
            });

            Self {
                poller,
                scanner,
                published,
                reported,
            }
        }

        fn scans(&self) -> usize {
            self.scanner.calls().len()
        }

        fn published(&self) -> usize {
            self.published.lock().unwrap().len()
        }
    }

    fn manual() -> Config {
        Config {
            interval_ms: 0,
            ..Config::default()
        }
    }

    // Lets the loop run without moving the clock.
    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    async fn advance(seconds: u64) {
        tokio::time::sleep(Duration::from_secs(seconds)).await;
        settle().await;
    }

    #[tokio::test(start_paused = true)]
    async fn scans_at_startup_and_publishes_the_result() {
        let running = Running::start(
            Config::default(),
            FakeScanner::returning([Ok(vec![listener(1, 3000)])]),
        );

        let snapshot = running.poller.first_scan().await;

        assert!(snapshot.scanned);
        assert_eq!(snapshot.error, None);
        assert_eq!(*snapshot.processes, vec![listener(1, 3000)]);
        assert_eq!(running.published(), 1);
        assert_eq!(
            running.poller.find_by_pid(1).map(|process| process.pid),
            Some(1)
        );
        assert!(running.poller.find_by_pid(2).is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn a_scan_that_found_nothing_new_publishes_nothing() {
        let running = Running::start(
            Config::default(),
            FakeScanner::returning([Ok(vec![listener(1, 3000)])]),
        );

        advance(30).await;

        assert!(running.scans() >= 10, "{} scans", running.scans());
        assert_eq!(running.published(), 1);
        assert_eq!(running.poller.snapshot.borrow().revision, 1);
        // Every scan is still reported, changed or not: the tray uses that to
        // retry a menu it could not apply.
        assert_eq!(*running.reported.lock().unwrap(), running.scans());
    }

    #[tokio::test(start_paused = true)]
    async fn an_empty_first_scan_is_still_published() {
        let running = Running::start(manual(), FakeScanner::returning([Ok(Vec::new())]));
        settle().await;
        assert_eq!(running.published(), 1);
        assert!(running.poller.first_scan().await.scanned);
    }

    #[tokio::test(start_paused = true)]
    async fn a_changed_scan_is_published() {
        let running = Running::start(
            Config::default(),
            FakeScanner::returning([
                Ok(vec![listener(1, 3000)]),
                Ok(vec![listener(1, 3000)]),
                Ok(vec![listener(1, 3000), listener(2, 8080)]),
            ]),
        );

        advance(30).await;

        assert_eq!(running.published(), 2);
        assert_eq!(running.poller.processes().len(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_scan_keeps_the_last_data_and_reports_the_error_once() {
        let running = Running::start(
            Config::default(),
            FakeScanner::returning([
                Ok(vec![listener(1, 3000)]),
                Err("lsof did not finish within 10 s".to_string()),
                Err("lsof did not finish within 10 s".to_string()),
                Ok(vec![listener(1, 3000)]),
            ]),
        );

        advance(30).await;

        let published = running.published.lock().unwrap().clone();
        let revisions: Vec<u64> = published.iter().map(|snapshot| snapshot.revision).collect();
        assert_eq!(revisions, vec![1, 2, 3]);
        let errors: Vec<Option<&str>> = published
            .iter()
            .map(|snapshot| snapshot.error.as_deref())
            .collect();
        // Good, failed (once, though it failed twice), good again.
        assert_eq!(
            errors,
            vec![None, Some("lsof did not finish within 10 s"), None]
        );
        for snapshot in &published {
            assert_eq!(*snapshot.processes, vec![listener(1, 3000)]);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn manual_mode_scans_only_when_asked() {
        let running = Running::start(manual(), FakeScanner::default());

        advance(60).await;
        assert_eq!(running.scans(), 1);

        running.poller.request_scan();
        settle().await;
        assert_eq!(running.scans(), 2);
    }

    // The old poller returned early when a scan was in flight, so a refresh
    // asked for during a scan was lost: for good, in manual mode.
    #[tokio::test(start_paused = true)]
    async fn requests_during_a_scan_lead_to_exactly_one_more() {
        let running = Running::start(manual(), FakeScanner::default().gated());
        settle().await;
        assert_eq!(running.scans(), 1);

        for _ in 0..3 {
            running.poller.request_scan();
        }
        running.scanner.finish_one_scan();
        settle().await;
        assert_eq!(running.scans(), 2, "the follow-up scan should have started");

        running.scanner.finish_one_scan();
        advance(60).await;
        assert_eq!(running.scans(), 2, "three requests are one follow-up");
    }

    #[tokio::test(start_paused = true)]
    async fn scan_now_waits_for_a_scan_that_started_after_the_request() {
        let running = Running::start(manual(), FakeScanner::default().gated());
        settle().await;

        let waiter = tokio::spawn({
            let poller = running.poller.clone();
            async move { poller.scan_now().await }
        });
        settle().await;

        // The scan that was already running when the request came in.
        running.scanner.finish_one_scan();
        settle().await;
        assert!(!waiter.is_finished(), "an earlier scan must not satisfy it");
        assert_eq!(running.scans(), 2);

        running.scanner.finish_one_scan();
        let snapshot = waiter.await.unwrap().expect("the follow-up scan");
        assert!(snapshot.scanned);
        assert_eq!(snapshot.error, None);
    }

    // A refresh that fails in the webview (a timeout) leaves an error there
    // that no event will clear if the next scan finds nothing new. The
    // result handed back by the request is what clears it.
    #[tokio::test(start_paused = true)]
    async fn scan_now_returns_the_result_even_when_nothing_changed() {
        let running = Running::start(
            manual(),
            FakeScanner::returning([Ok(vec![listener(1, 3000)])]),
        );
        settle().await;
        assert_eq!(running.published(), 1);

        let snapshot = running.poller.scan_now().await.expect("scan");

        assert_eq!(running.published(), 1, "nothing new to announce");
        assert_eq!(*snapshot.processes, vec![listener(1, 3000)]);
        assert_eq!(snapshot.error, None);
        // Still the revision the event carried: nothing has changed since.
        assert_eq!(snapshot.revision, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn scan_now_gives_up_on_a_scan_that_never_finishes() {
        let running = Running::start(manual(), FakeScanner::default().gated());
        settle().await;

        let error = running.poller.scan_now().await.unwrap_err();
        assert!(error.contains("timed out"), "{error}");
    }

    #[tokio::test(start_paused = true)]
    async fn first_scan_reports_a_timeout_instead_of_waiting_forever() {
        let running = Running::start(manual(), FakeScanner::default().gated());

        let snapshot = running.poller.first_scan().await;

        assert!(!snapshot.scanned);
        assert!(snapshot.error.unwrap().contains("timed out"));
    }

    #[tokio::test(start_paused = true)]
    async fn pausing_stops_periodic_scans_and_resuming_scans_at_once() {
        let running = Running::start(Config::default(), FakeScanner::default());
        settle().await;
        assert_eq!(running.scans(), 1);

        running.poller.set_paused(true);
        advance(60).await;
        assert_eq!(running.scans(), 1, "pausing itself must not scan");

        // A request still gets through while paused.
        running.poller.request_scan();
        settle().await;
        assert_eq!(running.scans(), 2);

        running.poller.set_paused(false);
        settle().await;
        assert_eq!(running.scans(), 3);

        advance(30).await;
        assert!(running.scans() >= 12, "{} scans", running.scans());
    }

    #[tokio::test(start_paused = true)]
    async fn a_hidden_window_slows_the_scans_down() {
        let running = Running::start(Config::default(), FakeScanner::default());
        settle().await;

        running.poller.set_window_visible(false);
        settle().await;
        let before = running.scans();
        advance(60).await;
        let hidden = running.scans() - before;
        assert!((3..=5).contains(&hidden), "{hidden} scans in a minute");

        // Showing the window again scans straight away and restores the pace.
        let before = running.scans();
        running.poller.set_window_visible(true);
        settle().await;
        assert_eq!(running.scans(), before + 1);
        advance(60).await;
        assert!(running.scans() - before >= 20);
    }

    #[tokio::test(start_paused = true)]
    async fn a_hidden_window_keeps_the_pace_while_a_port_is_watched() {
        let running = Running::start(
            Config {
                watch_while_hidden: true,
                window_visible: false,
                ..Config::default()
            },
            FakeScanner::default(),
        );

        advance(60).await;

        assert!(running.scans() >= 20, "{} scans", running.scans());
    }

    #[tokio::test(start_paused = true)]
    async fn a_slow_interval_is_not_made_faster_by_hiding_the_window() {
        let running = Running::start(
            Config {
                interval_ms: 60_000,
                window_visible: false,
                ..Config::default()
            },
            FakeScanner::default(),
        );

        advance(150).await;

        assert_eq!(running.scans(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn turning_udp_on_rescans_with_it() {
        let running = Running::start(manual(), FakeScanner::default());
        settle().await;

        running.poller.set_scan_settings(0, true, false);
        settle().await;
        assert_eq!(running.scanner.calls(), vec![false, true]);

        // Setting the same values again changes nothing.
        running.poller.set_scan_settings(0, true, false);
        settle().await;
        assert_eq!(running.scans(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn changing_the_interval_scans_and_adopts_the_new_pace() {
        let running = Running::start(manual(), FakeScanner::default());
        settle().await;

        running.poller.set_scan_settings(10_000, false, false);
        settle().await;
        assert_eq!(running.scans(), 2);

        advance(60).await;
        assert_eq!(running.scans(), 8);
    }

    #[test]
    fn hidden_pace_is_the_slower_of_the_two() {
        let hidden = |interval_ms| Config {
            interval_ms,
            window_visible: false,
            ..Config::default()
        };
        assert_eq!(hidden(3_000).interval(), Some(HIDDEN_INTERVAL));
        assert_eq!(hidden(60_000).interval(), Some(Duration::from_secs(60)));
        assert_eq!(hidden(0).interval(), None);
        assert_eq!(
            Config::default().interval(),
            Some(Duration::from_millis(3_000))
        );
    }
}
