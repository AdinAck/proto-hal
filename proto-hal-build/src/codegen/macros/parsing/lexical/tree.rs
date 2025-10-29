use syn::{
    Path, braced,
    parse::Parse,
    token::{Brace, Comma},
};

use super::Entry;

/// A node of a path tree, which can either be a branch, or a leaf.
///
/// A branch contains a comma separated list of child nodes.
///
/// A leaf contains an [`Entry`].
#[derive(Debug, PartialEq, Eq)]
pub enum Tree {
    Branch { path: Path, children: Vec<Tree> },
    Leaf { path: Path, entry: Entry },
}

impl Parse for Tree {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;

        if input.peek(Brace) {
            // foo::bar {
            //  baz(...)
            // }

            let block;
            braced!(block in input);

            let children = block
                .parse_terminated(Parse::parse, Comma)?
                .into_iter()
                .collect();

            Ok(Self::Branch { path, children })
        } else {
            // foo::bar::baz ...

            Ok(Self::Leaf {
                path: path,
                entry: input.parse()?,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse_quote;

    use super::Tree;

    #[test]
    fn leaf() {
        let tokens = quote! { ::foo::bar::baz(&mut foo) => 0xdeadbeef };
        let tree: Tree = parse_quote! { #tokens };

        assert!(matches!(tree, Tree::Leaf { path, entry }
                if path == parse_quote! { ::foo::bar::baz }
                && entry == parse_quote! { (&mut foo) => 0xdeadbeef }
        ))
    }

    #[test]
    fn branch() {
        let tokens = quote! { ::foo::bar { baz(&mut foo) => 0xdeadbeef, booz(dead) => beef } };
        let tree: Tree = parse_quote! { #tokens };

        assert!(matches!(tree, Tree::Branch { path, children }
            if path == parse_quote! { ::foo::bar }
            && children.first().is_some_and(|node| node == &parse_quote! { baz(&mut foo) => 0xdeadbeef })
            && children.iter().nth(1).is_some_and(|node| node == &parse_quote! { booz(dead) => beef })
        ))
    }

    #[test]
    fn nested_branch() {
        let tokens = quote! {
            ::foo {
                bar {
                    baz(&mut foo) => 0xdeadbeef,
                    booz(dead) => beef
                },
                bum,
            }
        };
        let tree: Tree = parse_quote! { #tokens };

        assert!(matches!(tree, Tree::Branch { path, children }
            if path == parse_quote! { ::foo }
            && children.first().is_some_and(|node| node == &parse_quote! {
                bar {
                    baz(&mut foo) => 0xdeadbeef,
                    booz(dead) => beef
                }
            })
            && children.iter().nth(1).is_some_and(|node| node == &parse_quote! { bum })
        ))
    }
}
