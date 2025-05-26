use proc_macro2::Span;
use syn::{parse_quote, Ident, Path};
use ters::ters;

#[ters]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Entitlement {
    #[get]
    peripheral: String,
    #[get]
    register: String,
    #[get]
    field: String,
    #[get]
    variant: String,
}

impl Entitlement {
    pub fn to(path: impl AsRef<str>) -> Self {
        let mut path = path.as_ref().split("::");

        Self {
            peripheral: path.next().unwrap_or("").to_string(),
            register: path.next().unwrap_or("").to_string(),
            field: path.next().unwrap_or("").to_string(),
            variant: path.next().unwrap_or("").to_string(),
        }
    }

    pub fn render(&self) -> Path {
        let peripheral = Ident::new(&self.peripheral(), Span::call_site());
        let register = Ident::new(&self.register(), Span::call_site());
        let field = Ident::new(&self.field(), Span::call_site());
        let variant = Ident::new(&self.variant(), Span::call_site());
        parse_quote! {
            crate::#peripheral::#register::#field::#variant
        }
    }
}
