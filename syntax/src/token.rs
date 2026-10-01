//! Tokens lexed from the source text by the [lexer](crate::lexer).
//!
//! Some tokens like [literals](Token::Literal), [identifiers](Token::Ident), and [doc comments](Token::Doc) retain
//! a slice of the corresponding source text.

use derive_more::{Deref, Display, From, FromStr};
use std::fmt;

pub use Keyword::*;
pub use Operator::*;
pub use Punctuation::*;

/// A token lexed from some slice of source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, From)]
pub enum Token<'src> {
    /// A [`Literal`].
    Literal(Literal<'src>),
    /// An [`Ident`].
    Ident(Ident<'src>),
    /// A [`Doc`].
    Doc(Doc<'src>),
    /// A [`Doc`] documenting the file, written with `//!`.
    #[from(ignore)]
    FileDoc(Doc<'src>),
    /// A keyword i.e. `register`, `leaky`. (see [`Keyword`])
    Keyword(Keyword),
    /// An operator i.e. `|`, `~`, `@`, `&`. (see [`Operator`])
    Operator(Operator),
    /// A punctuation i.e. `,`, `{}`, `[]`. (see [`Punctuation`])
    Punctuation(Punctuation),

    /// A source text slice that corresponds with no [`Token`] value.
    ///
    /// *Note: Integer literal lexing failures are also represented by [`Unrecognized`](Token::Unrecognized).*
    #[from(ignore)]
    Unrecognized(&'src str),
}

/// An integer literal i.e. `42`, `0x2a`, `0b10_1010`, `0o52`.
///
/// Written in decimal, or after a prefix in hexadecimal (`0x`), binary (`0b`), or octal (`0o`). Dereferences to the
/// literal value, and implements [`Display`](fmt::Display) to reflect the source text.
///
/// *Note: Literal values are limited to 32 bits.*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deref, Display)]
#[display("{source}")]
pub struct Literal<'src> {
    /// The value.
    #[deref]
    pub(crate) value: u32,
    /// The literal as written in the source text i.e. `0x0400_0000`.
    pub(crate) source: &'src str,
}

/// An identifier i.e. `foo`, `_foo`, `theFoo42`.
///
/// An `XID_Start` character or `_`, followed by any number of `XID_Continue` characters, as defined
/// by [Unicode Standard Annex #31](https://www.unicode.org/reports/tr31/). A [`Keyword`] is not an
/// identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deref, Display)]
#[deref(forward)]
pub struct Ident<'src>(pub(crate) &'src str);

/// The text of a doc comment i.e. `foo` in `/// foo`.
///
/// Everything after `///` or `//!` to the end of the line, with surrounding whitespace trimmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deref, Display)]
#[deref(forward)]
pub struct Doc<'src>(pub(crate) &'src str);

/// A keyword token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, FromStr)]
#[display(rename_all = "lowercase")]
#[from_str(rename_all = "lowercase")]
pub enum Keyword {
    /// The keyword "device".
    Device,
    /// The keyword "import".
    Import,
    /// The keyword "peripheral".
    Peripheral,
    /// The keyword "register".
    Register,
    /// The keyword "field".
    Field,
    /// The keyword "schema".
    Schema,
    /// The keyword "variant".
    Variant,
    /// The keyword "group".
    Group,
    /// The keyword "interrupts".
    Interrupts,
    /// The keyword "requires".
    Requires,
    /// The keyword "read".
    Read,
    /// The keyword "write".
    Write,
    /// The keyword "store".
    Store,
    /// The keyword "volatile".
    Volatile,
    /// The keyword "hardware".
    Hardware,
    /// The keyword "leaky".
    Leaky,
    /// The keyword "inert".
    Inert,
    /// The keyword "as".
    As,
    /// The keyword "by".
    By,
    /// The keyword "in".
    In,
    /// The keyword "refines".
    Refines,
    /// The keyword "assumes".
    Assumes,
    /// The keyword "reset".
    Reset,
    /// The keyword "self".
    Self_,
}

/// An operator token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Operator {
    /// The operator "|".
    #[display("|")]
    Pipe,
    /// The operator "~".
    #[display("~")]
    Tilde,
    /// The operator "@".
    #[display("@")]
    At,
    /// The operator "&".
    #[display("&")]
    Amp,
    /// The operator "-".
    #[display("-")]
    Minus,
    /// The operator ".".
    #[display(".")]
    Dot,
    /// The operator "..".
    #[display("..")]
    DotDot,
    /// The operator "..=".
    #[display("..=")]
    DotDotEq,
    /// The operator "...".
    #[display("...")]
    Ellipsis,
    /// The operator ":".
    #[display(":")]
    Colon,
    /// The operator "::".
    #[display("::")]
    ColonColon,
    /// The operator "#".
    #[display("#")]
    Hash,
}

