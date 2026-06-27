use crate::classifier::SystemKind;
use crate::home::user_home;
use crate::platform::windows::paths;
use crate::scanner::PortProcess;
use std::path::PathBuf;

pub fn classify(process: &mut PortProcess) {
    let kind = detect_system_kind(process);
    process.system_kind = kind;
    process.is_system_service = kind != SystemKind::User;
}

fn detect_system_kind(process: &PortProcess) -> SystemKind {
    let is_current_user = users_match(&process.user, &current_username());
    let runs_user_project = is_current_user
        && (is_under_user_home(&process.working_directory)
            || process
                .script_path
                .as_ref()
                .is_some_and(|path| is_under_user_home(path))
            || is_under_user_home(&process.executable_path));

    if runs_user_project {
        return SystemKind::User;
    }

    if paths::is_microsoft_path(&process.executable_path, &process.command_line) {
        return SystemKind::Microsoft;
    }

    if is_system_user(&process.user) {
        return SystemKind::System;
    }

    if is_current_user {
        return SystemKind::User;
    }

    SystemKind::System
}

fn is_system_user(user: &str) -> bool {
    let user = user.to_ascii_uppercase();
    user.contains("SYSTEM")
        || user.contains("LOCAL SERVICE")
        || user.contains("NETWORK SERVICE")
        || user.starts_with("NT AUTHORITY\\")
        || user.starts_with("NT SERVICE\\")
}

fn users_match(left: &str, right: &str) -> bool {
    let left = left.to_ascii_lowercase();
    let right = right.to_ascii_lowercase();
    left == right
        || left.ends_with(&format!("\\{right}"))
        || right.ends_with(&format!("\\{left}"))
        || left.split('\\').next_back() == right.split('\\').next_back()
}

fn is_under_user_home(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }

    let home = PathBuf::from(user_home());
    let target = PathBuf::from(path);
    crate::platform::windows::paths::path_starts_with(&target, &home)
}

fn current_username() -> String {
    std::env::var("USERNAME").unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{PortBinding, PortProcess};

    #[test]
    fn classifies_microsoft_binary() {
        let mut p = PortProcess {
            pid: 1,
            name: "svchost".into(),
            user: "NT AUTHORITY\\SYSTEM".into(),
            ports: vec![PortBinding {
                address: "*".into(),
                port: 80,
                protocol: "TCP".into(),
            }],
            executable_path: "C:\\Windows\\System32\\svchost.exe".into(),
            script_path: None,
            command_line: String::new(),
            working_directory: String::new(),
            project_root: String::new(),
            system_kind: SystemKind::User,
            is_system_service: false,
            uptime_seconds: 0,
        };
        classify(&mut p);
        assert_eq!(p.system_kind, SystemKind::Microsoft);
    }
}
