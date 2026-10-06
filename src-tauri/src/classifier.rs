use serde::Serialize;

use crate::platform;
use crate::scanner::PortProcess;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemKind {
    Apple,
    Microsoft,
    Distro,
    System,
    User,
}

pub fn classify(process: &mut PortProcess) {
    platform::classify(process);
}
