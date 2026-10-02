//! Parsing of [imports](Import).

use chumsky::prelude::*;

use super::{Error, Input, ident, path::module_path};
use crate::{
    ast::{Import, ImportTree},
    diagnostic::Expected,
    token::{keyword, token},
};

/// An [`Import`].
///
/// ```text
/// import ::= "import" import_tree
/// ```
pub(super) fn import<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Import<'src>, Error<'src>> + Clone {
    just(keyword![Import])
        .ignore_then(import_tree().spanned())
        .map(|tree| Import { tree })
}

/// An [`ImportTree`].
///
/// ```text
/// import_tree ::= module_path ("as" IDENT)?
///               | module_path "::" "{" group_item ("," group_item)* ","? "}"
/// group_item  ::= "self" ("as" IDENT)? | import_tree
/// ```
fn import_tree<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, ImportTree<'src>, Error<'src>> + Clone {
    recursive(|import_tree| {
        // before :: or as
        let path = module_path().spanned().labelled(Expected::ModulePath);
        // as ...
        let alias = just(keyword![As]).ignore_then(ident().spanned()).or_not();

        // self or tree
        let item = choice((
            just(keyword![Self_])
                .ignore_then(alias.clone())
                .map(|alias| ImportTree::Self_ { alias }),
            import_tree,
        ));

        // { ... }
        let group = item
            .spanned()
            .separated_by(just(token![,]))
            .at_least(1)
            .allow_trailing()
            .collect()
            .delimited_by(just(token![LBrace]), just(token![RBrace]));

        choice((
            path.clone()
                .then_ignore(just(token![::]))
                .then(group)
                .map(|(prefix, trees)| ImportTree::Group { prefix, trees }),
            path.then(alias)
                .map(|(path, alias)| ImportTree::Path { path, alias }),
        ))
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        diagnostic::{Expected, Found},
        parser::{
            import::import,
            tests::{expectations, parse},
        },
        token::keyword,
    };

    #[test]
    fn imports() {
        for s in [
            "import foo",
            "import foo::bar",
            "import foo::bar as baz",
            "import foo::{bar, baz}",
            "import foo::bar::{baz}",
            "import foo::{self, bar as baz}",
            "import foo::{self as bar, baz::{qux, corge}}",
        ] {
            similar_asserts::assert_eq!(
                expected: s,
                found: parse!(import(), s).unwrap().to_string(),
                "expected display impl for `{s}` to reproduce the source text",
            );
        }
    }

    #[test]
    fn trailing_comma() {
        let s = "import foo::{bar, baz,}";

        similar_asserts::assert_eq!(
            expected: "import foo::{bar, baz}",
            found: parse!(import(), s).unwrap().to_string(),
            "expected display impl for the import parsed from `{s}` to omit the trailing comma",
        );
    }

    #[test]
    fn expected() {
        for (s, expected) in [
            ("import", vec![Expected::ModulePath]),
            ("import foo as", vec![Expected::Identifier]),
            (
                "import foo::{",
                vec![Expected::Token(keyword![Self_]), Expected::ModulePath],
            ),
        ] {
            similar_asserts::assert_eq!(
                expected: vec![(Found::End, expected.clone())],
                found: expectations(parse!(import(), s).unwrap_err()),
                "expected parsing `{s}` as an import to fail, expecting {expected:?}",
            );
        }
    }

    #[test]
    fn reject() {
        for s in [
            "",
            "import foo::",
            "import foo.bar",
            "import device::foo",
            "import foo[a, b]",
            "import foo as bar::baz",
            "/// Foo.\nimport foo",
            "import self",
            "import foo::{}",
            "import foo::{bar} as baz",
            "import foo::{self::bar}",
        ] {
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(import(), s).ok().map(|import| import.to_string()),
                "expected parsing {s:?} as an import to fail",
            );
        }
    }
}