/// A punctuation token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Punctuation {
    /// The punctuation ",".
    #[display(",")]
    Comma,
    /// The punctuation "{".
    #[display("{{")]
    LBrace,
    /// The punctuation "}".
    #[display("}}")]
    RBrace,
    /// The punctuation "[".
    #[display("[")]
    LBracket,
    /// The punctuation "]".
    #[display("]")]
    RBracket,
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Literal(literal) => write!(f, "{literal}"),
            Self::Ident(ident) => write!(f, "{ident}"),
            Self::Doc(doc) if doc.is_empty() => write!(f, "///"),
            Self::Doc(doc) => write!(f, "/// {doc}"),
            Self::FileDoc(doc) if doc.is_empty() => write!(f, "//!"),
            Self::FileDoc(doc) => write!(f, "//! {doc}"),
            Self::Keyword(keyword) => write!(f, "{keyword}"),
            Self::Operator(operator) => write!(f, "{operator}"),
            Self::Punctuation(punctuation) => write!(f, "{punctuation}"),
            Self::Unrecognized(s) => write!(f, "{s}"),
        }
    }
}

macro_rules! impl_token {
    {
        $(operators {$(
            ($op:tt) => $op_name:ident $(,)?
        )*})?

        $(punctuation {$(
            ($punct:tt) => $punct_name:ident $(,)?
        )*})?
    } => {
        /// Convenience macro to create *atomic* [`Token`]s.
        ///
        /// ```ignore
        /// token![Device] // expands to: `Token::from(token::Device)`
        /// token![.] // expands to: `Token::Operator(token::Dot)`
        /// token![,] // expands to: `Token::Punctuation(token::Comma)`
        /// token![LBrace] // expands to: `Token::from(token::LBrace)`
        /// ```
        ///
        /// For non-atomic tokens, see [`ident!`](ident), [`keyword!`](keyword), and [`literal!`](literal).
        ///
        /// *Note: Some punctuation (braces and brackets) must be referred to by name, i.e. "LBrace" for
        /// `{`.*
        macro_rules! token {
            [$i: ident] => {
                $crate::token::Token::from($crate::token::$i)
            };

            $($(
                [$op] => {
                    $crate::token::Token::Operator($crate::token::$op_name)
                };
            )*)?

            $($(
                [$punct] => {
                    $crate::token::Token::Punctuation($crate::token::$punct_name)
                };
            )*)?
        }

        pub(crate) use token;
    };
}

impl_token! {
    operators {
        (|) => Pipe,
        (~) => Tilde,
        (@) => At,
        (&) => Amp,
        (-) => Minus,
        (.) => Dot,
        (..) => DotDot,
        (..=) => DotDotEq,
        (...) => Ellipsis,
        (:) => Colon,
        (::) => ColonColon,
        (#) => Hash,
    }

    punctuation {
        (,) => Comma,
    }
}

/// Convenience macro for [keyword](Token::Keyword) tokens.
///
/// ```ignore
/// keyword![Device] // expands to: `Token::Keyword(Keyword::Device)`
/// token![Device] // expands to: `Token::from(Keyword::Device)`
/// ```
///
/// This allows the keyword token to be used as a pattern:
///
/// ```ignore
/// match token {
///     keyword![Write] => "write",
///     keyword![Read] => "read",
///     _ => "invalid",
/// }
/// ```
macro_rules! keyword {
    [$ident: ident] => {
        $crate::token::Token::Keyword($crate::token::$ident)
    };
}

pub(crate) use keyword;

/// Convenience macro for [literal](Token::Literal) tokens.
///
/// ```ignore
/// literal![0x2a] // expands to: `Token::Literal(bare_literal![0x2a])`
/// ```
#[cfg(test)]
macro_rules! literal {
    [$value: tt] => {
        $crate::token::Token::Literal($crate::token::bare_literal![$value])
    };
}

#[cfg(test)]
pub(crate) use literal;

/// Convenience macro for [literals](Literal).
///
/// ```ignore
/// bare_literal![0x2a] // expands to: `Literal { value: 0x2a, source: "0x2a" }`
/// ```
#[cfg(test)]
macro_rules! bare_literal {
    [$value: tt] => {
        $crate::token::Literal {
            value: $value,
            source: stringify!($value),
        }
    };
}

#[cfg(test)]
pub(crate) use bare_literal;

/// Convenience macro for [identifier](Token::Ident) tokens.
///
/// ```ignore
/// ident!["foo"] // expands to: `Token::Ident(Ident("foo"))`
/// ```
///
/// This allows the ident token to be used as a pattern:
///
/// ```ignore
/// match token {
///     ident!["foo"] => "is foo",
///     ident![s] => s,
///     _ => "not an identifier",
/// }
/// ```
macro_rules! ident {
    [$text: tt] => {
        $crate::token::Token::Ident($crate::token::Ident($text))
    };
}

pub(crate) use ident;
