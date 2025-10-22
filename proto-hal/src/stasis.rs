pub trait State<Parent> {}

#[diagnostic::on_unimplemented(
    message = "`{Self}` has entitlements, but `{State}` is not one of them",
    label = "must be an entitlement of `{Self}`",
    // note = "learn more: <docs link>"
)]
pub unsafe trait Entitled<State> {}

/// A marker type for an unavailable resource.
pub struct Unavailable;

impl<F> State<F> for Unavailable {}

/// A marker type for a dynamic state.
pub struct Dynamic {
    _sealed: (),
}

impl<F> State<F> for Dynamic {}

macro_rules! numerics {
    {
        $($name:ident ($ty:ty) $(,)?)*
    } => {
        $(
            pub struct $name<const V: $ty> {
                _sealed: (),
            }

            impl<F, const V: $ty> State<F> for $name<V> {}
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
