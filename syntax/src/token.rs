//! Tokens lexed from the source text by the [lexer](crate::lexer) and parsed by the [parser](crate::parser).
//!
//! Some tokens like [identifiers](Token::Ident) and [doc comments](Token::Doc) retain a slice of the corresponding
//! source region.

use derive_more::From;
use std::fmt;

use crate::ast::Span;

pub use Keyword::*;
pub use Operator::*;
pub use Punctuation::*;

/// A token paired with its originating span from the source text.
pub type Spanned<'src> = (Token<'src>, Span);

/// A token lexed from some region of source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, From)]
pub enum Token<'src> {
    /// An integer literal i.e. `42`, `0xc4`, `0b1101`, `0o777`.
    Literal(u32),
    /// An identifier i.e. `gpioa`.
    Ident(&'src str),
    /// A doc comment: `/// ...`.
    #[from(ignore)]
    Doc(&'src str),
    /// A keyword i.e. `register`, `leaky`. (see [`Keyword`])
    Keyword(Keyword),
    /// An operator i.e. `|`, `~`, `@`, `&`. (see [`Operator`])
    Operator(Operator),
    /// A punctuation i.e. `,`, `{}`, `()`. (see [`Punctuation`])
    Punctuation(Punctuation),

    /// A source region that corresponds with no [`Token`] value.
    ///
    /// *Note: Integer literal lexing failures are also represented by [`Unrecognized`](Token::Unrecognized).*
    #[from(ignore)]
    Unrecognized(&'src str),
}

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
    /// The keyword "array".
    Array,
    /// The keyword "interrupts".
    Interrupts,
    /// The keyword "reserved".
    Reserved,
    /// The keyword "requires".
    Requires,
    /// The keyword "read".
    Read,
    /// The keyword "wrote".
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
    /// The keyword "extends".
    Extends,
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
    /// The operator "+".
    Plus,
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
}

/// A punctuation token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Punctuation {
    /// The punctuation "#".
    Hash,
    /// The punctuation ",".
    Comma,
    /// The punctuation "(".
    LParen,
    /// The punctuation ")".
    RParen,
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
            Self::Literal(n) => write!(f, "{n}"),
            Self::Ident(s) => write!(f, "{s}"),
            Self::Doc(..) => write!(f, "doc comment"),
            Self::Keyword(Device) => write!(f, "device"),
            Self::Keyword(Import) => write!(f, "import"),
            Self::Keyword(Peripheral) => write!(f, "peripheral"),
            Self::Keyword(Register) => write!(f, "register"),
            Self::Keyword(Field) => write!(f, "field"),
            Self::Keyword(Schema) => write!(f, "schema"),
            Self::Keyword(Variant) => write!(f, "variant"),
            Self::Keyword(Group) => write!(f, "group"),
            Self::Keyword(Array) => write!(f, "array"),
            Self::Keyword(Interrupts) => write!(f, "interrupts"),
            Self::Keyword(Reserved) => write!(f, "reserved"),
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
            Self::Keyword(Extends) => write!(f, "extends"),
            Self::Keyword(Assumes) => write!(f, "assumes"),
            Self::Keyword(Reset) => write!(f, "reset"),
            Self::Operator(Pipe) => write!(f, "|"),
            Self::Operator(Tilde) => write!(f, "~"),
            Self::Operator(At) => write!(f, "@"),
            Self::Operator(Amp) => write!(f, "&"),
            Self::Operator(Plus) => write!(f, "+"),
            Self::Operator(Minus) => write!(f, "-"),
            Self::Operator(Dot) => write!(f, "."),
            Self::Operator(DotDot) => write!(f, ".."),
            Self::Operator(DotDotEq) => write!(f, "..="),
            Self::Operator(Ellipsis) => write!(f, "..."),
            Self::Punctuation(Hash) => write!(f, "#"),
            Self::Punctuation(Comma) => write!(f, ","),
            Self::Punctuation(LParen) => write!(f, "("),
            Self::Punctuation(RParen) => write!(f, ")"),
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
        /// token![Lparen] // expands to: `Token::from(token::Lparen)`
        /// ```
        ///
        /// For non-atomic tokens like [identifiers](Token::Ident) and [literals](Token::Literal), the [`From`]
        /// implementations may be used.
        ///
        /// *Note: Some punctuation (parenthesis, braces, and brackets) must be referred to by name, i.e. "LParen" for
        /// `(`.*
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
        (+) => Plus,
        (-) => Minus,
        (.) => Dot,
        (..) => DotDot,
        (..=) => DotDotEq,
        (...) => Ellipsis,
    }

    punctuation {
        (#) => Hash,
        (,) => Comma,
    }
}

/// A refinement of [`token!`](token) specifically for keywords.
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
