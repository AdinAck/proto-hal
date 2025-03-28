use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Serialize};

use crate::utils::diagnostic::{Context, Diagnostics};

use super::{peripheral::Peripheral, Collection, Ident};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hal {
    pub peripherals: Collection<Peripheral>,
}

impl Hal {
    pub fn empty() -> Self {
        Self {
            peripherals: Collection::new(),
        }
    }
}

impl PartialOrd for Hal {
    fn partial_cmp(&self, #[allow(unused)] other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

impl Ord for Hal {
    fn cmp(&self, #[allow(unused)] other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}

impl Hal {
    pub fn validate(&self) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();

        for peripheral in self.peripherals.map.values() {
            diagnostics.extend(peripheral.validate(&Context::new()));
        }

        diagnostics
    }
}

impl Ident for Hal {
    fn ident(&self) -> &str {
        "hal"
    }
}

impl ToTokens for Hal {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let peripheral_idents = self
            .peripherals
            .map
            .values()
            .map(|peripheral| format_ident!("{}", peripheral.ident));

        let peripheral_bodies = self
            .peripherals
            .map
            .values()
            .map(|peripheral| peripheral.to_token_stream());

        tokens.extend(quote! {
            #(
                #peripheral_bodies
            )*

            pub struct Reset {
                #(
                    #peripheral_idents: #peripheral_idents::Reset,
                )*
            }
        });
    }
}
