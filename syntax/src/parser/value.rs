//! Parsing of [values](crate::ast::Value).

use chumsky::prelude::*;

use super::{Error, Input, ident, literal};
use crate::{ast::Value, token::token};

/// A [`Value`].
///
/// ```text
/// value ::= LITERAL ":" LITERAL | LITERAL | IDENT
/// ```
pub(super) fn value<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Value<'src>, Error<'src>> + Clone {
    choice((
        literal()
            .then_ignore(just(token![:]))
            .then(literal())
            .map(|(msb, lsb)| Value::Domain { msb, lsb }),
        literal().map(Value::Literal),
        ident().map(Value::Ident),
    ))
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::Value,
        parser::{tests::parse, value::value},
        token::{Ident, bare_literal},
    };

    #[test]
    fn literal() {
        similar_asserts::assert_eq!(
            expected: Value::Literal(bare_literal![0x14]),
            found: parse!(value(), "0x14").unwrap(),
            "expected `0x14` to be parsed as a literal value",
        );
    }

    #[test]
    fn ident() {
        similar_asserts::assert_eq!(
            expected: Value::Ident(Ident("foo")),
            found: parse!(value(), "foo").unwrap(),
            "expected `foo` to be parsed as an identifier value",
        );
    }

    #[test]
    fn domain() {
        similar_asserts::assert_eq!(
            expected: Value::Domain {
                msb: bare_literal![15],
                lsb: bare_literal![0],
            },
            found: parse!(value(), "15:0").unwrap(),
            "expected `15:0` to be parsed as a domain value",
        );
    }

    #[test]
    fn reject() {
        for s in ["", "foo:bar", "15:", ":0", "register", "...", "[0]"] {
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(value(), s).ok(),
                "expected parsing `{s}` as a value to fail",
            );
        }
    }
}
