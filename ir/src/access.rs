use std::collections::HashSet;

use crate::structures::{entitlement::Entitlement, field::Numericity};

#[derive(Debug, Clone)]
pub struct Read {
    pub numericity: Numericity,
    pub entitlements: HashSet<Entitlement>,
    pub effects: (),
}

#[derive(Debug, Clone)]
pub struct Write {
    pub numericity: Numericity,
    pub entitlements: HashSet<Entitlement>,
    pub effects: (),
}

#[derive(Debug, Clone)]
pub enum Access {
    Read(Read),
    Write(Write),
    ReadWrite { read: Read, write: Write },
}

impl Access {
    pub fn read_write(numericity: Numericity) -> Access {
        Access::ReadWrite {
            read: Read {
                numericity: numericity.clone(),
                entitlements: HashSet::new(),
                effects: (),
            },
            write: Write {
                numericity,
                entitlements: HashSet::new(),
                effects: (),
            },
        }
    }

    pub fn is_read(&self) -> bool {
        match self {
            Self::Read(_) | Self::ReadWrite { read: _, write: _ } => true,
            Self::Write(_) => false,
        }
    }

    pub fn is_write(&self) -> bool {
        match self {
            Self::Write(_) | Self::ReadWrite { read: _, write: _ } => true,
            Self::Read(_) => false,
        }
    }
}
