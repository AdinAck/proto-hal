pub mod entitlement;
pub mod field;
pub mod hal;
pub mod interrupts;
pub mod peripheral;
pub mod register;
pub mod variant;

pub trait Node {
    type Index;
}
