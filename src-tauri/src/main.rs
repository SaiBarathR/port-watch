// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// A windows_subsystem = "windows" binary has no console, so println!/eprintln!
// from the CLI subcommands would vanish. Attach to the invoking shell's
// console before the first print.
//
// Attaching points all three standard handles at that console. A stream the
// caller redirected (`port-watch check 3000 > owners.json`, or captured by a
// script) arrives as a usable handle of its own, so those are put back
// afterwards; otherwise the JSON would go to the screen instead of the pipe.
#[cfg(windows)]
fn attach_parent_console() {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
        fn GetStdHandle(std_handle: u32) -> *mut c_void;
        fn SetStdHandle(std_handle: u32, handle: *mut c_void) -> i32;
        fn GetFileType(handle: *mut c_void) -> u32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_HANDLES: [u32; 3] = [-10i32 as u32, -11i32 as u32, -12i32 as u32];
    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;
    const FILE_TYPE_UNKNOWN: u32 = 0;

    // SAFETY: plain Win32 calls on this process's own standard handles; none
    // of them retains a pointer.
    unsafe {
        let redirected = STD_HANDLES.map(|id| {
            let handle = GetStdHandle(id);
            let usable = !handle.is_null()
                && handle != INVALID_HANDLE_VALUE
                && GetFileType(handle) != FILE_TYPE_UNKNOWN;
            usable.then_some((id, handle))
        });

        AttachConsole(ATTACH_PARENT_PROCESS);

        for (id, handle) in redirected.into_iter().flatten() {
            SetStdHandle(id, handle);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_cli = args.len() >= 2 && (args[1] == "install-cli" || args[1] == "check");

    #[cfg(windows)]
    if is_cli {
        attach_parent_console();
    }

    if is_cli {
        if args[1] == "install-cli" {
            port_watch_lib::cli_install::run_install_cli();
        } else {
            port_watch_lib::cli::run_check(&args[2..]);
        }
        return;
    }

    port_watch_lib::run();
}
