pub mod identity;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
pub mod parsers;
pub mod path_validation;
pub mod shared;
#[cfg(unix)]
pub mod unix;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{classify, scan};
#[cfg(target_os = "macos")]
pub use macos::{classify, scan};
#[cfg(target_os = "windows")]
pub use windows::{classify, scan};

#[cfg(target_os = "linux")]
pub use linux::guards;
#[cfg(target_os = "macos")]
pub use macos::guards;
#[cfg(target_os = "windows")]
pub use windows::guards;

#[cfg(target_os = "linux")]
pub use linux::shell;
#[cfg(target_os = "macos")]
pub use macos::shell;
#[cfg(target_os = "windows")]
pub use windows::shell;
