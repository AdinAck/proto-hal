/// Represents a vector table entry, i.e. a function pointer.
///
/// *Note: An empty entry (reserved) is the null pointer.*
#[repr(transparent)]
pub struct Vector(Option<unsafe extern "C" fn()>);

impl Vector {
    /// Create a vector with the provided function pointer.
    pub const fn handler(f: unsafe extern "C" fn()) -> Self {
        Self(Some(f))
    }

    /// Create a vector that is reserved.
    pub const fn reserved() -> Self {
        Self(None)
    }
}

/// # Safety
///
/// This impl is needed due to the underlying *const ()` value. This value is never
/// read, and as such is `Sync`.
unsafe impl Sync for Vector {}
