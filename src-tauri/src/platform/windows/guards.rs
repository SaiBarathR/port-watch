use std::path::Path;

use crate::platform::path_validation;
use super::paths;

fn is_protected_canonical(path: &Path) -> bool {
    paths::is_protected_path(path)
}

pub fn is_protected_path(path: &str) -> bool {
    path_validation::resolve_existing_path(path)
        .map(|canonical| is_protected_canonical(&canonical))
        .unwrap_or_else(|_| {
            let normalized = Path::new(path.trim());
            paths::protected_prefixes()
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

pub fn resolve_delete_path(path: &str) -> Result<std::path::PathBuf, String> {
    path_validation::resolve_delete_path(path, is_protected_canonical)
}

pub fn validate_permanent_delete(path: &str, confirmation: &str) -> Result<(), String> {
    path_validation::validate_permanent_delete(path, confirmation, is_protected_canonical)
}

pub fn resolve_permanent_delete(path: &str, confirmation: &str) -> Result<std::path::PathBuf, String> {
    path_validation::resolve_permanent_delete(path, confirmation, is_protected_canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::user_home;
    use std::fs;
    use std::path::Path;

    #[test]
    fn blocks_program_files() {
        if let Some(program_files) = super::super::paths::program_files() {
            assert!(is_protected_path(
                program_files.join("Example").to_str().unwrap()
            ));
        }
    }

    #[test]
    fn blocks_system_root() {
        if let Some(system_root) = super::super::paths::system_root() {
            assert!(is_protected_path(
                system_root.join("System32").to_str().unwrap()
            ));
        }
    }

    #[test]
    fn allows_user_home() {
        let path = Path::new(user_home()).join("port-watch-guard-user-test");
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        assert!(is_user_allowed_path(path.to_str().unwrap()));
        let _ = fs::remove_dir_all(&path);
    }
}
