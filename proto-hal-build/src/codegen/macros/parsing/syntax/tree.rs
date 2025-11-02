use syn::{
    Path, braced,
    parse::Parse,
    token::{Brace, Comma},
};

use super::Entry;

/// A syntax tree for gates, containing a [`Path`] and a [`Node`].
#[derive(Debug, PartialEq, Eq)]
pub struct Tree {
    /// The path of the node.
    pub path: Path,
    /// The [`Node`], either a branch or leaf.
    pub node: Node,
}

/// A node of a path tree, which can either be a branch, or a leaf.
///
/// A branch contains a comma separated list of child nodes.
///
/// A leaf contains an [`Entry`].
#[derive(Debug, PartialEq, Eq)]
pub enum Node {
    /// The tree node is a branch with children.
    Branch(Vec<Tree>),
    /// The tree node is a leaf, terminating the branch.
    Leaf(Entry),
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

            Ok(Self {
                path,
                node: Node::Branch(children),
            })
        } else {
            // foo::bar::baz ...

            Ok(Self {
                path,
                node: Node::Leaf(input.parse()?),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse_quote;

    use crate::codegen::macros::parsing::syntax::tree::Node;

    use super::Tree;

    #[test]
    fn leaf() {
        let tokens = quote! { ::foo::bar::baz(&mut foo) => 0xdeadbeef };
        let tree: Tree = parse_quote! { #tokens };

        assert_eq!(tree.path, parse_quote! { ::foo::bar::baz });
        assert!(matches!(tree.node, Node::Leaf (entry)
                if entry == parse_quote! { (&mut foo) => 0xdeadbeef }
        ))
    }

    #[test]
    fn branch() {
        let tokens = quote! { ::foo::bar { baz(&mut foo) => 0xdeadbeef, booz(dead) => beef } };
        let tree: Tree = parse_quote! { #tokens };

        assert_eq!(tree.path, parse_quote! { ::foo::bar });
        assert!(matches!(tree.node, Node::Branch (children)
            if children.first().is_some_and(|child| child == &parse_quote! { baz(&mut foo) => 0xdeadbeef })
            && children.iter().nth(1).is_some_and(|child| child == &parse_quote! { booz(dead) => beef })
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

        assert_eq!(tree.path, parse_quote! { ::foo });
        assert!(matches!(tree.node, Node::Branch(children)
            if children.first().is_some_and(|node| node == &parse_quote! {
                bar {
                    baz(&mut foo) => 0xdeadbeef,
                    booz(dead) => beef
                }
            })
            && children.iter().nth(1).is_some_and(|node| node == &parse_quote! { bum })
        ))
    }
}
