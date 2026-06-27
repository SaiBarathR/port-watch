use std::path::Path;

use crate::platform::path_validation;
use super::paths;

fn is_protected_canonical(path: &Path) -> bool {
    paths::is_protected_path(path)
}

pub fn is_protected_path(path: &str) -> bool {
    path_validation::resolve_existing_path(path)
        .map(|canonical| is_protected_canonical(&canonical))
        .unwrap_or(false)
}

pub fn is_user_allowed_path(path: &str) -> bool {
    path_validation::validate_delete_path(path, is_protected_canonical).is_ok()
}

pub fn validate_delete_path(path: &str) -> Result<(), String> {
    path_validation::validate_delete_path(path, is_protected_canonical)
}

pub fn validate_permanent_delete(path: &str, confirmation: &str) -> Result<(), String> {
    path_validation::validate_permanent_delete(path, confirmation, is_protected_canonical)
}
