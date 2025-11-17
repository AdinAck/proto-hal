pub mod entitlement;
pub mod field;
pub mod interrupts;
pub mod model;
pub mod peripheral;
pub mod register;
pub mod variant;

pub trait Node {
    type Index;
}
