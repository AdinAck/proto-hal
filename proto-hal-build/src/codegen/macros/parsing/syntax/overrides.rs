use syn::{Expr, Ident, parenthesized, parse::Parse, token::Comma};

/// An override provides a means for altering the gate behavior, like changing the base address of a peripheral for
/// usage in a test environment, or providing an external critical section handle to forgo acquiring a new on within
/// the gate.
#[derive(Debug, PartialEq, Eq)]
pub enum Override {
    /// Override the base address of a peripheral.
    BaseAddress(Ident, Expr),
    /// Pass an external critical section handle.
    CriticalSection(Expr),
    /// The invoked override is unknown.
    Unknown(Ident),
}

impl Parse for Override {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<Ident>()?;

        let block;
        parenthesized!(block in input);

        Ok(match ident.to_string().as_str() {
            "base_addr" => {
                let peripheral_ident = block.parse()?;
                block.parse::<Comma>()?;
                let addr = block.parse::<Expr>()?;

                Self::BaseAddress(peripheral_ident, addr)
            }
            "critical_section" => Self::CriticalSection(block.parse()?),
            _unknown => {
                // pop unused tokens
                block.parse_terminated(Expr::parse, Comma)?;

                Self::Unknown(ident)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::Override;

    #[test]
    fn base_addr() {
        let override_: Override = parse_quote! { base_addr(foo, 0) };

        assert!(
            matches!(override_, Override::BaseAddress(ident, expr) if ident == "foo" && expr == parse_quote!(0))
        )
    }

    #[test]
    fn critical_section() {
        let override_: Override = parse_quote! { critical_section(cs) };

        assert!(matches!(override_, Override::CriticalSection(expr) if expr == parse_quote!(cs)))
    }

    #[test]
    fn unknown() {
        let override_: Override = parse_quote! { whoopsies() };

        assert!(matches!(override_, Override::Unknown(ident) if ident == "whoopsies"))
    }
}
