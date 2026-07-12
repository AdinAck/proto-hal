//! The syntax of phm, the proto-hal modeling language.
//!
//! A `.phm` model description describes a device: its peripherals, registers, fields,
//! and the stateful semantics — schemas, entitlements — that `proto-hal`
//! enforces in the type system.
//!
//! This crate is solely the language's *front*: the [lexer], the [parser],
//! and the [syntax tree](ast) they produce, along with the raw errors of both
//! stages. Everything beyond syntax — elaboration into a device model,
//! diagnostics rendering, code generation — lives downstream in
//! `proto-hal-build`, which consumes this crate.
//!
//! # Stages
//!
//! 1. [`lexer`] — source text → [`Token`](token::Token)s. Cannot fail:
//!    unrecognized input becomes tokens for the parser to judge in context.
//! 2. [`parser`] — tokens → [`ast::File`]. Each definition kind has its own
//!    grammar; recovery keeps going after errors so all of them are reported.
//!
//! [`parse`] runs both.

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod token;

#[cfg(test)]
mod tests;

/// The rich error type both stages produce, re-exported for consumers.
pub use chumsky::error::Rich;

use chumsky::{input::Input as _, prelude::Parser as _};

use crate::{
    ast::{SourceId, Span},
    token::Token,
};

/// An error from either stage of syntactic analysis.
#[derive(Debug)]
pub enum Error<'src> {
    /// The source couldn't be tokenized. Rare: unknown characters become
    /// [`Token::Unrecognized`] rather than errors, so this is effectively
    /// reserved for pathological input.
    Lex(Rich<'src, char, Span>),
    /// The tokens don't form the grammar.
    Parse(Rich<'src, Token<'src>, Span>),
}

/// Parse source text, collecting all lexical and syntactic errors. `source`
/// identifies the file, and is carried by every span produced.
///
/// Produces a [`File`](ast::File) whenever one can be recovered — even
/// alongside errors — so downstream stages can still operate on the healthy
/// parts of the tree.
pub fn parse<'src>(src: &'src str, source: SourceId) -> (Option<ast::File<'src>>, Vec<Error<'src>>) {
    let (tokens, lex_errors) = lexer::lexer()
        .parse(src.with_context::<Span>(source))
        .into_output_errors();

    let mut errors = lex_errors.into_iter().map(Error::Lex).collect::<Vec<_>>();

    let Some(tokens) = tokens else {
        return (None, errors);
    };

    let eoi = Span {
        start: src.len(),
        end: src.len(),
        context: source,
    };
    let input = tokens.as_slice().map(eoi, |(token, span)| (token, span));

    let (file, parse_errors) = parser::file().parse(input).into_output_errors();

    errors.extend(
        parse_errors
            .into_iter()
            .map(|error| Error::Parse(error.into_owned())),
    );

    (file, errors)
}
