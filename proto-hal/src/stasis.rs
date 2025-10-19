pub trait State<Parent>: Conjure {}

pub unsafe trait Entitled<State> {}

pub trait PartialConjure {
    type Target;

    /// # Safety
    /// Produce a value where the invariants of the value's existance
    /// are upheld by the user.
    unsafe fn partial_conjure() -> Self::Target;
}

pub trait Conjure {
    /// # Safety
    /// Produce a value where the invariants of the value's existence
    /// are upheld by the user.
    unsafe fn conjure() -> Self;
}

impl<T> PartialConjure for T
where
    T: Conjure,
{
    type Target = Self;

    unsafe fn partial_conjure() -> Self::Target {
        unsafe { Self::conjure() }
    }
}

/// A marker type for an unavailable resource.
pub struct Unavailable;

/// A marker type for a dynamic state.
pub struct Dynamic {
    _sealed: (),
}

pub struct Value<const V: u32> {
    _sealed: (),
}

impl Conjure for Unavailable {
    unsafe fn conjure() -> Self {
        Unavailable
    }
}

impl<F> State<F> for Unavailable {}

impl Conjure for Dynamic {
    unsafe fn conjure() -> Self {
        Dynamic { _sealed: () }
    }
}

impl<F> State<F> for Dynamic {}

impl<const V: u32> Conjure for Value<V> {
    unsafe fn conjure() -> Self {
        Self { _sealed: () }
    }
}

impl<F, const V: u32> State<F> for Value<V> {}
