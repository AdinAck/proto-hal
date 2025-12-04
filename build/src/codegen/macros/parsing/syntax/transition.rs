use quote::ToTokens;
use syn::{Expr, ExprLit, Lit, LitInt, parse::Parse};

/// A transition is the last component of an entry, delineating the transition to be performed in the gate.
///
/// ```ignore
/// foo::bar(&mut baz) => _,
/// //                 ^^^^
/// //                 transition
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum Transition {
    /// The transition input is an expression.
    Expr(Expr),
    /// The transition input is a literal integer.
    Lit(LitInt),
}

impl Parse for Transition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(match input.parse()? {
            Expr::Lit(ExprLit {
                lit: Lit::Int(lit), ..
            }) => Self::Lit(lit),
            other => Self::Expr(other),
        })
    }
}

impl ToTokens for Transition {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Expr(expr) => expr.to_tokens(tokens),
            Self::Lit(lit_int) => lit_int.to_tokens(tokens),
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::{ToTokens as _, quote};
    use syn::parse_quote;

    use super::Transition;

    #[test]
    fn lit() {
        let tokens = quote! { 0xdeadbeef };
        let transition: Transition = parse_quote! { #tokens };

        assert!(
            matches!(transition, Transition::Lit(lit) if lit.base10_parse::<u32>().is_ok_and(|num| num == 0xdeadbeef))
        )
    }

    #[test]
    fn expr() {
        let tokens = quote! { foo };
        let transition: Transition = parse_quote! { #tokens };

        assert!(
            matches!(transition, Transition::Expr(expr) if expr.to_token_stream().to_string() == tokens.to_string())
        )
    }
}
