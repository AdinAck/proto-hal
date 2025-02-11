use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::field::Field;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Register {
    pub ident: String,
    pub offset: u32,

    pub fields: HashMap<String, Field>,
}

impl Register {
    pub fn empty(ident: String, offset: u32) -> Self {
        Self {
            ident,
            offset,
            fields: HashMap::new(),
        }
    }
}
