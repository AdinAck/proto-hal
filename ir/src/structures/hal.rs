use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::Ident;

use crate::utils::diagnostic::{Context, Diagnostic, Diagnostics};

use super::peripheral::Peripheral;

#[derive(Debug, Clone)]
pub struct Hal {
    pub peripherals: HashMap<Ident, Peripheral>,
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

    pub fn render(&self) -> syn::Result<String> {
        Ok(prettyplease::unparse(&syn::parse_file(
            self.to_token_stream().to_string().as_str(),
        )?))
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

// codegen
impl Hal {
    fn generate_peripherals<'a>(peripherals: impl Iterator<Item = &'a Peripheral>) -> TokenStream {
        quote! {
            #(
                #peripherals
            )*
        }
    }

    fn generate_peripherals_struct<'a>(
        peripherals: impl Iterator<Item = &'a Peripheral> + Clone,
    ) -> TokenStream {
        let fundamental_peripheral_idents = peripherals
            .clone()
            .filter_map(|peripheral| {
                if peripheral.entitlements.is_empty() {
                    Some(peripheral.module_name())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let conditional_peripheral_idents = peripherals
            .filter_map(|peripheral| {
                if !peripheral.entitlements.is_empty() {
                    Some(peripheral.module_name())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        quote! {
            pub struct Peripherals {
                // fundamental
                #(
                    pub #fundamental_peripheral_idents: #fundamental_peripheral_idents::Reset,
                )*

                // conditional
                #(
                    pub #conditional_peripheral_idents: #conditional_peripheral_idents::Masked,
                )*
            }

            pub unsafe fn peripherals() -> Peripherals {
                #[allow(unsafe_op_in_unsafe_fn)]
                Peripherals {
                    // fundamental
                    #(
                        #fundamental_peripheral_idents: #fundamental_peripheral_idents::Reset::conjure(),
                    )*

                    // conditional
                    #(
                        #conditional_peripheral_idents: #conditional_peripheral_idents::Masked::conjure(),
                    )*
                }
            }
        }
    }
}

impl ToTokens for Hal {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(Self::generate_peripherals(self.peripherals.values()));
        tokens.extend(Self::generate_peripherals_struct(self.peripherals.values()));
    }
}
