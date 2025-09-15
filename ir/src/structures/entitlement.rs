use std::{collections::HashSet, fmt::Display};

use proc_macro2::Span;
use syn::{Ident, Path, parse_quote};
use ters::ters;

use crate::structures::{field::Dimensionality, hal::Hal};

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
            peripheral: Ident::new(path.next().unwrap_or("unknown"), Span::call_site()),
            register: Ident::new(path.next().unwrap_or("unknown"), Span::call_site()),
            field: Ident::new(path.next().unwrap_or("unknown"), Span::call_site()),
            variant: Ident::new(path.next().unwrap_or("unknown"), Span::call_site()),
        }
    }

    pub fn render(&self, hal: &Hal) -> Path {
        let (p, r, f, v) = hal.look_up(self).expect("entitlements must exist");

        match &f.dimensionality {
            Dimensionality::Single => {
                let peripheral = self.peripheral();
                let register = self.register();
                let field = self.field();
                let variant = self.variant();
                parse_quote! {
                    crate::#peripheral::#register::#field::#variant
                }
            }
            Dimensionality::Array { idents } => {
                let p_ident = p.module_name();
                let r_ident = r.module_name();
                let f_ident = f.module_name();
                let v_ident = v.type_name();
                let index = idents[self.field()];

                parse_quote! {
                    crate::#p_ident::#r_ident::#f_ident::#v_ident::<#index>
                }
            }
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

pub type Entitlements = HashSet<Entitlement>;
