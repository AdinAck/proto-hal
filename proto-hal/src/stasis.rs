use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// A trait providing an interface to freeze stateful types.
pub trait Freeze: Sized {
    fn freeze<const N: usize>(self) -> (Frozen<Self, N>, [Entitlement<Self>; N]) {
        (
            Frozen { resource: self },
            core::array::from_fn(|_| Entitlement { _p: PhantomData }), // this may introduce overhead, will have to investigate (seems not to)
        )
    }
}

/// A struct to represent an entitlement
/// to a type frozen in a particular state.
pub struct Entitlement<Resource>
where
    Resource: Freeze,
{
    _p: PhantomData<Resource>,
}

impl<P> From<P> for Entitlement<P>
where
    P: Freeze,
{
    fn from(_: P) -> Self {
        Self { _p: PhantomData }
    }
}

/// A struct to hold stateful types where
/// the state is frozen.
pub struct Frozen<Resource, const ENTITLEMENTS: usize>
where
    Resource: Freeze,
{
    resource: Resource,
}

impl<Resource: Freeze, const ENTITLEMENTS: usize> Deref for Frozen<Resource, ENTITLEMENTS> {
    type Target = Resource;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}

impl<Resource: Freeze, const ENTITLEMENTS: usize> DerefMut for Frozen<Resource, ENTITLEMENTS> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.resource
    }
}

impl<Resource: Freeze, const ENTITLEMENTS: usize> Frozen<Resource, ENTITLEMENTS> {
    pub fn release(
        self,
        #[expect(unused)] entitlements: [Entitlement<Resource>; ENTITLEMENTS],
    ) -> Resource {
        self.resource
    }
}

pub trait EntitlementLock: Sized {
    type Resource: Freeze;
}

impl<Resource: Freeze> EntitlementLock for Resource {
    type Resource = Self;
}

impl<Resource: Freeze> EntitlementLock for Entitlement<Resource> {
    type Resource = Resource;
}

/// Indicates a type-state is
/// entitled to another type-state.
///
/// # Safety
///
/// If a type implements this trait
/// erroneously, the generated
/// peripheral interfaces will be invalid.
pub unsafe trait Entitled<State> {}

/// A marker type for
/// an unsatisfied entitlement.
pub struct Unsatisfied;

/// A marker type for
/// an unavailable resource.
pub struct Unavailable;

/// A marker type for an unresolved state.
pub struct Unresolved;

/// To satisfy state-wise entitlement constrains when the states are not tracked,
/// this impl is needed.
unsafe impl Entitled<Self> for Unresolved {}

pub trait Conjure {
    /// # Safety
    /// Produce a value where the invariants of the value's existence
    /// are upheld by the user.
    unsafe fn conjure() -> Self;
}

pub trait Emplace<Writer> {
    fn set(&self, w: &mut Writer);
}

// Effectively "!Unresolved".
pub trait Corporeal {}

pub trait Position<T> {}
pub trait Outgoing<T>: Position<T> {}
pub trait Incoming<T>: Position<T> + Corporeal + Conjure {
    type Raw;
    const RAW: Self::Raw;
}

impl Conjure for Unresolved {
    unsafe fn conjure() -> Self {
        Self
    }
}

impl<Writer> Emplace<Writer> for Unresolved {
    fn set(&self, #[expect(unused)] w: &mut Writer) {
        // do nothing
    }
}

impl<T> Position<T> for Unresolved {}

impl Conjure for Unavailable {
    unsafe fn conjure() -> Self {
        Self
    }
}

impl<Writer> Emplace<Writer> for Unavailable {
    fn set(&self, #[expect(unused)] w: &mut Writer) {
        // do nothing
    }
}

impl Corporeal for Unavailable {}
impl<T> Position<T> for Unavailable {}
