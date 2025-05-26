use std::collections::HashSet;

use crate::structures::{entitlement::Entitlement, field::Numericity};

#[derive(Debug, Clone)]
pub struct AccessProperties {
    pub numericity: Numericity,
    pub entitlements: HashSet<Entitlement>,
    pub effects: (),
}

#[derive(Debug, Clone)]
pub enum Access {
    Read(AccessProperties),
    Write(AccessProperties),
    ReadWrite {
        read: AccessProperties,
        write: AccessProperties,
    },
}

impl Access {
    pub fn read_write(numericity: Numericity) -> Access {
        Access::ReadWrite {
            read: AccessProperties {
                numericity: numericity.clone(),
                entitlements: HashSet::new(),
                effects: (),
            },
            write: AccessProperties {
                numericity,
                entitlements: HashSet::new(),
                effects: (),
            },
        }
    }

    pub fn get_read(&self) -> Option<&AccessProperties> {
        if let Self::Read(read) | Self::ReadWrite { read, write: _ } = self {
            Some(read)
        } else {
            None
        }
    }

    pub fn get_write(&self) -> Option<&AccessProperties> {
        if let Self::Write(write) | Self::ReadWrite { read: _, write } = self {
            Some(write)
        } else {
            None
        }
    }

    pub fn is_read(&self) -> bool {
        self.get_read().is_some()
    }

    pub fn is_write(&self) -> bool {
        self.get_write().is_some()
    }
}
