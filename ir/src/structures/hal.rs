use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::peripheral::Peripheral;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hal {
    pub peripherals: HashMap<String, Peripheral>,
}

impl Hal {
    pub fn empty() -> Self {
        Self {
            peripherals: HashMap::new(),
        }
    }
}

impl PartialOrd for Hal {
    fn partial_cmp(&self, #[allow(unused)] other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

impl Ord for Hal {
    fn cmp(&self, #[allow(unused)] other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}
