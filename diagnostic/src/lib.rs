//! Rich diagnostics emitted by multiple stages of the MIOML language.

#![deny(missing_docs)]

use derive_more::{Deref, Display};
use source::Span;

/// A diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The diagnostic's [`Id`].
    pub id: Id,
    /// The diagnostic's message.
    pub message: String,
    /// The diagnostic's primary label.
    pub primary_label: Label,
    /// The diagnostic's supporting labels.
    pub supporting_labels: Vec<Label>,
    /// The diagnostic's notes.
    pub notes: Vec<String>,
}

impl Diagnostic {
    /// Create an error diagnostic.
    pub fn error(kind: impl Kind, message: impl Into<String>, primary_label: Label) -> Self {
        Self::new(kind, Severity::Error, message, primary_label)
    }

    /// Create a warning diagnostic.
    pub fn warning(kind: impl Kind, message: impl Into<String>, primary_label: Label) -> Self {
        Self::new(kind, Severity::Warning, message, primary_label)
    }

    fn new<K: Kind>(
        kind: K,
        severity: Severity,
        message: impl Into<String>,
        primary_label: Label,
    ) -> Self {
        Self {
            id: Id {
                severity,
                class: K::CLASS,
                code: kind.code(),
            },
            message: message.into(),
            primary_label,
            supporting_labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Attach a supporting label to the diagnostic.
    pub fn supporting_label(mut self, label: Label) -> Self {
        self.supporting_labels.push(label);
        self
    }

    /// Attach a note to the diagnostic.
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Determine whether the diagnostic is of `kind`.
    pub fn is<K: Kind>(&self, kind: K) -> bool {
        self.id.class == K::CLASS && self.id.code == kind.code()
    }
}

/// A diagnostic kind denotes a specific diagnostic message template.
pub trait Kind: Copy {
    /// The kind's class.
    const CLASS: Class;

    /// The kind's code within its class.
    fn code(self) -> Code;
}

/// A message attached to a source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// The label's message.
    pub message: String,
    /// The labelled span.
    pub span: Span,
}

impl Label {
    /// Create a diagnostic label.
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

/// A diagnostic's identifier i.e. `E1003` encoding its severity, class, and code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[display("{severity}{}{code}", *class as u8)]
pub struct Id {
    /// The diagnostic's severity.
    pub severity: Severity,
    /// The diagnostic's class.
    pub class: Class,
    /// The diagnostic's code within its class.
    pub code: Code,
}

/// The diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
pub enum Severity {
    /// An error diagnostic.
    #[display("E")]
    Error,
    /// A warning diagnostic.
    #[display("W")]
    Warning,
}

/// A diagnostic class indicates which stage of evaluation emitted the diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// Diagnostics of lexing and parsing.
    Syntax = 1,
    /// Diagnostics of the structure of a device model.
    Structural = 3,
}

/// A diagnostic code, pertaining to the diagnostic [`Kind`].
///
/// *Note: Codes are limited to three digits.*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deref, Display)]
#[display("{_0:03}")]
pub struct Code(u16);

impl Code {
    /// Create a diagnostic code from an integer.
    ///
    /// # Panics
    ///
    /// If `code` has more than three digits.
    ///
    /// Can be used in `const` contexts for compile-time validation:
    /// ```compile_fail
    /// # use proto_hal_diagnostic::Code;
    /// const { Code::new(1000) };
    /// ```
    pub const fn new(code: u16) -> Self {
        assert!(code <= 999, "codes may not exceed three digits");

        Self(code)
    }
}

/// Find the candidate closest in spelling to `found`, if any is close enough for `found` to likely be its misspelling.
///
/// *Note: As in rustc, a misspelling is within an edit distance of a third of its length, and at least one.*
pub fn closest<T: AsRef<str>>(found: &str, candidates: impl IntoIterator<Item = T>) -> Option<T> {
    let threshold = found.chars().count().max(3) / 3;

    candidates
        .into_iter()
        .map(|candidate| (strsim::levenshtein(found, candidate.as_ref()), candidate))
        .filter(|(distance, ..)| *distance <= threshold)
        .min_by_key(|(distance, ..)| *distance)
        .map(|(.., candidate)| candidate)
}

#[cfg(test)]
mod tests {
    use source::Span;

    use crate::{Class, Code, Diagnostic, Id, Label, Severity, closest};

    #[derive(Debug, Clone, Copy)]
    enum Kind {
        Foo = 7,
        Bar,
    }

    impl crate::Kind for Kind {
        const CLASS: Class = Class::Syntax;

        fn code(self) -> Code {
            Code::new(self as _)
        }
    }

    #[test]
    fn diagnostic_id() {
        let span = Span {
            source: 0,
            start: 0,
            end: 0,
        };

        for (diagnostic, expected) in [
            (
                Diagnostic::error(Kind::Foo, "foo", Label::new("foo", span)),
                "E1007",
            ),
            (
                Diagnostic::warning(Kind::Foo, "foo", Label::new("foo", span)),
                "W1007",
            ),
        ] {
            assert_eq!(
                diagnostic.id.to_string(),
                expected,
                "id doesn't match expected"
            );
        }
    }

    #[test]
    fn is() {
        let diagnostic = Diagnostic::error(
            Kind::Foo,
            "foo",
            Label::new(
                "foo",
                Span {
                    source: 0,
                    start: 0,
                    end: 0,
                },
            ),
        );

        assert!(
            diagnostic.is(Kind::Foo) && !diagnostic.is(Kind::Bar),
            "a diagnostic should be of the kind it was created with",
        );
    }
    #[test]
    fn id() {
        for (id, expected) in [
            (
                Id {
                    severity: Severity::Error,
                    class: Class::Syntax,
                    code: Code::new(3),
                },
                "E1003",
            ),
            (
                Id {
                    severity: Severity::Warning,
                    class: Class::Syntax,
                    code: Code::new(42),
                },
                "W1042",
            ),
        ] {
            assert_eq!(
                id.to_string(),
                expected,
                "id display doesn't match expected"
            );
        }
    }

    #[test]
    fn three_digits() {
        assert_eq!(*Code::new(999), 999, "999 should be a valid code");
    }

    #[test]
    #[should_panic(expected = "codes may not exceed three digits")]
    fn four_digits() {
        Code::new(1000);
    }

    #[test]
    fn misspellings() {
        for (found, expected) in [
            ("reed", Some("read")),
            ("regster", Some("register")),
            ("rad", Some("read")),
            ("foo", None),
            ("ab", None),
        ] {
            assert_eq!(
                closest(found, ["read", "write", "register"]),
                expected,
                "'{found}' should be a misspelling of {expected:?}",
            );
        }
    }
}
