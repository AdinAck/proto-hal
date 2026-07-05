//! The atoms of the language.
//!
//! Tokens are produced by the [lexer](crate::lexer) and consumed by the
//! [parser](crate::parser). They borrow from the source text, so identifiers,
//! doc comments, and [unrecognized regions](Token::Unrecognized) are zero-copy.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token<'src> {
    /// A number literal of any radix, e.g. `42`, `0xc4`, `0b1101`, `0o777`.
    Num(u32),
    /// A name, e.g. `gpioa`.
    Ident(&'src str),
    /// A doc comment: `/// ...`. Doc comments are tokens — unlike plain
    /// comments — so the parser can attach them to items.
    Doc(&'src str),

    // keywords
    Device,
    Import,
    Peripheral,
    Register,
    Field,
    Schema,
    Variant,
    Group,
    Array,
    Interrupts,
    Reserved,
    Requires,
    Read,
    Write,
    Store,
    Volatile,
    Hardware,
    Leaky,
    Inert,
    As,
    By,
    Extends,
    Assumes,
    Reset,

    // punctuation
    Hash,
    Pipe,
    Tilde,
    At,
    Comma,
    Amp,
    Plus,
    Minus,
    Dot,
    DotDot,
    DotDotEq,
    Ellipsis,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    /// A region of source no token recognizes — including number literals that
    /// fail to parse (bad separators, overflow). Lexed *successfully* so the
    /// parser reports it with expectations appropriate to its context.
    Unrecognized(&'src str),
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Num(n) => write!(f, "{n}"),
            Self::Ident(s) => write!(f, "{s}"),
            Self::Doc(..) => write!(f, "doc comment"),
            Self::Device => write!(f, "device"),
            Self::Import => write!(f, "import"),
            Self::Peripheral => write!(f, "peripheral"),
            Self::Register => write!(f, "register"),
            Self::Field => write!(f, "field"),
            Self::Schema => write!(f, "schema"),
            Self::Variant => write!(f, "variant"),
            Self::Group => write!(f, "group"),
            Self::Array => write!(f, "array"),
            Self::Interrupts => write!(f, "interrupts"),
            Self::Reserved => write!(f, "reserved"),
            Self::Requires => write!(f, "requires"),
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Store => write!(f, "store"),
            Self::Volatile => write!(f, "volatile"),
            Self::Hardware => write!(f, "hardware"),
            Self::Leaky => write!(f, "leaky"),
            Self::Inert => write!(f, "inert"),
            Self::As => write!(f, "as"),
            Self::By => write!(f, "by"),
            Self::Extends => write!(f, "extends"),
            Self::Assumes => write!(f, "assumes"),
            Self::Reset => write!(f, "reset"),
            Self::Hash => write!(f, "#"),
            Self::Pipe => write!(f, "|"),
            Self::Tilde => write!(f, "~"),
            Self::At => write!(f, "@"),
            Self::Comma => write!(f, ","),
            Self::Amp => write!(f, "&"),
            Self::Plus => write!(f, "+"),
            Self::Minus => write!(f, "-"),
            Self::Dot => write!(f, "."),
            Self::DotDot => write!(f, ".."),
            Self::DotDotEq => write!(f, "..="),
            Self::Ellipsis => write!(f, "..."),
            Self::LParen => write!(f, "("),
            Self::RParen => write!(f, ")"),
            Self::LBrace => write!(f, "{{"),
            Self::RBrace => write!(f, "}}"),
            Self::LBracket => write!(f, "["),
            Self::RBracket => write!(f, "]"),
            Self::Unrecognized(s) => write!(f, "{s}"),
        }
    }
}
