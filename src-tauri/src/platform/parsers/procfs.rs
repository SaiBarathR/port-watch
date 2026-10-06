//! The contents of files under /proc.

/// `btime <seconds>` in /proc/stat: when the machine booted.
pub fn parse_boot_time(proc_stat: &str) -> Option<u64> {
    proc_stat
        .lines()
        .find_map(|line| line.strip_prefix("btime "))
        .and_then(|value| value.trim().parse().ok())
}

/// The start time in /proc/<pid>/stat, in clock ticks since boot.
// The comm field (2nd) can contain spaces and parens — "(tmux: server)" — so
// split after its closing paren; starttime is field 22 overall, the 20th
// after the state.
pub fn parse_start_ticks(proc_pid_stat: &str) -> Option<u64> {
    proc_pid_stat
        .rsplit_once(')')
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_whitespace().nth(19))
        .and_then(|value| value.parse().ok())
}

/// The real UID in /proc/<pid>/status.
pub fn parse_uid(proc_pid_status: &str) -> Option<u32> {
    proc_pid_status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|uids| uids.split_whitespace().next())
        .and_then(|uid| uid.parse().ok())
}

/// /proc/<pid>/cmdline separates arguments with NUL bytes.
pub fn parse_cmdline(raw: &str) -> String {
    raw.replace('\0', " ").trim().to_string()
}

/// The name /etc/passwd gives a UID.
pub fn parse_passwd_name(passwd: &str, uid: u32) -> Option<String> {
    passwd.lines().find_map(|line| {
        let mut parts = line.split(':');
        let name = parts.next()?;
        let _password = parts.next()?;
        let line_uid = parts.next()?.parse::<u32>().ok()?;
        (line_uid == uid).then(|| name.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_boot_time_reads_btime() {
        let stat = "cpu  1 2 3\nintr 5\nbtime 1790000000\nprocesses 42\n";
        assert_eq!(parse_boot_time(stat), Some(1_790_000_000));
        assert_eq!(parse_boot_time("cpu  1 2 3\n"), None);
    }

    #[test]
    fn parse_start_ticks_survives_spaces_and_parens_in_the_name() {
        let stat = "4242 (tmux: server (1)) S 1 4242 4242 0 -1 4194560 100 0 0 0 5 3 0 0 20 0 1 0 987654 1000000 200 18446744073709551615";
        assert_eq!(parse_start_ticks(stat), Some(987_654));
        assert_eq!(parse_start_ticks(""), None);
    }

    // As the kernel prints it for a real process.
    #[test]
    fn parse_start_ticks_reads_a_real_stat_line() {
        let stat = "32 (cat) R 1 1 1 0 -1 4194304 74 0 0 0 0 0 0 0 20 0 1 0 1774404 2437120 207 18446744073709551615 187650786131968 187650786165264 281474583524000 0 0 0 0 0 0 0 0 0 17 8 0 0 0 0 0";
        assert_eq!(parse_start_ticks(stat), Some(1_774_404));
    }

    #[test]
    fn parse_uid_reads_the_real_uid() {
        let status = "Name:\tpython3\nUmask:\t0022\nUid:\t1000\t1001\t1002\t1003\nGid:\t100\t100\t100\t100\n";
        assert_eq!(parse_uid(status), Some(1000));
        assert_eq!(parse_uid("Name:\tpython3\n"), None);
        assert_eq!(parse_uid("Uid:\tnot-a-number\n"), None);
    }

    #[test]
    fn parse_cmdline_joins_arguments_with_spaces() {
        assert_eq!(
            parse_cmdline("python3\0-m\0http.server\08000\0"),
            "python3 -m http.server 8000"
        );
        assert_eq!(parse_cmdline(""), "");
    }

    #[test]
    fn parse_passwd_name_finds_the_user_for_a_uid() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\n# a comment\nbroken\ntester:x:1000:1000::/home/tester:/bin/sh\n";
        assert_eq!(parse_passwd_name(passwd, 1000), Some("tester".into()));
        assert_eq!(parse_passwd_name(passwd, 0), Some("root".into()));
        assert_eq!(parse_passwd_name(passwd, 42), None);
    }
}
