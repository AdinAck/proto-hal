//! Types supporting the rest of the crate.

use derive_more::Display;

pub use source::{SourceId, Span, Spanned};

/// The result of parsing a node within a sequence, either the node or an [`Error`](Parsed::Error) spanning what was
/// skipped where it failed. A diagnostic reports the failure, and the sequence continues.
///
/// *Note: An [`Error`](Parsed::Error) displays as `<error>`, as the source text it spans is not kept.*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Parsed<T> {
    /// A node that parsed.
    Node(T),
    /// A failure to parse.
    #[display("<error>")]
    Error,
}
