//! Structures related to operating on MIOML source text.

#![deny(missing_docs)]

use std::{
    fmt,
    ops::{Index, Range},
    path::PathBuf,
};

use derive_more::{Deref, DerefMut, Display};
use elsa::FrozenVec;

/// The loaded source files, each identified by its [`SourceId`].
#[derive(Default)]
pub struct Sources {
    files: FrozenVec<Box<File>>,
}

/// A source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The path the file was read from.
    pub path: PathBuf,
    /// The text of the file.
    pub text: String,
}

impl Sources {
    /// Add a source file, producing its [`SourceId`] along with the file handle.
    pub fn add(&self, path: PathBuf, text: String) -> (SourceId, &File) {
        let source = self.files.len();

        (source, self.files.push_get(Box::new(File { path, text })))
    }
}

impl fmt::Debug for Sources {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.files.iter()).finish()
    }
}

impl Index<SourceId> for Sources {
    type Output = File;

    fn index(&self, source: SourceId) -> &File {
        &self.files[source]
    }
}

/// The index of a source file among the [loaded sources](Sources).
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

/// `T` paired with its originating span from the source text. Dereferences to the inner `T`, and displays as it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deref, DerefMut, Display)]
#[display("{inner}")]
pub struct Spanned<T> {
    /// The value.
    #[deref]
    #[deref_mut]
    pub inner: T,
    /// The span of the value in the source text.
    pub span: Span,
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
    type Spanned = Spanned<T>;

    fn make_wrapped(self, inner: T) -> Self::Spanned {
        Spanned { inner, span: self }
    }

    fn inner_of(spanned: &Self::Spanned) -> &T {
        &spanned.inner
    }

    fn span_of(spanned: &Self::Spanned) -> &Self {
        &spanned.span
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{Sources, Span, Spanned};

    #[test]
    fn sources() {
        let sources = Sources::default();

        let (foo, first) = sources.add("foo.mio".into(), "register foo".into());
        let (bar, ..) = sources.add("bar.mio".into(), "import foo".into());

        assert_eq!(
            first.text, "register foo",
            "a file should remain borrowed while another is added"
        );
        assert_eq!(
            (foo, bar),
            (0, 1),
            "files should be identified in the order added"
        );
        assert_eq!(
            sources[bar].path,
            Path::new("bar.mio"),
            "a file should be found by its id"
        );
    }

    #[test]
    fn spanned() {
        let spanned = Spanned {
            inner: 42,
            span: Span {
                source: 0,
                start: 0,
                end: 2,
            },
        };

        assert_eq!(
            *spanned, 42,
            "a spanned value should dereference to the value"
        );
        assert_eq!(
            spanned.to_string(),
            "42",
            "a spanned value should display as the value"
        );
    }
}
