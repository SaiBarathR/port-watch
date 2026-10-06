use crate::classifier::SystemKind;
use crate::home::user_home;
use crate::scanner::PortProcess;

pub fn classify(process: &mut PortProcess) {
    let kind = detect_system_kind(process);
    process.system_kind = kind;
    process.is_system_service = kind != SystemKind::User;
}

fn detect_system_kind(process: &PortProcess) -> SystemKind {
    let is_current_user = process.user == current_username();
    let runs_user_project = is_current_user
        && (is_under_user_home(&process.working_directory)
            || process
                .script_path
                .as_ref()
                .is_some_and(|path| is_under_user_home(path)));

    if runs_user_project {
        return SystemKind::User;
    }

    if is_apple_binary(&process.executable_path, &process.command_line) {
        return SystemKind::Apple;
    }

    if is_system_user(&process.user) {
        return SystemKind::System;
    }

    // Anything else the current user runs is theirs, wherever the binary
    // lives: Homebrew services under /opt/homebrew, tools under /usr/local,
    // an app's helper copied into a temporary folder. Same rule as Linux.
    if is_current_user {
        return SystemKind::User;
    }

    SystemKind::System
}

fn is_apple_binary(executable_path: &str, command_line: &str) -> bool {
    let apple_prefixes = ["/System", "/usr/sbin", "/sbin", "/Library/Apple"];

    if apple_prefixes
        .iter()
        .any(|prefix| executable_path.starts_with(prefix))
    {
        return true;
    }

    if executable_path.starts_with("/usr/") && !executable_path.starts_with("/usr/local/") {
        return true;
    }

    // Only argv0 counts — an arbitrary argument mentioning "com.apple." (e.g.
    // a log path) must not mark a user process as Apple's.
    let argv0 = command_line.split_whitespace().next().unwrap_or("");
    if argv0.contains("com.apple.") {
        return true;
    }

    false
}

fn is_system_user(user: &str) -> bool {
    if user == "root" {
        return true;
    }

    user.starts_with('_') && user != current_username()
}

// Component-wise so /Users/foobar does not count as under /Users/foo.
fn is_under_user_home(path: &str) -> bool {
    let home = user_home();
    !path.is_empty()
        && !home.is_empty()
        && std::path::Path::new(path).starts_with(std::path::Path::new(home))
}

fn current_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{PortBinding, PortProcess};

    fn sample_process(executable: &str, user: &str, cwd: &str) -> PortProcess {
        PortProcess {
            id: String::new(),
            pid: 1,
            name: "test".into(),
            user: user.into(),
            ports: vec![PortBinding {
                address: "*".into(),
                port: 8080,
                protocol: "TCP".into(),
            }],
            executable_path: executable.into(),
            script_path: None,
            command_line: String::new(),
            working_directory: cwd.into(),
            project_root: String::new(),
            system_kind: SystemKind::User,
            is_system_service: false,
            started_at: 0,
            delete_blocked: None,
        }
    }

    #[test]
    fn classifies_apple_system() {
        let mut p = sample_process("/System/Library/CoreServices/ControlCenter", "ginpachi", "");
        classify(&mut p);
        assert_eq!(p.system_kind, SystemKind::Apple);
        assert!(p.is_system_service);
    }

    #[test]
    fn classifies_current_user_tools_outside_home_as_user() {
        // A brew-services database: binary and data directory both outside
        // home and /Applications.
        for (executable, cwd) in [
            (
                "/opt/homebrew/opt/postgresql@16/bin/postgres",
                "/opt/homebrew/var/postgresql@16",
            ),
            (
                "/usr/local/opt/redis/bin/redis-server",
                "/usr/local/var/db/redis",
            ),
            (
                "/private/var/folders/ab/T/com.google.Chrome.code_sign_clone/Google Chrome",
                "/",
            ),
            ("", "/"),
        ] {
            let mut p = sample_process(executable, &current_username(), cwd);
            classify(&mut p);
            assert_eq!(p.system_kind, SystemKind::User, "{executable}");
            assert!(!p.is_system_service, "{executable}");
        }
    }

    #[test]
    fn classifies_apple_binaries_run_by_the_current_user_as_apple() {
        let mut p = sample_process("/usr/libexec/rapportd", &current_username(), "/");
        classify(&mut p);
        assert_eq!(p.system_kind, SystemKind::Apple);
        assert!(p.is_system_service);
    }

    #[test]
    fn classifies_other_accounts_as_system() {
        for user in ["root", "_postgres", "someone-else"] {
            let mut p = sample_process("/opt/homebrew/bin/postgres", user, "/");
            classify(&mut p);
            assert_eq!(p.system_kind, SystemKind::System, "{user}");
            assert!(p.is_system_service, "{user}");
        }
    }

    #[test]
    fn classifies_user_process() {
        let home = user_home();
        let mut p = sample_process(
            "/usr/local/bin/python3",
            &current_username(),
            &format!("{home}/projects/app"),
        );
        classify(&mut p);
        assert_eq!(p.system_kind, SystemKind::User);
        assert!(!p.is_system_service);
    }
}
