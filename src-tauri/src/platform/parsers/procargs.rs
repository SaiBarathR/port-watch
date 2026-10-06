//! The reply to `sysctl(KERN_PROCARGS2)` on macOS: the argument count, the
//! path of the executable, NUL padding, the arguments, then the environment.

/// The arguments joined by spaces, on one line, as `ps -o command=` prints
/// them: a control character becomes a backslash and its octal code.
pub fn parse_command_line(reply: &[u8]) -> Option<String> {
    let (count, rest) = reply.split_first_chunk::<4>()?;
    let count = usize::try_from(i32::from_ne_bytes(*count)).ok()?;
    if count == 0 {
        return Some(String::new());
    }

    let path_end = rest.iter().position(|&byte| byte == 0)?;
    let after_path = &rest[path_end..];
    let arguments_start = after_path.iter().position(|&byte| byte != 0)?;

    let arguments: Vec<String> = after_path[arguments_start..]
        .split(|&byte| byte == 0)
        .take(count)
        .map(|argument| String::from_utf8_lossy(argument).into_owned())
        .collect();

    // Written out before the trim below, so an argument that ends in a
    // newline keeps it.
    let mut line = String::new();
    for character in arguments.join(" ").chars() {
        if character.is_ascii_control() {
            line.push_str(&format!("\\{:03o}", character as u32));
        } else {
            line.push(character);
        }
    }
    Some(line.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(count: i32, path: &str, padding: usize, rest: &[&str]) -> Vec<u8> {
        let mut bytes = count.to_ne_bytes().to_vec();
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend(std::iter::repeat_n(0, padding));
        for piece in rest {
            bytes.extend_from_slice(piece.as_bytes());
            bytes.push(0);
        }
        bytes
    }

    #[test]
    fn joins_the_arguments_and_leaves_the_environment_out() {
        let bytes = reply(
            3,
            "/usr/local/bin/node",
            5,
            &[
                "node",
                "server.js",
                "--port=3000",
                "HOME=/Users/dev",
                "PATH=/bin",
            ],
        );
        assert_eq!(
            parse_command_line(&bytes).as_deref(),
            Some("node server.js --port=3000")
        );
    }

    #[test]
    fn an_empty_argument_keeps_its_place_unless_it_is_last() {
        let bytes = reply(3, "/bin/echo", 1, &["echo", "", "x", "A=1"]);
        assert_eq!(parse_command_line(&bytes).as_deref(), Some("echo  x"));
        let bytes = reply(3, "/bin/echo", 1, &["echo", "x", "", "A=1"]);
        assert_eq!(parse_command_line(&bytes).as_deref(), Some("echo x"));
    }

    #[test]
    fn control_characters_are_written_out_so_the_line_stays_one_line() {
        let bytes = reply(3, "/bin/sh", 1, &["sh", "-c", "echo a\necho\tb\u{7f}"]);
        assert_eq!(
            parse_command_line(&bytes).as_deref(),
            Some("sh -c echo a\\012echo\\011b\\177")
        );
    }

    #[test]
    fn a_last_argument_of_control_characters_is_not_trimmed_away() {
        let bytes = reply(3, "/usr/bin/perl", 1, &["perl", "-e", "\n"]);
        assert_eq!(parse_command_line(&bytes).as_deref(), Some("perl -e \\012"));
        let bytes = reply(3, "/usr/bin/perl", 1, &["perl", "x\t", ""]);
        assert_eq!(parse_command_line(&bytes).as_deref(), Some("perl x\\011"));
    }

    #[test]
    fn text_that_is_not_ascii_is_kept_as_it_is() {
        let bytes = reply(
            2,
            "/usr/bin/node",
            1,
            &["node", "/Users/dev/café/server.js"],
        );
        assert_eq!(
            parse_command_line(&bytes).as_deref(),
            Some("node /Users/dev/café/server.js")
        );
    }

    #[test]
    fn a_process_with_no_arguments_has_an_empty_command_line() {
        let bytes = reply(0, "/sbin/launchd", 3, &["A=1"]);
        assert_eq!(parse_command_line(&bytes).as_deref(), Some(""));
    }

    #[test]
    fn a_reply_cut_short_is_read_as_far_as_it_goes() {
        let mut bytes = reply(2, "/bin/sleep", 2, &["sleep"]);
        bytes.extend_from_slice(b"60");
        assert_eq!(parse_command_line(&bytes).as_deref(), Some("sleep 60"));
    }

    #[test]
    fn a_reply_that_is_not_one_is_refused() {
        assert_eq!(parse_command_line(&[]), None);
        assert_eq!(parse_command_line(&[1, 0]), None);
        assert_eq!(parse_command_line(&(-1i32).to_ne_bytes()), None);
        // A count, and a path that never ends.
        assert_eq!(parse_command_line(&reply(1, "/bin/sleep", 0, &[])), None);
        // A count, a path, and nothing after it.
        assert_eq!(parse_command_line(&reply(1, "/bin/sleep", 4, &[])), None);
    }
}
