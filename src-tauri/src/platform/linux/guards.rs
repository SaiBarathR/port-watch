use std::path::Path;

pub fn is_protected_canonical(path: &Path) -> bool {
    let normalized = path.to_string_lossy();
    if normalized.starts_with("/usr/local/") {
        return false;
    }

    const PROTECTED_PREFIXES: &[&str] = &["/usr", "/bin", "/sbin", "/lib", "/lib64", "/opt"];

    PROTECTED_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_system_paths() {
        assert!(is_protected_canonical(Path::new("/usr/bin/python3")));
        assert!(is_protected_canonical(Path::new("/opt/app")));
    }

    #[test]
    fn allows_usr_local() {
        assert!(!is_protected_canonical(Path::new("/usr/local/bin/node")));
    }
}
