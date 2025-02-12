use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{entitlement::Entitlement, register::Register};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peripheral {
    pub ident: String,
    pub base_addr: u32,
    pub entitlements: HashSet<Entitlement>,
    pub registers: HashMap<String, Register>,
}

impl Peripheral {
    pub fn empty(ident: String, base_addr: u32) -> Self {
        Self {
            ident,
            base_addr,
            entitlements: HashSet::new(),
            registers: HashMap::new(),
        }
    }
}

impl PartialOrd for Peripheral {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Peripheral {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.base_addr.cmp(&other.base_addr)
    }
}
