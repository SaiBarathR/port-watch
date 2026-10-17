//! Running from an AppImage. The image's launcher points library, module and
//! data paths into the image's mount (`$APPDIR`), and forces a few GTK
//! settings, for this process. The programs the app starts for the user (the
//! browser, file manager, terminal, editor) are not part of the image and
//! must not inherit them: with the image's PYTHONHOME, a terminal opened from
//! a row has no working python3.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::Command;

// Set by the launcher with no record of what they were before it ran.
const LAUNCHER_ONLY: [&str; 6] = [
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "GDK_BACKEND",
    "GTK_THEME",
];

/// The image's mount, when this process is the one an AppImage started. The
/// variables are inherited by everything started from an AppImage (the
/// terminal this was run from may be one), so they count only when this
/// executable is inside the mount they name.
fn mount() -> Option<PathBuf> {
    let appdir = PathBuf::from(std::env::var_os("APPDIR")?);
    let exe = std::env::current_exe().ok()?;
    (appdir.is_absolute() && exe.starts_with(&appdir)).then_some(appdir)
}

pub fn running() -> bool {
    mount().is_some()
}

/// The .AppImage file this process was started from. Unlike the executable
/// in the mount, it is still there after the app quits.
pub fn image() -> Option<PathBuf> {
    mount()?;
    let image = PathBuf::from(std::env::var_os("APPIMAGE")?);
    (image.is_absolute() && image.is_file()).then_some(image)
}

/// A command for a program outside the image, with the environment the user
/// would have given it. Outside an AppImage, a plain `Command::new`.
pub fn host_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    if let Some(mount) = mount() {
        for (key, value) in host_changes(&mount, std::env::vars_os()) {
            match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        // The launcher also moves into the mount, which goes when the app
        // quits. The program starts where the image was started from instead
        // (OWD), as it would have without the image. A caller that names a
        // folder of its own still has the last word.
        let folder = std::env::var_os("OWD")
            .map(PathBuf::from)
            .filter(|folder| folder.is_dir() && !folder.starts_with(&mount))
            .or_else(dirs::home_dir);
        if let Some(folder) = folder {
            command.current_dir(folder);
        }
    }
    command
}

// What to change in this environment for a program outside the image: each
// launcher-only variable goes, and every path list loses its entries in the
// mount, or goes when nothing else is left in it.
fn host_changes(
    mount: &Path,
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, Option<OsString>)> {
    vars.into_iter()
        .filter_map(|(key, value)| {
            if LAUNCHER_ONLY.iter().any(|name| key == *name) {
                return Some((key, None));
            }
            let in_mount = |entry: &&[u8]| Path::new(OsStr::from_bytes(entry)).starts_with(mount);
            let entries: Vec<&[u8]> = value.as_bytes().split(|byte| *byte == b':').collect();
            if !entries.iter().any(in_mount) {
                return None;
            }
            let kept: Vec<&[u8]> = entries
                .into_iter()
                .filter(|entry| !entry.is_empty() && !in_mount(entry))
                .collect();
            let value = (!kept.is_empty()).then(|| OsString::from_vec(kept.join(&b':')));
            Some((key, value))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changes(vars: &[(&str, &str)]) -> Vec<(String, Option<String>)> {
        host_changes(
            Path::new("/tmp/.mount_Port.Wab12"),
            vars.iter().map(|(key, value)| (key.into(), value.into())),
        )
        .into_iter()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|value| value.to_string_lossy().into_owned()),
            )
        })
        .collect()
    }

    #[test]
    fn path_lists_keep_only_their_entries_outside_the_mount() {
        assert_eq!(
            changes(&[
                (
                    "XDG_DATA_DIRS",
                    "/tmp/.mount_Port.Wab12/usr/share:/usr/share:/usr/local/share"
                ),
                ("PATH", "/tmp/.mount_Port.Wab12/usr/bin/:/usr/bin:/bin"),
            ]),
            vec![
                (
                    "XDG_DATA_DIRS".into(),
                    Some("/usr/share:/usr/local/share".into())
                ),
                ("PATH".into(), Some("/usr/bin:/bin".into())),
            ]
        );
    }

    #[test]
    fn variables_that_only_point_into_the_mount_are_removed() {
        assert_eq!(
            changes(&[
                ("PYTHONHOME", "/tmp/.mount_Port.Wab12/usr/"),
                ("PYTHONPATH", "/tmp/.mount_Port.Wab12/usr/share/pyshared/:"),
                (
                    "GDK_PIXBUF_MODULE_FILE",
                    "/tmp/.mount_Port.Wab12//usr/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"
                ),
            ]),
            vec![
                ("PYTHONHOME".into(), None),
                ("PYTHONPATH".into(), None),
                ("GDK_PIXBUF_MODULE_FILE".into(), None),
            ]
        );
    }

    #[test]
    fn the_launchers_own_settings_are_removed() {
        assert_eq!(
            changes(&[
                ("APPDIR", "/tmp/.mount_Port.Wab12"),
                ("APPIMAGE", "/home/me/Port.Watch.AppImage"),
                ("GDK_BACKEND", "x11"),
                ("GTK_THEME", "Adwaita:dark"),
            ]),
            vec![
                ("APPDIR".into(), None),
                ("APPIMAGE".into(), None),
                ("GDK_BACKEND".into(), None),
                ("GTK_THEME".into(), None),
            ]
        );
    }

    #[test]
    fn everything_else_is_left_as_it_is() {
        assert!(changes(&[
            ("HOME", "/home/me"),
            ("PATH", "/usr/bin:/bin"),
            // Another image's mount, and one whose name only starts the same.
            ("XDG_DATA_DIRS", "/tmp/.mount_Other/usr/share:/usr/share"),
            ("LD_LIBRARY_PATH", "/tmp/.mount_Port.Wab123/usr/lib"),
        ])
        .is_empty());
    }
}
