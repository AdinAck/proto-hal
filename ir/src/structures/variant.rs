use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    pub ident: String,
    pub bits: u32,
}

impl Variant {
    pub fn empty(ident: String, bits: u32) -> Self {
        Self { ident, bits }
    }
}
