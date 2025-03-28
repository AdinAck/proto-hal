use serde::{Deserialize, Serialize};

use super::Ident;

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

impl PartialOrd for Variant {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Variant {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.bits.cmp(&other.bits)
    }
}

impl Ident for Variant {
    fn ident(&self) -> &str {
        &self.ident
    }
}
