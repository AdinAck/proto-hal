//! Lexical analysis: source text → [`Token`]s.
//!
//! Lexing ahead of parsing keeps the grammar free of character-level concerns:
//! keywords are atomic, numbers are parsed (with radix prefixes) in exactly one
//! place, plain comments vanish here, and doc comments survive as tokens so the
//! parser can attach them to items.

use std::fmt;

use chumsky::prelude::*;

use crate::ast::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token<'src> {
    Num(u32),
    Ident(&'src str),
    /// `/// ...`
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

pub fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<(Token<'src>, Span)>, extra::Err<Rich<'src, char, Span>>> {
    // digits of the given radix, potentially with `_` separators
    let prefixed_digits = |radix: u32| {
        any()
            .filter(move |c: &char| c.is_digit(radix) || *c == '_')
            .repeated()
            .at_least(1)
    };

    // as above, where the first character must be a digit — so identifiers
    // like `_foo` are not mistaken for numbers
    let digits = |radix: u32| {
        any().filter(move |c: &char| c.is_digit(radix)).then(
            any()
                .filter(move |c: &char| c.is_digit(radix) || *c == '_')
                .repeated(),
        )
    };

    // a number that fails to parse (empty separators, overflow) is lexed as
    // a single `Unrecognized` token so the parser reports it in context
    let num = choice((
        just("0x").then(prefixed_digits(16)).ignored(),
        just("0b").then(prefixed_digits(2)).ignored(),
        just("0o").then(prefixed_digits(8)).ignored(),
        digits(10).ignored(),
    ))
    .to_slice()
    .map(|s: &str| {
        let (digits, radix) = match s.split_at_checked(2) {
            Some(("0x", digits)) => (digits, 16),
            Some(("0b", digits)) => (digits, 2),
            Some(("0o", digits)) => (digits, 8),
            _ => (s, 10),
        };

        u32::from_str_radix(&digits.replace('_', ""), radix)
            .map(Token::Num)
            .unwrap_or(Token::Unrecognized(s))
    })
    .labelled("number");

    let keyword = text::ident()
        .map(|s: &str| match s {
            "device" => Token::Device,
            "import" => Token::Import,
            "peripheral" => Token::Peripheral,
            "register" => Token::Register,
            "field" => Token::Field,
            "schema" => Token::Schema,
            "variant" => Token::Variant,
            "group" => Token::Group,
            "array" => Token::Array,
            "interrupts" => Token::Interrupts,
            "reserved" => Token::Reserved,
            "requires" => Token::Requires,
            "read" => Token::Read,
            "write" => Token::Write,
            "store" => Token::Store,
            "volatile" => Token::Volatile,
            "hardware" => Token::Hardware,
            "leaky" => Token::Leaky,
            "inert" => Token::Inert,
            "as" => Token::As,
            "extends" => Token::Extends,
            "assumes" => Token::Assumes,
            "reset" => Token::Reset,
            _ => Token::Ident(s),
        })
        .labelled("keyword");

    let doc = just("///")
        .ignore_then(any().and_is(just('\n').not()).repeated().to_slice())
        .map(|s: &str| Token::Doc(s.trim()))
        .labelled("doc comment");

    let operator = choice((
        just("...").to(Token::Ellipsis),
        just("..=").to(Token::DotDotEq),
        just("..").to(Token::DotDot),
        just('.').to(Token::Dot),
        just('|').to(Token::Pipe),
        just('~').to(Token::Tilde),
        just('@').to(Token::At),
        just('&').to(Token::Amp),
        just('+').to(Token::Plus),
        just('-').to(Token::Minus),
    ))
    .labelled("operator");

    let punct = choice((
        just('#').to(Token::Hash),
        just(',').to(Token::Comma),
        just('(').to(Token::LParen),
        just(')').to(Token::RParen),
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('[').to(Token::LBracket),
        just(']').to(Token::RBracket),
    ))
    .labelled("punctuation");

    // any character no other token begins with — lexed successfully so the
    // *parser* reports it, with expectations appropriate to its context.
    // digits are excluded so that invalid numbers remain (specific) lexical
    // errors rather than falling through to garbage tokens
    let unrecognized = any()
        .filter(|c: &char| !c.is_ascii_digit())
        .to_slice()
        .map(Token::Unrecognized);

    let token = choice((doc, num, keyword, operator, punct, unrecognized));

    // `//` but not `///`, which is a doc comment
    let comment = just("//")
        .then(just('/').not())
        .then(any().and_is(just('\n').not()).repeated())
        .labelled("comment")
        .padded();

    token
        .map_with(|token, e| (token, e.span()))
        .padded_by(comment.repeated())
        .padded()
        // the lexer can no longer fail on any input, since unrecognized
        // characters and invalid numbers become tokens — this is a safety net
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect()
}
