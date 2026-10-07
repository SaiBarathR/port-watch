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

/// The folder a process's project lives in: the nearest one at or above
/// `path` that holds a project marker, else the folder `path` is or is in.
/// Empty when `path` does not say where that is.
pub fn infer_project_root(path: &str) -> String {
    // Without the working directory a script may be known only as
    // `server.js`, and a relative path would be looked up against wherever
    // this app happens to have been started.
    let mut current = PathBuf::from(path.trim());
    if !current.is_absolute() {
        return String::new();
    }

    // A script or a program is given when the working directory is unknown.
    // The folder is what every action on a row is for: a file can be neither
    // shown in the file manager as a folder nor opened in a terminal.
    if current.is_file() {
        current.pop();
    }
    let start = current.clone();

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

    start
        .to_string_lossy()
        .trim_end_matches(['/', '\\'])
        .to_string()
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

    #[test]
    fn a_file_with_no_project_around_it_gives_its_folder() {
        let temp = tempfile::tempdir().unwrap();
        let folder = fs::canonicalize(temp.path()).unwrap().join("scripts");
        fs::create_dir_all(&folder).unwrap();
        let script = folder.join("serve.py");
        fs::write(&script, "").unwrap();

        let expected = folder.to_string_lossy().into_owned();
        assert_eq!(infer_project_root(&script.to_string_lossy()), expected);
        assert_eq!(infer_project_root(&expected), expected);
        assert_eq!(infer_project_root(&format!("{expected}/")), expected);
    }

    #[test]
    fn a_path_that_does_not_say_where_it_is_gives_nothing() {
        assert_eq!(infer_project_root(""), "");
        assert_eq!(infer_project_root("  "), "");
        assert_eq!(infer_project_root("server.js"), "");
        assert_eq!(infer_project_root("./bin/serve"), "");
    }
}
