pub mod diagnostic;
pub mod entitlement;
pub mod field;
pub mod interrupts;
pub mod model;
pub mod peripheral;
pub mod register;
pub mod variant;

pub use entitlement::{Entitlement, Entitlements};
pub use field::Field;
pub use interrupts::{Interrupt, Interrupts};
pub use model::Model;
pub use peripheral::Peripheral;
pub use register::Register;
pub use variant::Variant;

#[doc(hidden)]
pub trait Node {
    type Index;
}
