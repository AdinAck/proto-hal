use derive_more::{AsRef, Deref};
use syn::{Expr, parse::Parse};

/// A binding is the first entry component, delineating the resource passed to the gate.
///
/// ```ignore
/// foo::bar(&mut baz) => _,
/// //      ^^^^^^^^^^
/// //      binding
/// ```
#[derive(Debug, PartialEq, Eq, AsRef, Deref)]
pub struct Binding(Expr);

impl Binding {
    /// The binding provides a view to the resource.
    ///
    /// ```ignore
    /// &foo
    /// ```
    pub fn is_viewed(&self) -> bool {
        matches!(self.as_ref(), Expr::Reference(r) if r.mutability.is_none())
    }

    /// The binding provides dynamic access to the resource.
    ///
    /// ```ignore
    /// &mut foo
    /// ```
    pub fn is_dynamic(&self) -> bool {
        matches!(self.as_ref(), Expr::Reference(r) if r.mutability.is_some())
    }

    /// The binding moves the resource.
    ///
    /// ```ignore
    /// foo
    /// ```
    pub fn is_moved(&self) -> bool {
        !matches!(self.as_ref(), Expr::Reference(..))
    }

    /// The binding provides dynamic access or moves the resource.
    ///
    /// *See [`is_dynamic`] and [`is_moved`].*
    pub fn is_mutated(&self) -> bool {
        self.is_moved() || self.is_dynamic()
    }
}

impl Parse for Binding {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self(input.parse()?))
    }
}

#[cfg(test)]
mod tests {
    use quote::{ToTokens as _, quote};
    use syn::parse_quote;

    use super::Binding;

    #[test]
    fn moved() {
        let tokens = quote! { foo };
        let moved: Binding = parse_quote! { #tokens };

        assert!(moved.is_moved());
        assert!(moved.is_mutated());
        assert_eq!(moved.to_token_stream().to_string(), tokens.to_string());
    }

    #[test]
    fn viewed() {
        let tokens = quote! { &foo };
        let viewed: Binding = parse_quote! { #tokens };

        assert!(viewed.is_viewed());
        assert!(!viewed.is_mutated());
        assert_eq!(viewed.to_token_stream().to_string(), tokens.to_string());
    }

    #[test]
    fn dynamic() {
        let tokens = quote! { &mut foo };
        let dynamic: Binding = parse_quote! { #tokens };

        assert!(dynamic.is_dynamic());
        assert!(dynamic.is_mutated());
        assert_eq!(dynamic.to_token_stream().to_string(), tokens.to_string());
    }
}
