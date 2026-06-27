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

    if is_current_user {
        if is_under_user_home(&process.executable_path) || is_under_applications(&process.executable_path)
        {
            return SystemKind::User;
        }
    }

    if is_binary_outside_user_home(&process.executable_path) {
        return SystemKind::System;
    }

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

    if command_line.contains("com.apple.") {
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

fn is_binary_outside_user_home(executable_path: &str) -> bool {
    if executable_path.is_empty() {
        return false;
    }

    let home = user_home();
    !executable_path.starts_with(home) && !is_under_applications(executable_path)
}

fn is_under_user_home(path: &str) -> bool {
    !path.is_empty() && path.starts_with(user_home())
}

fn is_under_applications(path: &str) -> bool {
    let home = user_home();
    path.starts_with("/Applications/") || path.starts_with(&format!("{home}/Applications/"))
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
            uptime_seconds: 0,
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
