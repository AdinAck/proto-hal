//! Structures related to operating on MIOML source text.

#![deny(missing_docs)]

use std::ops::Range;

/// Identifies a source file: an index into the loaded sources.
pub type SourceId = usize;

/// A byte range within a source file, carrying the [`SourceId`] of the file it points into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// The file the span points into.
    pub source: SourceId,
    /// The offset of the span's first byte.
    pub start: usize,
    /// The offset just past the span's last byte.
    pub end: usize,
}

impl Span {
    /// The span's byte range within its source.
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

#[cfg(feature = "chumsky")]
impl chumsky::span::Span for Span {
    type Context = SourceId;
    type Offset = usize;

    fn new(source: SourceId, range: Range<usize>) -> Self {
        Self {
            source,
            start: range.start,
            end: range.end,
        }
    }

    fn context(&self) -> SourceId {
        self.source
    }

    fn start(&self) -> usize {
        self.start
    }

    fn end(&self) -> usize {
        self.end
    }
}

#[cfg(feature = "chumsky")]
impl<T> chumsky::span::WrappingSpan<T> for Span {
    type Spanned = chumsky::span::Spanned<T, Self>;

    fn make_wrapped(self, inner: T) -> Self::Spanned {
        chumsky::span::Spanned { inner, span: self }
    }

    fn inner_of(spanned: &Self::Spanned) -> &T {
        &spanned.inner
    }

    fn span_of(spanned: &Self::Spanned) -> &Self {
        &spanned.span
    }
}
