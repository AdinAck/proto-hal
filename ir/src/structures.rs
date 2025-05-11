use std::ops::{Deref, DerefMut};
pub mod entitlement;
pub mod field;
pub mod hal;
pub mod interrupts;
pub mod peripheral;
pub mod register;
pub mod schema;
pub mod variant;

pub struct Validated<S> {
    structure: S,
}

impl<S> Deref for Validated<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.structure
    }
}

impl<S> DerefMut for Validated<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.structure
    }
}

pub trait Ident {
    fn ident(&self) -> &str;
}
