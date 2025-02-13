use std::collections::HashMap;

use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Serialize};

use super::peripheral::Peripheral;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hal {
    pub peripherals: HashMap<String, Peripheral>,
}

impl Hal {
    pub fn empty() -> Self {
        Self {
            peripherals: HashMap::new(),
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

impl ToTokens for Hal {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let peripheral_idents = self
            .peripherals
            .values()
            .map(|peripheral| format_ident!("{}", peripheral.ident));

        let peripheral_bodies = self
            .peripherals
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
