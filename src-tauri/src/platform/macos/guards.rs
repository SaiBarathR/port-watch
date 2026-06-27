use std::path::Path;

use crate::platform::path_validation;

fn is_protected_canonical(path: &Path) -> bool {
    let normalized = path.to_string_lossy();
    if normalized.starts_with("/usr/local/") {
        return false;
    }

    const PROTECTED_PREFIXES: &[&str] = &[
        "/System",
        "/usr",
        "/bin",
        "/sbin",
        "/Library",
    ];

    PROTECTED_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

#[allow(dead_code)]
pub fn is_protected_path(path: &str) -> bool {
    if path.trim().starts_with("/usr/local/") {
        return false;
    }

    path_validation::resolve_existing_path(path)
        .map(|canonical| is_protected_canonical(&canonical))
        .unwrap_or_else(|_| {
            const PROTECTED_PREFIXES: &[&str] = &[
                "/System",
                "/usr",
                "/bin",
                "/sbin",
                "/Library",
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

#[allow(dead_code)]
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
    fn blocks_system_paths() {
        assert!(is_protected_path("/System/Library/LaunchDaemons/foo"));
        assert!(is_protected_path("/usr/bin/python3"));
    }

    #[test]
    fn allows_usr_local() {
        assert!(!is_protected_path("/usr/local/bin/node"));
    }

    #[test]
    fn allows_user_home() {
        let path = Path::new(user_home()).join("port-watch-guard-user-test");
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        assert!(is_user_allowed_path(path.to_str().unwrap()));
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn rejects_parent_traversal() {
        let home = user_home();
        let base = Path::new(home).join("port-watch-traversal-test");
        let nested = base.join("deep/nested");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&nested).unwrap();
        let escape = nested.join("../../../../../../../etc/passwd");
        if escape.exists() {
            assert!(validate_delete_path(escape.to_str().unwrap()).is_err());
        }
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_symlink_escape() {
        let home = user_home();
        let temp = Path::new(&home).join("port-watch-guard-test");
        let outside = Path::new("/tmp/port-watch-guard-outside");
        let _ = fs::remove_dir_all(&temp);
        let _ = fs::remove_dir_all(outside);
        fs::create_dir_all(&temp).unwrap();
        fs::create_dir_all(outside).unwrap();
        let link = temp.join("escape-link");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.as_os_str(), &link).unwrap();
            assert!(validate_delete_path(link.to_str().unwrap()).is_err());
        }
        let _ = fs::remove_dir_all(&temp);
        let _ = fs::remove_dir_all(outside);
    }
}
