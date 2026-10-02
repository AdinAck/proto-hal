//! Parsing of [values](crate::ast::Value).

use chumsky::prelude::*;

use super::{Error, Input, ident, literal};
use crate::{ast::Value, token::token};

/// A [`Value`].
///
/// ```text
/// value ::= LITERAL ":" LITERAL | LITERAL | IDENT
/// ```
pub(super) fn value<'src>() -> impl Parser<'src, Input<'src>, Value<'src>, Error<'src>> + Clone {
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
        token::{Ident, Literal},
    };

    #[test]
    fn literal() {
        assert_eq!(
            parse(value(), "0x14").unwrap(),
            Value::Literal(Literal(0x14))
        );
    }

    #[test]
    fn ident() {
        assert_eq!(parse(value(), "foo").unwrap(), Value::Ident(Ident("foo")));
    }

    #[test]
    fn domain() {
        assert_eq!(
            parse(value(), "15:0").unwrap(),
            Value::Domain {
                msb: Literal(15),
                lsb: Literal(0),
            },
        );
    }

    #[test]
    fn reject() {
        for s in ["", "foo:bar", "15:", ":0", "register", "...", "[0]"] {
            assert!(
                parse(value(), s).is_err(),
                "'{s}' was parsed as a value when it shouldn't be",
            );
        }
    }
}
