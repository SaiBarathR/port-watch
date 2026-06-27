use std::path::PathBuf;
use std::sync::OnceLock;

static USER_HOME: OnceLock<String> = OnceLock::new();

pub fn user_home() -> &'static str {
    USER_HOME.get_or_init(|| {
        dirs::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .or_else(|| std::env::var("HOME").ok())
            .or_else(|| std::env::var("USERPROFILE").ok())
            .unwrap_or_default()
    })
}

const PROJECT_MARKERS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
    "Gemfile",
    "pom.xml",
];

pub fn infer_project_root(path: &str) -> String {
    if path.trim().is_empty() {
        return String::new();
    }

    let mut current = PathBuf::from(path);
    if current.is_file() {
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        }
    }

    loop {
        if PROJECT_MARKERS
            .iter()
            .any(|marker| current.join(marker).is_file())
        {
            return current.to_string_lossy().into_owned();
        }

        if !current.pop() {
            break;
        }
    }

    path.trim_end_matches(['/', '\\']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn user_home_is_non_empty() {
        assert!(!user_home().is_empty());
    }

    #[test]
    fn infer_project_root_finds_package_json() {
        let temp = std::env::temp_dir().join("port-watch-test-project");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join("src/nested")).unwrap();
        fs::write(temp.join("package.json"), "{}").unwrap();

        let nested = temp.join("src/nested").to_string_lossy().into_owned();
        let root = infer_project_root(&nested);
        assert_eq!(root, temp.to_string_lossy().into_owned());

        let _ = fs::remove_dir_all(&temp);
    }
}
