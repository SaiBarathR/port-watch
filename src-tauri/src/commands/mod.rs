pub mod cli_install;
pub mod filesystem;
pub mod notifications;
pub mod process;
pub mod settings;
pub mod workflow;

/// Runs work that can take a while (a stop waiting out its grace period, a
/// large folder going to the Trash, an editor starting) off the main thread.
pub(crate) async fn blocking<T: Send + 'static>(
    what: &str,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("{what} task failed: {e}"))?
}
