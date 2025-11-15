pub mod entitlement;
pub mod field;
pub mod hal;
pub mod interrupts;
pub mod peripheral;
pub mod register;
pub mod variant;

use syn::Ident;

pub trait ParentNode {
    type ChildIndex;

    fn add_child_index(&mut self, index: Self::ChildIndex, child_ident: Ident);
}
