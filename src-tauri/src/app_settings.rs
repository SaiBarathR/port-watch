use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct AppSettings {
    inner: Mutex<AppSettingsInner>,
}

#[derive(Debug)]
struct AppSettingsInner {
    allow_system_process_actions: bool,
    use_https_for_localhost: bool,
    preferred_editor: String,
}

impl Default for AppSettingsInner {
    fn default() -> Self {
        Self {
            allow_system_process_actions: false,
            use_https_for_localhost: false,
            preferred_editor: "cursor".to_string(),
        }
    }
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

    pub fn set_use_https_for_localhost(&self, use_https: bool) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.use_https_for_localhost = use_https;
        }
    }

    pub fn use_https_for_localhost(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.use_https_for_localhost)
            .unwrap_or(false)
    }

    pub fn set_preferred_editor(&self, editor: String) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.preferred_editor = editor;
        }
    }

    pub fn preferred_editor(&self) -> String {
        self.inner
            .lock()
            .map(|inner| inner.preferred_editor.clone())
            .unwrap_or_else(|_| "cursor".to_string())
    }
}
