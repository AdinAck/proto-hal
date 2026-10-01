//! Tokens lexed from the source text by the [lexer](crate::lexer).
//!
//! Some tokens like [identifiers](Token::Ident) and [doc comments](Token::Doc) retain a slice of the corresponding
//! source text.

use derive_more::{Deref, Display, From};
use std::fmt;

pub use Keyword::*;
pub use Operator::*;
pub use Punctuation::*;

/// A token lexed from some slice of source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, From)]
pub enum Token<'src> {
    /// A [`Literal`].
    Literal(Literal),
    /// An [`Ident`].
    Ident(Ident<'src>),
    /// A [`Doc`].
    Doc(Doc<'src>),
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
/// Written in decimal, or after a prefix in hexadecimal (`0x`), binary (`0b`), or octal (`0o`).
/// Digits may be separated by underscores i.e. `0x0400_0000`. The value must fit in 32 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deref, Display)]
pub struct Literal(pub(crate) u32);

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
/// Everything after `///` to the end of the line, with surrounding whitespace trimmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deref, Display)]
#[deref(forward)]
pub struct Doc<'src>(pub(crate) &'src str);

/// A keyword token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

/// An operator token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// The operator "|".
    Pipe,
    /// The operator "~".
    Tilde,
    /// The operator "@".
    At,
    /// The operator "&".
    Amp,
    /// The operator "-".
    Minus,
    /// The operator ".".
    Dot,
    /// The operator "..".
    DotDot,
    /// The operator "..=".
    DotDotEq,
    /// The operator "...".
    Ellipsis,
    /// The operator ":".
    Colon,
    /// The operator "#".
    Hash,
}

/// A punctuation token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Punctuation {
    /// The punctuation ",".
    Comma,
    /// The punctuation "{".
    LBrace,
    /// The punctuation "}".
    RBrace,
    /// The punctuation "[".
    LBracket,
    /// The punctuation "]".
    RBracket,
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Literal(literal) => write!(f, "{literal}"),
            Self::Ident(ident) => write!(f, "{ident}"),
            Self::Doc(..) => write!(f, "doc comment"),
            Self::Keyword(Device) => write!(f, "device"),
            Self::Keyword(Import) => write!(f, "import"),
            Self::Keyword(Peripheral) => write!(f, "peripheral"),
            Self::Keyword(Register) => write!(f, "register"),
            Self::Keyword(Field) => write!(f, "field"),
            Self::Keyword(Schema) => write!(f, "schema"),
            Self::Keyword(Variant) => write!(f, "variant"),
            Self::Keyword(Group) => write!(f, "group"),
            Self::Keyword(Interrupts) => write!(f, "interrupts"),
            Self::Keyword(Requires) => write!(f, "requires"),
            Self::Keyword(Read) => write!(f, "read"),
            Self::Keyword(Write) => write!(f, "write"),
            Self::Keyword(Store) => write!(f, "store"),
            Self::Keyword(Volatile) => write!(f, "volatile"),
            Self::Keyword(Hardware) => write!(f, "hardware"),
            Self::Keyword(Leaky) => write!(f, "leaky"),
            Self::Keyword(Inert) => write!(f, "inert"),
            Self::Keyword(As) => write!(f, "as"),
            Self::Keyword(By) => write!(f, "by"),
            Self::Keyword(In) => write!(f, "in"),
            Self::Keyword(Refines) => write!(f, "refines"),
            Self::Keyword(Assumes) => write!(f, "assumes"),
            Self::Keyword(Reset) => write!(f, "reset"),
            Self::Operator(Pipe) => write!(f, "|"),
            Self::Operator(Tilde) => write!(f, "~"),
            Self::Operator(At) => write!(f, "@"),
            Self::Operator(Amp) => write!(f, "&"),
            Self::Operator(Minus) => write!(f, "-"),
            Self::Operator(Dot) => write!(f, "."),
            Self::Operator(DotDot) => write!(f, ".."),
            Self::Operator(DotDotEq) => write!(f, "..="),
            Self::Operator(Ellipsis) => write!(f, "..."),
            Self::Operator(Colon) => write!(f, ":"),
            Self::Operator(Hash) => write!(f, "#"),
            Self::Punctuation(Comma) => write!(f, ","),
            Self::Punctuation(LBrace) => write!(f, "{{"),
            Self::Punctuation(RBrace) => write!(f, "}}"),
            Self::Punctuation(LBracket) => write!(f, "["),
            Self::Punctuation(RBracket) => write!(f, "]"),
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
/// literal![42] // expands to: `Token::Literal(Literal(42))`
/// ```
///
/// This allows the literal token to be used as a pattern:
///
/// ```ignore
/// match token {
///     literal![0] => "zero",
///     literal![n] => "some other literal",
///     _ => "not a literal",
/// }
/// ```
macro_rules! literal {
    [$value: tt] => {
        $crate::token::Token::Literal($crate::token::Literal($value))
    };
}

pub(crate) use literal;

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
