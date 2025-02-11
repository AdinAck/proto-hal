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
