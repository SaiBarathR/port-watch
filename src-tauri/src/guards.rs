use crate::home::user_home;

const PROTECTED_PREFIXES: &[&str] = &[
    "/System",
    "/usr",
    "/bin",
    "/sbin",
    "/Library",
];

pub fn is_protected_path(path: &str) -> bool {
    let normalized = normalize_path(path);
    PROTECTED_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

pub fn is_user_allowed_path(path: &str) -> bool {
    let normalized = normalize_path(path);
    normalized.starts_with(user_home()) && !is_protected_path(path)
}

pub fn validate_delete_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("Path is empty".into());
    }

    if is_protected_path(path) {
        return Err(format!("Protected system path cannot be deleted: {path}"));
    }

    if !is_user_allowed_path(path) {
        return Err(format!(
            "Only paths under {} can be deleted: {path}",
            user_home()
        ));
    }

    Ok(())
}

pub fn validate_permanent_delete(path: &str, confirmation: &str) -> Result<(), String> {
    validate_delete_path(path)?;

    let expected = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("");

    if expected.is_empty() {
        return Err("Could not determine folder basename for confirmation".into());
    }

    if confirmation != expected {
        return Err(format!(
            "Confirmation must match folder name \"{expected}\""
        ));
    }

    Ok(())
}

fn normalize_path(path: &str) -> String {
    path.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_system_paths() {
        assert!(is_protected_path("/System/Library/LaunchDaemons/foo"));
        assert!(is_protected_path("/usr/bin/python3"));
        assert!(is_protected_path("/Library/Apple/usr/bin"));
    }

    #[test]
    fn allows_user_home() {
        let path = format!("{}/hermes-projects/hosted-web-projects", user_home());
        assert!(is_user_allowed_path(&path));
    }

    #[test]
    fn permanent_delete_requires_basename() {
        let path = format!("{}/hermes-projects/hosted-web-projects", user_home());
        assert!(validate_permanent_delete(&path, "hosted-web-projects").is_ok());
        assert!(validate_permanent_delete(&path, "wrong").is_err());
    }
}
