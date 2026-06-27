use crate::platform;

pub fn validate_delete_path(path: &str) -> Result<(), String> {
    platform::guards::validate_delete_path(path)
}

pub fn validate_permanent_delete(path: &str, confirmation: &str) -> Result<(), String> {
    platform::guards::validate_permanent_delete(path, confirmation)
}
