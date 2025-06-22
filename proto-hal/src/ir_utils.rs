pub use arbitrary_int;

#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub struct RegisterValue(u32);

impl RegisterValue {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn word(self) -> u32 {
        self.0
    }
}

impl RegisterValue {
    pub fn bool(&self, offset: u8) -> bool {
        match (self.0 >> offset) & 1 {
            0 => false,
            1 => true,
            _ => unreachable!(),
        }
    }

    pub fn region(&self, offset: u8, width: u8) -> u32 {
        (self.0 >> offset) & (u32::MAX >> (32 - width))
    }
}

macro_rules! impl_uint_standard {
    ($ux:ident, $width:expr) => {
        impl RegisterValue {
            pub fn $ux(&self, offset: u8) -> $ux {
                unsafe {
                    ((self.0 >> offset) & (u32::MAX >> (32 - $width)))
                        .try_into()
                        .unwrap_unchecked()
                }
            }
        }
    };
}

macro_rules! impl_uint_special {
    ($ux:ident, $width:expr) => {
        impl RegisterValue {
            pub fn $ux(&self, offset: u8) -> $ux {
                unsafe {
                    $ux::new_unchecked(
                        ((self.0 >> offset) & (u32::MAX >> (32 - $width)))
                            .try_into()
                            .unwrap_unchecked(),
                    )
                }
            }
        }
    };
}

use arbitrary_int::*;

impl_uint_special!(u2, 2);
impl_uint_special!(u3, 3);
impl_uint_special!(u4, 4);
impl_uint_special!(u5, 5);
impl_uint_special!(u6, 6);
impl_uint_special!(u7, 7);
impl_uint_standard!(u8, 8);
impl_uint_special!(u9, 9);
impl_uint_special!(u10, 10);
impl_uint_special!(u11, 11);
impl_uint_special!(u12, 12);
impl_uint_special!(u13, 13);
impl_uint_special!(u14, 14);
impl_uint_special!(u15, 15);
impl_uint_standard!(u16, 16);
impl_uint_special!(u17, 17);
impl_uint_special!(u18, 18);
impl_uint_special!(u19, 19);
impl_uint_special!(u20, 20);
impl_uint_special!(u21, 21);
impl_uint_special!(u22, 22);
impl_uint_special!(u23, 23);
impl_uint_special!(u24, 24);
impl_uint_special!(u25, 25);
impl_uint_special!(u26, 26);
impl_uint_special!(u27, 27);
impl_uint_special!(u28, 28);
impl_uint_special!(u29, 29);
impl_uint_special!(u30, 30);
impl_uint_special!(u31, 31);
impl_uint_standard!(u32, 32);

pub trait AsBuilder: Into<Self::Builder> {
    type Builder;
}

pub trait AsRegister: Into<Self::Register> {
    type Register;
}

// Type-state indicating the state cannot
// be statically determined currently.
pub struct Unresolved;
