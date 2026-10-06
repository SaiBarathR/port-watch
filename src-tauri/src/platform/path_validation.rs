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

// Folders every account has. Projects live inside them, but the folder itself
// is never what "delete this project folder" means.
const STANDARD_HOME_FOLDERS: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Movies",
    "Music",
    "Pictures",
    "Public",
    "Videos",
];

// App data, settings and installed apps. Nothing at any depth inside these is
// a project folder; hidden folders in home (.ssh, .config, .local, ...) are
// treated the same way.
const APP_DATA_HOME_FOLDERS: &[&str] = &["Library", "Applications", "AppData"];

/// What the app is willing to delete: a real folder inside the user's home,
/// outside the places that never hold a project.
pub struct DeleteRules {
    home: PathBuf,
    is_protected: fn(&Path) -> bool,
}

impl DeleteRules {
    /// `home` must already be canonical.
    pub fn new(home: PathBuf, is_protected: fn(&Path) -> bool) -> Self {
        Self { home, is_protected }
    }

    pub fn for_current_user() -> Result<Self, String> {
        Ok(Self::new(
            canonical_home()?,
            crate::platform::guards::is_protected_canonical,
        ))
    }

    /// The canonical form of `path`, if it is a folder that may be deleted.
    pub fn resolve(&self, path: &Path) -> Result<PathBuf, String> {
        let canonical = resolve_existing_folder(path)?;
        self.check(&canonical)?;
        Ok(canonical)
    }

    /// Like `resolve`, and the user must also have typed the folder's name.
    pub fn resolve_permanent(&self, path: &Path, confirmation: &str) -> Result<PathBuf, String> {
        let expected = path
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

        self.resolve(path)
    }

    fn check(&self, canonical: &Path) -> Result<(), String> {
        if (self.is_protected)(canonical) {
            return Err("This folder is a system location.".into());
        }

        let Ok(relative) = canonical.strip_prefix(&self.home) else {
            return Err("This folder is outside your home folder.".into());
        };

        let mut components = relative.components();
        let Some(first) = components.next() else {
            return Err("Your home folder itself cannot be deleted.".into());
        };
        let first = first.as_os_str().to_string_lossy();
        let is_nested = components.next().is_some();

        if first.starts_with('.') || matches_any(&first, APP_DATA_HOME_FOLDERS) {
            return Err(format!(
                "{first} holds app data and settings, so nothing in it can be deleted here."
            ));
        }

        if !is_nested && matches_any(&first, STANDARD_HOME_FOLDERS) {
            return Err(format!(
                "{first} is one of your standard folders, not a project folder."
            ));
        }

        Ok(())
    }
}

