use std::path::{Path, PathBuf};

use crate::home::user_home;

pub fn canonical_home() -> Result<PathBuf, String> {
    let home = user_home();
    if home.is_empty() {
        return Err("Could not determine user home directory".into());
    }

    let path = PathBuf::from(home);
    if path.exists() {
        std::fs::canonicalize(&path).map_err(|err| format!("Failed to resolve home: {err}"))
    } else {
        Ok(path)
    }
}

pub fn resolve_existing_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Path is empty".into());
    }

    let path = Path::new(trimmed);
    if !path.exists() {
        return Err(format!("Path does not exist: {trimmed}"));
    }

    let metadata = std::fs::symlink_metadata(path)
        .map_err(|err| format!("Failed to inspect path: {err}"))?;
    if metadata.file_type().is_symlink() {
        return Err("Symlinks cannot be deleted".into());
    }

    std::fs::canonicalize(path).map_err(|err| format!("Failed to resolve path: {err}"))
}

// Deleting these top-level directories is never what "delete this project
// folder" means, even though they live under home.
const PROTECTED_HOME_CHILDREN: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Library",
    "Movies",
    "Music",
    "Pictures",
    "Public",
    "AppData",
    ".ssh",
    ".gnupg",
    ".config",
];

pub fn assert_delete_allowed(
    canonical: &Path,
    is_protected: fn(&Path) -> bool,
) -> Result<(), String> {
    if is_protected(canonical) {
        return Err(format!(
            "Protected system path cannot be deleted: {}",
            canonical.display()
        ));
    }

    let home = canonical_home()?;
    if !canonical.starts_with(&home) {
        return Err(format!(
            "Only paths under {} can be deleted: {}",
            home.display(),
            canonical.display()
        ));
    }

    if canonical == home {
        return Err("The home directory itself cannot be deleted".into());
    }

    if let Ok(relative) = canonical.strip_prefix(&home) {
        let mut components = relative.components();
        if let (Some(first), None) = (components.next(), components.next()) {
            if PROTECTED_HOME_CHILDREN
                .iter()
                .any(|name| first.as_os_str().eq_ignore_ascii_case(name))
            {
                return Err(format!(
                    "{} is a protected user directory and cannot be deleted",
                    canonical.display()
                ));
            }
        }
    }

    Ok(())
}

pub fn resolve_delete_path(path: &str, is_protected: fn(&Path) -> bool) -> Result<PathBuf, String> {
    let canonical = resolve_existing_path(path)?;
    assert_delete_allowed(&canonical, is_protected)?;
    Ok(canonical)
}

pub fn resolve_permanent_delete(
    path: &str,
    confirmation: &str,
    is_protected: fn(&Path) -> bool,
) -> Result<PathBuf, String> {
    let expected = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    if expected.is_empty() {
        return Err("Could not determine folder basename for confirmation".into());
    }

    if confirmation != expected {
        return Err(format!(
            "Confirmation must match folder name \"{expected}\""
        ));
    }

    resolve_delete_path(path, is_protected)
}

pub fn assert_system_actions_allowed(
    is_system_service: bool,
    allow_system_actions: bool,
) -> Result<(), String> {
    if is_system_service && !allow_system_actions {
        return Err(
            "System process actions are disabled. Enable them in Settings to continue.".into(),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn not_protected(_: &Path) -> bool {
        false
    }

    #[test]
    fn rejects_home_itself() {
        let home = canonical_home().unwrap();
        let err = assert_delete_allowed(&home, not_protected).unwrap_err();
        assert!(err.contains("home directory"));
    }

    #[test]
    fn rejects_protected_home_children() {
        let home = canonical_home().unwrap();
        let err = assert_delete_allowed(&home.join("Documents"), not_protected).unwrap_err();
        assert!(err.contains("protected user directory"));
        let err = assert_delete_allowed(&home.join(".ssh"), not_protected).unwrap_err();
        assert!(err.contains("protected user directory"));
    }

    #[test]
    fn allows_projects_inside_protected_children() {
        let home = canonical_home().unwrap();
        assert_delete_allowed(&home.join("Documents/my-project"), not_protected).unwrap();
        assert_delete_allowed(&home.join("Dev/my-project"), not_protected).unwrap();
    }

    #[test]
    fn rejects_symlinks() {
        let temp = std::env::temp_dir().join("port-watch-symlink-test");
        let target = temp.join("target");
        let link = temp.join("link");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&target).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();

        #[cfg(unix)]
        {
            let err = resolve_delete_path(link.to_str().unwrap(), not_protected).unwrap_err();
            assert!(err.contains("Symlinks"));
        }

        let _ = fs::remove_dir_all(&temp);
    }
}
