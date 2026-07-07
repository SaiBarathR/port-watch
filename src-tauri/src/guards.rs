use crate::platform;

pub fn resolve_delete_path(path: &str) -> Result<std::path::PathBuf, String> {
    platform::guards::resolve_delete_path(path)
}

pub fn resolve_permanent_delete(path: &str, confirmation: &str) -> Result<std::path::PathBuf, String> {
    platform::guards::resolve_permanent_delete(path, confirmation)
}
