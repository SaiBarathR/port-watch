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

    if is_distro_binary(&process.executable_path) {
        return SystemKind::Distro;
    }

    if is_system_user(&process.user) {
        return SystemKind::System;
    }

    if is_current_user {
        if is_under_user_home(&process.executable_path) {
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

fn is_distro_binary(executable_path: &str) -> bool {
    if executable_path.is_empty() {
        return false;
    }

    let distro_prefixes = ["/usr/bin/", "/usr/lib/", "/usr/sbin/", "/bin/", "/sbin/", "/lib/"];
    if distro_prefixes
        .iter()
        .any(|prefix| executable_path.starts_with(prefix))
    {
        return !executable_path.starts_with("/usr/local/");
    }

    executable_path.starts_with("/usr/") && !executable_path.starts_with("/usr/local/")
}

fn is_system_user(user: &str) -> bool {
    if user == "root" || user == "nobody" {
        return true;
    }

    user.starts_with('_') && user != current_username()
}

fn is_binary_outside_user_home(executable_path: &str) -> bool {
    if executable_path.is_empty() {
        return false;
    }

    !executable_path.starts_with(user_home())
}

fn is_under_user_home(path: &str) -> bool {
    !path.is_empty() && path.starts_with(user_home())
}

fn current_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{PortBinding, PortProcess};

    #[test]
    fn classifies_distro_binary() {
        let mut p = PortProcess {
            pid: 1,
            name: "nginx".into(),
            user: "root".into(),
            ports: vec![PortBinding {
                address: "*".into(),
                port: 80,
                protocol: "TCP".into(),
            }],
            executable_path: "/usr/sbin/nginx".into(),
            script_path: None,
            command_line: String::new(),
            working_directory: "/".into(),
            project_root: String::new(),
            system_kind: SystemKind::User,
            is_system_service: false,
            uptime_seconds: 0,
        };
        classify(&mut p);
        assert_eq!(p.system_kind, SystemKind::Distro);
    }
}
