//! This module contains the infrastructure proto-hal employs to coerce the Rust compiler into
//! enforcing *stasis*.
//!
//! Stasis is the foundational principle upon which proto-hal operates. The idea that
//! types can portray hardware state, and transitioning *state* is represented as transmuting
//! *type*.

use core::marker::PhantomData;

/// Implementors of this trait are type-states corresponding to some parent resource
/// with a physical value.
///
/// # Safety
/// Inherits from [`State`].
///
/// Implementing this trait is a contract that the implementor is a type-state of the
/// parent and the assigned physical value is correct.
/// If this is untrue, [stasis](TODO: link docs) is broken, which ultimately results
/// in undefined behavior.
pub unsafe trait Physical<Parent>: State<Parent> {
    /// The physical value the state denotes.
    const VALUE: u32;
}

/// Implementors of this trait are type-states corresponding to some parent resource.
///
/// # Safety
/// Implementing this trait is a contract that the implementor is a type-state of the
/// parent. If this is untrue, [stasis](TODO: link docs) is broken, which ultimately
/// results in undefined behavior.
pub unsafe trait State<Parent>: Conjure {}

/// Implementors of this trait are entitlements of the specified pattern.
pub unsafe trait Entitlement<P: Pattern> {}

/// Implementors of this trait are entitlement patterns on a particular axis.
pub unsafe trait Pattern {
    type Axis: Axis;
}

/// Implementors of this trait are entitlement axes.
pub unsafe trait Axis {}

pub mod axes {
    use super::Axis;

    pub struct Statewise;
    pub struct Affordance;
    pub struct Ontological;

    unsafe impl Axis for Statewise {}
    unsafe impl Axis for Affordance {}
    unsafe impl Axis for Ontological {}
}

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

impl<Resource, Key> Frozen<Resource, Key>
where
    Resource: Conjure,
{
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

impl<Resource, Key> Conjure for Frozen<Resource, Key>
where
    Resource: Conjure,
{
    unsafe fn conjure() -> Self {
        Self {
            resource: unsafe { Conjure::conjure() },
            _key: PhantomData,
        }
    }
}

/// A marker type for a dynamic state.
pub struct Dynamic {
    _sealed: (),
}

unsafe impl<Parent> State<Parent> for Dynamic {}

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
                pub fn value(&self) -> $ty {
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
