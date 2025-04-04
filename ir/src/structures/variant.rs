use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{entitlement::Entitlement, Ident};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    pub ident: String,
    pub bits: u32,
    pub entitlements: HashSet<Entitlement>,
}

impl Variant {
    pub fn new(ident: impl Into<String>, bits: u32) -> Self {
        Self {
            ident: ident.into(),
            bits,
            entitlements: HashSet::new(),
        }
    }

    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        self.entitlements.extend(entitlements);
        self
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
