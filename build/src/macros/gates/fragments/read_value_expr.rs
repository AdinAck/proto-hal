use model::{
    field::{FieldNode, numericity::Numericity},
    peripheral::Peripheral,
    register::Register,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

use crate::macros::gates::utils::unique_register_ident;

pub fn read_value_expr(
    register_path: &Path,
    field_ident: &Ident,
    peripheral: &Peripheral,
    register: &Register,
    field: &FieldNode,
) -> Option<TokenStream> {
    let reg = unique_register_ident(peripheral, register);
    let mask = u32::MAX >> (32 - field.width);
    let shift = if field.offset == 0 {
        None
    } else {
        let offset = &field.offset;
        Some(quote! { >> #offset })
    };

    // if the field touches the end of the register, a mask is not needed
    let value = if field.offset + field.width == 32 {
        quote! { #reg #shift }
    } else {
        quote! { (#reg #shift) & #mask }
    };

    Some(match field.access.get_read()? {
        Numericity::Numeric(..) => value,
        Numericity::Enumerated(..) => quote! {
            unsafe { #register_path::#field_ident::ReadVariant::from_bits(#value) }
        },
    })
}
