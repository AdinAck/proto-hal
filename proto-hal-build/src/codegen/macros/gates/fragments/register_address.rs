use std::collections::HashMap;

use model::structures::{peripheral::Peripheral, register::Register};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Expr, Ident};

/// The MMIO mapped address of the register.
pub fn register_address(
    peripheral: &Peripheral,
    register: &Register,
    overridden_base_addrs: &HashMap<Ident, Expr>,
) -> TokenStream {
    if let Some(expr) = overridden_base_addrs.get(&peripheral.module_name()) {
        let offset = register.offset as usize;
        if offset == 0 {
            quote! { #expr }
        } else {
            quote! { (#expr + #offset) }
        }
    } else {
        (peripheral.base_addr + register.offset).to_token_stream()
    }
}
