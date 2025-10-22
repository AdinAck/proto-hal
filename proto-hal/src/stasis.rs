//! This module contains the infrastructure proto-hal employs to coerce the Rust compiler into
//! enforcing *stasis*.
//!
//! Stasis is the foundational principle upon which proto-hal operates. The idea that
//! types can portray hardware state, and transitioning *state* is represented as transmuting
//! *type*.

/// Implementors of this trait are type-states corresponding to some parent resource.
///
/// # Safety
/// Implementing this trait is a contract that the implementor is a type-state of the parent.
/// If this is untrue, [stasis](TODO: link docs) is broken, which ultimately results in
/// undefined behavior.
pub unsafe trait State<Parent> {}

/// Implementors of this trait are type-stated resources with entitlement constraints.
/// Many kinds of resources can be entitled in any of the following ways:
///
/// ## Statewise
/// State inhabitancy can be dependent on other state(s) inhabitancy.
///
/// ### Example
/// If a state of a field is entitled to a some set of other states in other fields, then
/// transitioning *to* this state requires proof that the dependency states will be inhabited
/// when the transition is complete.
///
/// ## Affordance
/// The ability to and quality of interacting with a field can be dependent on state(s)
/// inhabitancy.
///
/// ### Example
/// If write access to a field is entitled to some set of other states in other fields, then
/// *writing to* this field requires proof that the dependency states are inhabited.
///
/// ## Ontology
/// The existance a field can be dependent on state(s) inhabitancy.
///
/// ### Example
/// The interpretation of the bits in a register is not always fixed. In other words, the
/// *fields* of a register can change. Some fields within the same register may be
/// superpositioned if the fields themselves are entitled to complementary states.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has entitlements, but `{Locus}` is not one of them",
    label = "must be an entitlement of `{Self}`",
    // note = "learn more: <docs link>"
)]
pub unsafe trait Entitled<Locus> {}

/// A universal type-state indicating that the true hardware state is not statically
/// resolved currently.
pub struct Dynamic {
    _sealed: (),
}

unsafe impl<F> State<F> for Dynamic {}

macro_rules! numerics {
    {
        $($name:ident ($ty:ty) $(,)?)*
    } => {
        $(
            pub struct $name<const V: $ty> {
                _sealed: (),
            }

            unsafe impl<F, const V: $ty> State<F> for $name<V> {}
        )*
    };
}

numerics! {
    Bool(bool),
    UInt8(u8),
    Int8(i8),
    UInt16(u16),
    Int16(i16),
    UInt32(u32),
    Int32(i32),
}
