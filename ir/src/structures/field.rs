use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::variant::Variant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Numericity {
    Numeric,
    Enumerated { variants: HashMap<String, Variant> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub ident: String,
    pub offset: u8,
    pub width: u8,
    pub numericity: Numericity,
}

impl Field {
    pub fn empty(ident: String, offset: u8, width: u8, numericity: Numericity) -> Self {
        Self {
            ident,
            offset,
            width,
            numericity,
        }
    }
}
