use ir::structures::{
    field::{Field, Numericity},
    peripheral::Peripheral,
    register::Register,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

use crate::codegen::macros::gates::utils::unique_register_ident;

pub fn read_value_expr(
    register_path: &Path,
    field_ident: &Ident,
    peripheral: &Peripheral,
    register: &Register,
    field: &Field,
) -> Option<TokenStream> {
    let reg = unique_register_ident(peripheral, register);
    let mask = u32::MAX >> (32 - field.width);
    let shift = if field.offset == 0 {
        None
    } else {
        let offset = &field.offset;
        Some(quote! { >> #offset })
    };

    let value = quote! {
        (#reg #shift) & #mask
    };

    Some(match field.access.get_read()?.numericity {
        Numericity::Numeric => value,
        Numericity::Enumerated { .. } => quote! {
            unsafe { #register_path::#field_ident::read::Variant::from_bits(#value) }
        },
    })
}
