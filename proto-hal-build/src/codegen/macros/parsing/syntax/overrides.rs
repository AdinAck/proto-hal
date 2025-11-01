use syn::{Expr, Ident, parenthesized, parse::Parse, token::Comma};

#[derive(Debug, PartialEq, Eq)]
pub enum Override {
    BaseAddress(Ident, Expr),
    CriticalSection(Expr),
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
            _unknown => Self::Unknown(ident),
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
