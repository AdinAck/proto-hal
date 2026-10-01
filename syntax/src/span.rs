//! Locations in the source text.

pub use source::{SourceId, Span};

/// `T` paired with its originating span from the source text.
pub type Spanned<T> = chumsky::span::Spanned<T, Span>;