fn matches_any(name: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn resolve_existing_folder(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("No project folder is known for this process.".into());
    }

    // A relative path would resolve against wherever this app was started.
    if !path.is_absolute() {
        return Err("The folder is not an absolute path.".into());
    }

    // Rebuilding from components drops a trailing separator or `/.`, either of
    // which would make the lookup below follow a final symlink instead of
    // reporting it.
    let entry: PathBuf = path.components().collect();
    let metadata = std::fs::symlink_metadata(&entry).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            "This folder no longer exists.".to_string()
        } else {
            format!("Failed to inspect the folder: {err}")
        }
    })?;

    if metadata.file_type().is_symlink() {
        return Err("This is a symlink, not a folder.".into());
    }

    if !metadata.is_dir() {
        return Err("This is a file, not a folder.".into());
    }

    std::fs::canonicalize(&entry).map_err(|err| format!("Failed to resolve the folder: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn not_protected(_: &Path) -> bool {
        false
    }

    // A stand-in home directory, so no test touches the real one.
    struct Home {
        _dir: tempfile::TempDir,
        path: PathBuf,
        rules: DeleteRules,
    }

    impl Home {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let path = fs::canonicalize(dir.path()).unwrap().join("home");
            fs::create_dir(&path).unwrap();
            let rules = DeleteRules::new(path.clone(), not_protected);
            Self {
                _dir: dir,
                path,
                rules,
            }
        }

        fn mkdir(&self, relative: &str) -> PathBuf {
            let path = self.path.join(relative);
            fs::create_dir_all(&path).unwrap();
            path
        }

        fn outside(&self, relative: &str) -> PathBuf {
            let path = self.path.parent().unwrap().join(relative);
            fs::create_dir_all(&path).unwrap();
            path
        }
    }

    #[test]
    fn allows_a_project_folder() {
        let home = Home::new();
        let project = home.mkdir("Dev/my-project");
        assert_eq!(home.rules.resolve(&project), Ok(project));
    }

    #[test]
    fn allows_projects_inside_standard_folders() {
        let home = Home::new();
        for parent in ["Documents", "Desktop", "Downloads"] {
            let project = home.mkdir(&format!("{parent}/my-project"));
            assert_eq!(home.rules.resolve(&project), Ok(project));
        }
    }

    #[test]
    fn rejects_home_itself() {
        let home = Home::new();
        let err = home.rules.resolve(&home.path).unwrap_err();
        assert!(err.contains("home folder itself"), "{err}");
    }

    #[test]
    fn rejects_standard_folders_themselves() {
        let home = Home::new();
        for name in ["Documents", "Desktop", "Downloads", "Pictures"] {
            let err = home.rules.resolve(&home.mkdir(name)).unwrap_err();
            assert!(err.contains("standard folders"), "{name}: {err}");
        }
    }

    #[test]
    fn rejects_app_data_at_any_depth() {
        let home = Home::new();
        for relative in [
            "Library",
            "Library/Application Support",
            "Library/Application Support/Some App/data",
            "Applications/Some.app/Contents",
            "AppData/Local/Programs/cursor",
            ".config",
            ".config/nvim",
            ".local/share/app",
            ".ssh",
            ".cargo/bin",
        ] {
            let err = home.rules.resolve(&home.mkdir(relative)).unwrap_err();
            assert!(err.contains("app data and settings"), "{relative}: {err}");
        }
    }

    #[test]
    fn rejects_folders_outside_home() {
        let home = Home::new();
        let err = home
            .rules
            .resolve(&home.outside("elsewhere/project"))
            .unwrap_err();
        assert!(err.contains("outside your home folder"), "{err}");

        // A sibling whose name merely starts with the home folder's name.
        let err = home.rules.resolve(&home.outside("homework")).unwrap_err();
        assert!(err.contains("outside your home folder"), "{err}");
    }

    #[test]
    fn rejects_system_locations() {
        fn everything_protected(_: &Path) -> bool {
            true
        }
        let home = Home::new();
        let project = home.mkdir("Dev/my-project");
        let rules = DeleteRules::new(home.path.clone(), everything_protected);
        let err = rules.resolve(&project).unwrap_err();
        assert!(err.contains("system location"), "{err}");
    }

    #[test]
    fn parent_traversal_is_judged_by_where_it_lands() {
        let home = Home::new();
        let project = home.mkdir("Dev/my-project");
        home.mkdir(".ssh");
        home.outside("elsewhere");

        let err = home.rules.resolve(&project.join("../..")).unwrap_err();
        assert!(err.contains("home folder itself"), "{err}");
        let err = home.rules.resolve(&project.join("../../.ssh")).unwrap_err();
        assert!(err.contains("app data and settings"), "{err}");
        let err = home
            .rules
            .resolve(&project.join("../../../elsewhere"))
            .unwrap_err();
        assert!(err.contains("outside your home folder"), "{err}");
    }

    #[test]
    fn rejects_missing_relative_and_empty_paths() {
        let home = Home::new();
        let err = home.rules.resolve(&home.path.join("gone")).unwrap_err();
        assert!(err.contains("no longer exists"), "{err}");
        let err = home.rules.resolve(Path::new("Dev/my-project")).unwrap_err();
        assert!(err.contains("not an absolute path"), "{err}");
        let err = home.rules.resolve(Path::new("")).unwrap_err();
        assert!(err.contains("No project folder"), "{err}");
    }

    #[test]
    fn rejects_files() {
        let home = Home::new();
        let file = home.mkdir("Dev").join("server");
        fs::write(&file, "").unwrap();
        let err = home.rules.resolve(&file).unwrap_err();
        assert!(err.contains("a file, not a folder"), "{err}");
    }

    #[test]
    #[cfg(unix)]
    fn rejects_symlinks_however_they_are_spelled() {
        let home = Home::new();
        let project = home.mkdir("Dev/my-project");
        let outside = home.outside("elsewhere");

        let inside_link = home.path.join("Dev/link");
        std::os::unix::fs::symlink(&project, &inside_link).unwrap();
        let escape_link = home.path.join("Dev/escape");
        std::os::unix::fs::symlink(&outside, &escape_link).unwrap();

        for link in [&inside_link, &escape_link] {
            // With a trailing separator or `/.`, lstat reports the target.
            for suffix in ["", "/", "//", "/."] {
                let spelled = PathBuf::from(format!("{}{suffix}", link.display()));
                let err = home.rules.resolve(&spelled).unwrap_err();
                assert!(err.contains("symlink"), "{}: {err}", spelled.display());
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn a_folder_reached_through_a_symlink_is_judged_by_its_real_location() {
        let home = Home::new();
        let outside = home.outside("elsewhere/project");
        std::os::unix::fs::symlink(outside.parent().unwrap(), home.path.join("mount")).unwrap();

        let err = home
            .rules
            .resolve(&home.path.join("mount/project"))
            .unwrap_err();
        assert!(err.contains("outside your home folder"), "{err}");
    }

    #[test]
    fn permanent_delete_requires_the_folder_name() {
        let home = Home::new();
        let project = home.mkdir("Dev/my-project");

        let err = home
            .rules
            .resolve_permanent(&project, "my-projec")
            .unwrap_err();
        assert!(err.contains("Confirmation must match"), "{err}");
        let err = home.rules.resolve_permanent(&project, "").unwrap_err();
        assert!(err.contains("Confirmation must match"), "{err}");
        assert_eq!(
            home.rules.resolve_permanent(&project, "my-project"),
            Ok(project)
        );
    }

    #[test]
    fn the_real_home_resolves() {
        assert!(canonical_home().is_ok());
    }
}
