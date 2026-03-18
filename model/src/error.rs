use crate::entitlement::PatternError;

pub type Result<T> = ::core::result::Result<T, Error>;

/// Elaborates errors that may be emitted during model generation.
#[derive(Debug, Clone)]
pub enum Error {
    Pattern(PatternError),
}

impl From<PatternError> for Error {
    fn from(value: PatternError) -> Self {
        Self::Pattern(value)
    }
}
