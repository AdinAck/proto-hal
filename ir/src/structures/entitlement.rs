use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Entitlement {}

impl Entitlement {
    pub fn to(_path: impl Into<String>) -> Self {
        Self {}
    }
}
