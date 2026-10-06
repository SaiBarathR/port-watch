use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemKind {
    Apple,
    Microsoft,
    Distro,
    System,
    User,
}
