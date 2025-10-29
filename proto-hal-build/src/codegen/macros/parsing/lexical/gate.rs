use syn::{Token, parse::Parse, token::Comma};

use super::{Override, Tree};

/// The input provided to a gate macro.
///
/// A gate contains:
/// 1. Path trees
/// 1. Any overrides
///
/// This is the top-level lexical parsing structure for gate macros, encapsulating all provided tokens.
///
/// ```ignore
/// ::foo::bar {
///     baz::bum { bing(&mut a) => 0 },
///     bong(&b),
/// },
/// ::sna::fu(c),
/// @deadbeef,
/// ```
#[derive(Debug)]
pub struct Gate {
    pub trees: Vec<Tree>,
    pub overrides: Vec<Override>,
}

impl Parse for Gate {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut trees = Vec::new();
        let mut overrides = Vec::new();

        enum Kind {
            Tree(Tree),
            Override(Override),
        }

        let punctuated = input.parse_terminated(
            |buf| {
                if buf.peek(Token![@]) {
                    buf.parse::<Token![@]>()?;
                    Ok(Kind::Override(buf.parse()?))
                } else {
                    let tree = buf.parse::<Tree>()?;
                    Ok(Kind::Tree(tree))
                }
            },
            Comma,
        )?;

        for item in punctuated {
            match item {
                Kind::Tree(tree) => trees.push(tree),
                Kind::Override(override_) => overrides.push(override_),
            }
        }

        Ok(Self { trees, overrides })
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse_quote;

    use super::{
        super::{Entry, Transition, Tree},
        Gate,
    };

    #[test]
    fn empty() {
        let gate: Gate = parse_quote! {};

        assert!(gate.trees.is_empty());
        assert!(gate.overrides.is_empty())
    }

    #[test]
    fn overrides() {
        let tokens = quote! { @base_addr(foo, 0), @critical_section(cs) };
        let gate: Gate = parse_quote! { #tokens };

        assert!(gate.trees.is_empty());
        assert_eq!(gate.overrides[0], parse_quote! { base_addr(foo, 0) });
        assert_eq!(gate.overrides[1], parse_quote! { critical_section(cs) });
    }

    #[test]
    fn nodes() {
        let tokens = quote! {
            foo,
            ::foo,
            foo {
                bar,
            },
            foo::bar {
                baz,
            },
            ::foo::bar::baz,
            foo::bar {
                baz => _,
            },
            ::foo::bar::baz => _,
            foo(_),
            foo {
                bar(_) => _,
            }
        };
        let gate: Gate = parse_quote! { #tokens };

        let expected_trees = [
            Tree::Leaf {
                path: parse_quote!(foo),
                entry: Entry {
                    binding: None,
                    transition: None,
                },
            },
            Tree::Leaf {
                path: parse_quote!(::foo),
                entry: Entry {
                    binding: None,
                    transition: None,
                },
            },
            Tree::Branch {
                path: parse_quote!(foo),
                children: vec![Tree::Leaf {
                    path: parse_quote!(bar),
                    entry: Entry {
                        binding: None,
                        transition: None,
                    },
                }],
            },
            Tree::Branch {
                path: parse_quote!(foo::bar),
                children: vec![Tree::Leaf {
                    path: parse_quote!(baz),
                    entry: Entry {
                        binding: None,
                        transition: None,
                    },
                }],
            },
            Tree::Leaf {
                path: parse_quote!(::foo::bar::baz),
                entry: Entry {
                    binding: None,
                    transition: None,
                },
            },
            Tree::Branch {
                path: parse_quote!(foo::bar),
                children: vec![Tree::Leaf {
                    path: parse_quote!(baz),
                    entry: Entry {
                        binding: None,
                        transition: Some(Transition::Expr(parse_quote!(_))),
                    },
                }],
            },
            Tree::Leaf {
                path: parse_quote!(::foo::bar::baz),
                entry: Entry {
                    binding: None,
                    transition: Some(Transition::Expr(parse_quote!(_))),
                },
            },
            Tree::Leaf {
                path: parse_quote!(foo),
                entry: Entry {
                    binding: Some(parse_quote!(_)),
                    transition: None,
                },
            },
            Tree::Branch {
                path: parse_quote!(foo),
                children: vec![Tree::Leaf {
                    path: parse_quote!(bar),
                    entry: Entry {
                        binding: Some(parse_quote!(_)),
                        transition: Some(Transition::Expr(parse_quote!(_))),
                    },
                }],
            },
        ];

        for (tree, expected) in gate.trees.iter().zip(expected_trees.iter()) {
            assert_eq!(tree, expected);
        }
    }

    #[test]
    fn mix() {
        let tokens = quote! {
            foo {
                bar(_) => _,
            },
            @base_addr(foo, 0),
        };
        let gate: Gate = parse_quote! { #tokens };

        assert_eq!(
            gate.trees[0],
            Tree::Branch {
                path: parse_quote!(foo),
                children: vec![Tree::Leaf {
                    path: parse_quote!(bar),
                    entry: Entry {
                        binding: Some(parse_quote!(_)),
                        transition: Some(Transition::Expr(parse_quote!(_))),
                    },
                }],
            },
        );
        assert_eq!(gate.overrides[0], parse_quote! { base_addr(foo, 0) });
    }
}
