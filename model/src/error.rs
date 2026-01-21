use crate::entitlement::PatternError;

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
