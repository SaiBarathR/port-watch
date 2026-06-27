use serde::{Deserialize, Serialize};

use crate::platform;
use crate::scanner::PortProcess;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemKind {
    Apple,
    Microsoft,
    Distro,
    System,
    User,
}

impl SystemKind {
    pub fn is_vendor(self) -> bool {
        matches!(
            self,
            SystemKind::Apple | SystemKind::Microsoft | SystemKind::Distro
        )
    }
}

pub fn classify(process: &mut PortProcess) {
    platform::classify(process);
}
