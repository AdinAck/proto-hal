#![no_std]

pub mod gpio;

#[cfg(feature = "stm32")]
pub mod stm32;

pub mod interrupt;
pub mod macro_utils;
pub mod prelude;
pub mod stasis;

/// Types that encapsulate a resource that can be configured to be
/// in a "reset" state implement this trait.
pub trait IntoReset {
    /// The form of the implementor type in the "reset" state.
    type Reset;

    /// Transform the implementor type into the "reset" state.
    fn into_reset(self) -> Self::Reset;
}
