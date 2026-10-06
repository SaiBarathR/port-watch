//! Readers for what the scan tools print. They are pure, and compiled on
//! every platform so that each one's tests run wherever the suite runs.

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub mod lsof;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub mod powershell;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub mod procargs;
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub mod procfs;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub mod ps;
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub mod ss;

#[cfg(test)]
macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/",
            $name
        ))
    };
}
#[cfg(test)]
pub(crate) use fixture;
