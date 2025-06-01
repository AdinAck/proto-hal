use std::collections::HashMap;

use colored::Colorize;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::Ident;

use crate::utils::diagnostic::{Context, Diagnostic, Diagnostics};

use super::{entitlement::Entitlement, field::Numericity, peripheral::Peripheral};

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
                diagnostics.insert(
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
        }

        // collect all entitlements
        let mut entitlements = HashMap::<Context, Vec<Entitlement>>::new();

        let context = Context::new();

        for peripheral in self.peripherals.values() {
            let context = context.clone().and(peripheral.module_name().to_string());

            entitlements
                .entry(context.clone())
                .or_default()
                .extend(peripheral.entitlements.clone());

            for register in peripheral.registers.values() {
                let context = context.clone().and(register.module_name().to_string());

                for field in register.fields.values() {
                    let context = context.clone().and(field.module_name().to_string());

                    // TODO: when field superpositioning is implemented:
                    // TODO: entitlements.extend(field.entitlements.clone());

                    let accesses = [field.access.get_read(), field.access.get_write()];

                    for access in accesses.iter().flatten() {
                        entitlements
                            .entry(context.clone())
                            .or_default()
                            .extend(access.entitlements.clone());

                        if let Numericity::Enumerated { variants } = &access.numericity {
                            for variant in variants.values() {
                                let context = context.clone().and(variant.type_name().to_string());

                                entitlements
                                    .entry(context)
                                    .or_default()
                                    .extend(variant.entitlements.clone());
                            }
                        }
                    }
                }
            }
        }

        // traverse the hal tree given the entitlement path and ensure the path exists
        for (context, entitlements) in entitlements {
            for entitlement in entitlements {
                let Some(peripheral) = self.peripherals.get(entitlement.peripheral()) else {
                    diagnostics.insert(
                        Diagnostic::error(format!(
                            "entitlement peripheral [{}] does not exist",
                            entitlement.peripheral().to_string().bold()
                        ))
                        .with_context(context.clone()),
                    );

                    continue;
                };

                let Some(register) = peripheral.registers.get(entitlement.register()) else {
                    diagnostics.insert(
                        Diagnostic::error(format!(
                            "entitlement register [{}] does not exist",
                            entitlement.register().to_string().bold()
                        ))
                        .with_context(context.clone()),
                    );

                    continue;
                };

                let Some(field) = register.fields.get(entitlement.field()) else {
                    diagnostics.insert(
                        Diagnostic::error(format!(
                            "entitlement field [{}] does not exist",
                            entitlement.field().to_string().bold()
                        ))
                        .with_context(context.clone()),
                    );

                    continue;
                };

                let Some(read) = field.access.get_read() else {
                    diagnostics.insert(
                        Diagnostic::error(format!("entitlements path [{}] targets unresolvable field which cannot be entitled to", entitlement.to_string().bold()))
                            .with_context(context.clone()),
                    );

                    continue;
                };

                let Numericity::Enumerated { variants } = &read.numericity else {
                    diagnostics.insert(
                        Diagnostic::error(format!("entitlement path [{}] targets numeric field which cannot be entitled to", entitlement.to_string().bold()))
                            .with_context(context.clone()),
                    );

                    continue;
                };

                let Some(_variant) = variants.get(entitlement.variant()) else {
                    diagnostics.insert(
                        Diagnostic::error(format!(
                            "entitlement variant [{}] does not exist",
                            entitlement.variant().to_string().bold()
                        ))
                        .with_context(context.clone()),
                    );

                    continue;
                };
            }
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
