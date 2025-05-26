use std::fmt::Display;

use proc_macro2::Span;
use syn::{parse_quote, Ident, Path};
use ters::ters;

#[ters]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Entitlement {
    #[get]
    peripheral: Ident,
    #[get]
    register: Ident,
    #[get]
    field: Ident,
    #[get]
    variant: Ident,
}

impl Entitlement {
    pub fn to(path: impl AsRef<str>) -> Self {
        let mut path = path.as_ref().split("::");

        Self {
            peripheral: Ident::new(path.next().unwrap_or(""), Span::call_site()),
            register: Ident::new(path.next().unwrap_or(""), Span::call_site()),
            field: Ident::new(path.next().unwrap_or(""), Span::call_site()),
            variant: Ident::new(path.next().unwrap_or(""), Span::call_site()),
        }
    }

    pub fn render(&self) -> Path {
        let peripheral = self.peripheral();
        let register = self.register();
        let field = self.field();
        let variant = self.variant();
        parse_quote! {
            crate::#peripheral::#register::#field::#variant
        }
    }
}

impl Display for Entitlement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}::{}::{}::{}",
            self.peripheral(),
            self.register(),
            self.field(),
            self.variant()
        )
    }
}
