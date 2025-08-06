use std::collections::HashSet;

use crate::structures::{entitlement::Entitlement, field::Numericity, variant::Variant};

#[derive(Debug, Clone)]
pub struct AccessProperties {
    pub numericity: Numericity,
    pub entitlements: HashSet<Entitlement>,
    pub effects: (),
}

impl AccessProperties {
    pub fn enumerated(variants: impl IntoIterator<Item = Variant>) -> Self {
        Self {
            numericity: Numericity::enumerated(variants),
            entitlements: HashSet::new(),
            effects: (),
        }
    }

    pub fn numeric() -> Self {
        Self {
            numericity: Numericity::Numeric,
            entitlements: HashSet::new(),
            effects: (),
        }
    }

    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        self.entitlements.extend(entitlements);
        self
    }
}

#[derive(Debug, Clone)]
pub enum ReadWrite {
    Symmetrical(AccessProperties),
    Asymmetrical {
        read: AccessProperties,
        write: AccessProperties,
    },
}

#[derive(Debug, Clone)]
pub enum Access {
    Read(AccessProperties),
    Write(AccessProperties),
    ReadWrite(ReadWrite),
}

impl Access {
    pub fn read(numericity: Numericity) -> Access {
        Access::Read(AccessProperties {
            numericity,
            entitlements: HashSet::new(),
            effects: (),
        })
    }

    pub fn write(numericity: Numericity) -> Access {
        Access::Write(AccessProperties {
            numericity,
            entitlements: HashSet::new(),
            effects: (),
        })
    }

    pub fn read_write(numericity: Numericity) -> Access {
        Access::ReadWrite(ReadWrite::Symmetrical(AccessProperties {
            numericity: numericity.clone(),
            entitlements: HashSet::new(),
            effects: (),
        }))
    }

    pub fn read_write_asymmetrical(
        read_numericity: Numericity,
        write_numericity: Numericity,
    ) -> Access {
        Access::ReadWrite(ReadWrite::Asymmetrical {
            read: AccessProperties {
                numericity: read_numericity,
                entitlements: HashSet::new(),
                effects: (),
            },
            write: AccessProperties {
                numericity: write_numericity,
                entitlements: HashSet::new(),
                effects: (),
            },
        })
    }

    pub fn get_read(&self) -> Option<&AccessProperties> {
        if let Self::Read(read)
        | Self::ReadWrite(
            ReadWrite::Symmetrical(read) | ReadWrite::Asymmetrical { read, .. },
        ) = self
        {
            Some(read)
        } else {
            None
        }
    }

    pub fn get_write(&self) -> Option<&AccessProperties> {
        if let Self::Write(write)
        | Self::ReadWrite(
            ReadWrite::Symmetrical(write) | ReadWrite::Asymmetrical { write, .. },
        ) = self
        {
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
