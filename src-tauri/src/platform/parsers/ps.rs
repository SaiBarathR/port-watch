//! `ps -ww -p <pids> -o pid=,user=,command=` output.

use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PsInfo {
    pub user: String,
    pub command_line: String,
}

pub fn parse(stdout: &str) -> HashMap<u32, PsInfo> {
    stdout.lines().filter_map(parse_line).collect()
}

// ps pads columns with runs of spaces, so take the first two tokens and keep
// the rest verbatim as the command line.
fn parse_line(line: &str) -> Option<(u32, PsInfo)> {
    let (pid, rest) = split_token(line);
    let pid = pid.parse::<u32>().ok()?;
    let (user, rest) = split_token(rest);

    Some((
        pid,
        PsInfo {
            user: user.to_string(),
            command_line: rest.trim().to_string(),
        },
    ))
}

fn split_token(input: &str) -> (&str, &str) {
    let trimmed = input.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(end) => (&trimmed[..end], &trimmed[end..]),
        None => (trimmed, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;

    #[test]
    fn reads_user_and_command_line_per_pid() {
        let info = parse(fixture!("ps.txt"));
        assert_eq!(info.len(), 4);
        assert_eq!(info[&1].user, "root");
        assert_eq!(info[&1].command_line, "/sbin/launchd");
        assert_eq!(info[&4310].user, "dev");
        assert_eq!(
            info[&4310].command_line,
            "/opt/homebrew/opt/postgresql@16/bin/postgres -D /opt/homebrew/var/postgresql@16"
        );
        // Spaces inside the command line are kept as they are.
        assert_eq!(
            info[&5120].command_line,
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome Helper --type=utility  --lang=en-US"
        );
        // A process ps could name but not describe.
        assert_eq!(info[&99999].user, "_windowserver");
        assert_eq!(info[&99999].command_line, "");
    }

    #[test]
    fn lines_that_are_not_a_process_are_skipped() {
        assert!(parse("ps: illegal process id: x\n\n").is_empty());
    }
}
