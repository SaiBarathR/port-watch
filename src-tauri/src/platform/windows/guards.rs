use std::path::Path;

use super::paths;

pub fn is_protected_canonical(path: &Path) -> bool {
    paths::is_protected_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_program_files() {
        if let Some(program_files) = paths::program_files() {
            assert!(is_protected_canonical(&program_files.join("Example")));
        }
    }

    #[test]
    fn blocks_system_root() {
        if let Some(system_root) = paths::system_root() {
            assert!(is_protected_canonical(&system_root.join("System32")));
        }
    }
}
