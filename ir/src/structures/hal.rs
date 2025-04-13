use std::collections::HashMap;

use quote::{quote, ToTokens};

use crate::utils::diagnostic::{Context, Diagnostic, Diagnostics};

use super::{peripheral::Peripheral, Ident};

#[derive(Debug, Clone)]
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

    pub fn render(&self) -> String {
        prettyplease::unparse(
            &syn::parse_file(self.to_token_stream().to_string().as_str()).unwrap(),
        )
    }
}

impl Hal {
    pub fn validate(&self) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = Context::new();

        let mut sorted_peripherals = self.peripherals.values().collect::<Vec<_>>();
        sorted_peripherals.sort_by(|lhs, rhs| lhs.base_addr.cmp(&rhs.base_addr));

        for window in sorted_peripherals.windows(2) {
            let lhs = window[0];
            let rhs = window[1];

            if lhs.base_addr + lhs.width() > rhs.base_addr {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "peripherals [{}] and [{}] overlap.",
                        lhs.ident, rhs.ident
                    ))
                    .with_context(new_context.clone()),
                );
            }
        }

        for peripheral in self.peripherals.values() {
            diagnostics.extend(peripheral.validate(&Context::new()));

            // TODO: validate entitlements
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
