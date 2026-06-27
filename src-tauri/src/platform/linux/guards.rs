use std::path::Path;

use crate::platform::path_validation;

fn is_protected_canonical(path: &Path) -> bool {
    let normalized = path.to_string_lossy();
    if normalized.starts_with("/usr/local/") {
        return false;
    }

    const PROTECTED_PREFIXES: &[&str] = &[
        "/usr",
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/opt",
    ];

    PROTECTED_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

pub fn is_protected_path(path: &str) -> bool {
    if path.trim().starts_with("/usr/local/") {
        return false;
    }

    path_validation::resolve_existing_path(path)
        .map(|canonical| is_protected_canonical(&canonical))
        .unwrap_or_else(|_| {
            const PROTECTED_PREFIXES: &[&str] = &[
                "/usr",
                "/bin",
                "/sbin",
                "/lib",
                "/lib64",
                "/opt",
            ];
            let normalized = path.trim();
            if normalized.starts_with("/usr/local/") {
                return false;
            }
            PROTECTED_PREFIXES
                .iter()
                .any(|prefix| normalized.starts_with(prefix))
        })
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
