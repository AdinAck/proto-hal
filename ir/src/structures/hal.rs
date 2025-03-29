use std::collections::HashMap;

use quote::{quote, ToTokens};
use serde::{Deserialize, Serialize};

use crate::utils::diagnostic::{Context, Diagnostics};

use super::{peripheral::Peripheral, Ident};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hal {
    pub peripherals: HashMap<String, Peripheral>,
}

impl Hal {
    pub fn new(peripherals: impl IntoIterator<Item = Peripheral>) -> Self {
        Self {
            peripherals: HashMap::from_iter(
                peripherals
                    .into_iter()
                    .map(|peripheral| (peripheral.ident.clone(), peripheral)),
            ),
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

        for peripheral in self.peripherals.values() {
            diagnostics.extend(peripheral.validate(&Context::new()));

            // for entitlement in &peripheral.entitlements {
            //     let p = self.peripherals.get(entitlement.)
            // }
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
        let peripheral_bodies = self
            .peripherals
            .values()
            .map(|peripheral| peripheral.to_token_stream());

        tokens.extend(quote! {
            #(
                #peripheral_bodies
            )*
        });
    }
}
