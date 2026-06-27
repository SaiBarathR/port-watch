use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct AppSettings {
    inner: Mutex<AppSettingsInner>,
}

#[derive(Debug, Default)]
struct AppSettingsInner {
    allow_system_process_actions: bool,
}

impl AppSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_allow_system_process_actions(&self, allow: bool) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.allow_system_process_actions = allow;
        }
    }

    pub fn allow_system_process_actions(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.allow_system_process_actions)
            .unwrap_or(false)
    }
}
