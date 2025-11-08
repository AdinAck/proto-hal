//! This module contains the infrastructure proto-hal employs to coerce the Rust compiler into
//! enforcing *stasis*.
//!
//! Stasis is the foundational principle upon which proto-hal operates. The idea that
//! types can portray hardware state, and transitioning *state* is represented as transmuting
//! *type*.

use core::marker::PhantomData;

/// Implementors of this trait are type-states corresponding to some parent resource.
///
/// # Safety
/// Implementing this trait is a contract that the implementor is a type-state of the parent.
/// If this is untrue, [stasis](TODO: link docs) is broken, which ultimately results in
/// undefined behavior.
pub unsafe trait State<Parent>: Conjure {
    /// The physical value the state denotes.
    const VALUE: u32;
}

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
///
/// # Safety
/// Implementing this trait is a contract that the implementor is a resource in which said resource
/// implementing this trait conforms to the device model.
/// If this is untrue, [stasis](TODO: link docs) is broken, which ultimately results in
/// undefined behavior.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has entitlements, but `{Locus}` is not one of them",
    label = "must be an entitlement of `{Self}`",
    // note = "learn more: <docs link>"
)]
pub unsafe trait Entitled<Locus> {}

/// Implementors of this trait are type-stated resources. Since device resources have no size,
/// they must be "conjured" when the type context requires them to be.
pub trait Conjure {
    /// Produce the resource out of thin air.
    ///
    /// # Safety
    ///
    /// If the production of the resource is contextually unsound (meaning it violates or is
    /// performed external to the device model) this action renders hardware invariance claims
    /// to be moot.
    unsafe fn conjure() -> Self;
}

/// A container for a resource that is forbidden from being *changed*.
pub struct Frozen<Resource, Key> {
    /// The resource which is frozen.
    resource: Resource,
    /// The resource consumed to unfreeze the frozen resource.
    _key: PhantomData<Key>,
}

impl<Resource, Key> Frozen<Resource, Key> {
    /// Freeze a resource, ensuring the resource is not destructively
    /// mutated or moved.
    ///
    /// # Safety
    ///
    /// If the specified key [`K`] is invalid given the entanglements of the resource,
    /// the invariances assumed by the freezing of the resource are rendered moot.
    pub unsafe fn freeze<K>(resource: Resource) -> Frozen<Resource, K> {
        Frozen {
            resource,
            _key: PhantomData,
        }
    }

    /// Provide the required key to retrieve the resource.
    pub fn unfreeze(self, #[expect(unused)] key: Key) -> Resource {
        self.resource
    }
}

/// A marker type for a dynamic state.
pub struct Dynamic {
    _sealed: (),
}

impl Conjure for Dynamic {
    unsafe fn conjure() -> Self {
        Dynamic { _sealed: () }
    }
}

macro_rules! numerics {
    {
        $($name:ident ($ty:ty) $(,)?)*
    } => {
        $(
            pub struct $name<const V: $ty> {
                _sealed: (),
            }

            impl<const V: $ty> Conjure for $name<V> {
                unsafe fn conjure() -> Self {
                    Self { _sealed: () }
                }
            }

            impl<const V: $ty> $name<V> {
                pub fn value() -> $ty {
                    V
                }
            }
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
