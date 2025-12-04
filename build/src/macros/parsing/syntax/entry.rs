use syn::{
    parenthesized,
    parse::Parse,
    token::{FatArrow, Paren},
};

use super::{Binding, Transition};

/// An entry resides after the leaf of a path, optionally providing the binding and/or transition.
///
/// ```ignore
/// foo::bar(&mut baz) => _,
/// //      ^^^^^^^^^^^^^^^
/// //      entry
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    /// The binding component of the entry (see [`Binding`]).
    pub binding: Option<Binding>,
    /// The transition component of the entry (see [`Transition`]).
    pub transition: Option<Transition>,
}

impl Parse for Entry {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let binding = if input.peek(Paren) {
            let block;
            parenthesized!(block in input);
            Some(block.parse()?)
        } else {
            None
        };

        let transition = if input.peek(FatArrow) {
            input.parse::<FatArrow>()?;
            Some(input.parse()?)
        } else {
            None
        };

        Ok(Self {
            binding,
            transition,
        })
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse_quote;

    use super::{Binding, Entry, Transition};

    fn assert_binding_present(binding: Option<Binding>) {
        assert!(binding.is_some_and(|binding| binding.is_dynamic()));
    }

    fn assert_transition_present(transition: Option<Transition>) {
        assert!(
            transition.is_some_and(|transition| matches!(transition, Transition::Lit(lit)
                        if lit.base10_parse::<u32>().is_ok_and(|num| num == 0xdeadbeef)))
        );
    }

    #[test]
    fn empty() {
        let entry: Entry = parse_quote! {};

        assert!(entry.binding.is_none());
        assert!(entry.transition.is_none());
    }

    #[test]
    fn binding() {
        let tokens = quote! { (&mut foo) };
        let entry: Entry = parse_quote! { #tokens };

        assert_binding_present(entry.binding);
        assert!(entry.transition.is_none());
    }

    #[test]
    fn transition() {
        let tokens = quote! { => 0xdeadbeef };
        let entry: Entry = parse_quote! { #tokens };

        assert!(entry.binding.is_none());
        assert_transition_present(entry.transition);
    }

    #[test]
    fn exhaustive() {
        let tokens = quote! { (&mut foo) => 0xdeadbeef };
        let entry: Entry = parse_quote! { #tokens };

        assert_binding_present(entry.binding);
        assert_transition_present(entry.transition);
    }
}
