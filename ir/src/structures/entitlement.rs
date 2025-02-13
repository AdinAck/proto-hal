use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Entitlement {}

impl ToString for Entitlement {
    fn to_string(&self) -> String {
        "some::entitlement".into()
    }
}
