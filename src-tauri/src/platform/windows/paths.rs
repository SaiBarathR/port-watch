use std::path::{Path, PathBuf};

pub fn system_root() -> Option<PathBuf> {
    std::env::var("SystemRoot")
        .ok()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

pub fn program_files() -> Option<PathBuf> {
    std::env::var("ProgramFiles")
        .ok()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

pub fn program_files_x86() -> Option<PathBuf> {
    std::env::var("ProgramFiles(x86)")
        .ok()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

pub fn program_data() -> Option<PathBuf> {
    std::env::var("ProgramData")
        .ok()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

pub fn protected_prefixes() -> Vec<PathBuf> {
    [system_root(), program_files(), program_files_x86(), program_data()]
        .into_iter()
        .flatten()
        .collect()
}

pub fn canonicalize_if_exists(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        std::fs::canonicalize(path).ok()
    } else {
        Some(path.to_path_buf())
    }
}

pub fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    if path.starts_with(prefix) {
        return true;
    }

    match (canonicalize_if_exists(path), canonicalize_if_exists(prefix)) {
        (Some(path), Some(prefix)) => path.starts_with(&prefix),
        _ => false,
    }
}

pub fn is_protected_path(path: &Path) -> bool {
    protected_prefixes()
        .iter()
        .any(|prefix| path_starts_with(path, prefix))
}

pub fn is_microsoft_path(executable_path: &str, _command_line: &str) -> bool {
    let executable = PathBuf::from(executable_path);
    if !executable_path.is_empty() && is_protected_path(&executable) {
        return true;
    }

    // Match on the executable's own path segments rather than the free-form
    // command line, so a user project that merely mentions "microsoft" (e.g. a
    // folder named microsoft-graph-demo) is not flagged as a system service.
    let exe = executable_path.to_ascii_lowercase();
    exe.contains("\\windows\\system32\\") || exe.contains("\\program files\\windowsapps\\")
}
