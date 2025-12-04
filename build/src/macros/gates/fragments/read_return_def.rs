use inflector::Inflector as _;
use proc_macro2::{Span, TokenStream};
use quote::format_ident;
use quote::quote;
use syn::Ident;

use crate::macros::gates::fragments::register_read_return_def;
use crate::macros::parsing::semantic::FieldEntry;
use crate::macros::{gates::utils::return_rank::ReturnRank, parsing::semantic::policies::Refine};

pub fn read_return_def<'cx, EntryPolicy>(rank: &ReturnRank<'cx, EntryPolicy>) -> Option<TokenStream>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    match rank {
        ReturnRank::Empty | ReturnRank::Field { .. } => None,
        ReturnRank::Register {
            register_item,
            fields,
            ..
        } => Some(register_read_return_def(
            register_item.register().type_name(),
            register_item,
            fields,
        )),
        ReturnRank::Peripheral(map) => {
            let defs = map
                .iter()
                .map(|(k, v)| {
                    (
                        Ident::new(k.to_string().to_pascal_case().as_str(), Span::call_site()),
                        v,
                    )
                })
                .map(|(ident, registers)| {
                    let register_idents = registers
                        .values()
                        .map(|(register_item, ..)| register_item.register().module_name());

                    let (register_tys, register_defs) = registers
                        .values()
                        .map(|(register_item, fields)| {
                            let ident = format_ident!(
                                "{}{}",
                                register_item.peripheral().type_name(),
                                register_item.register().type_name()
                            );

                            (
                                ident.clone(),
                                register_read_return_def(ident, register_item, fields),
                            )
                        })
                        .collect::<(Vec<_>, Vec<_>)>();

                    quote! {
                        struct #ident {
                            #(#register_idents: #register_tys,)*
                        }

                        #(#register_defs)*
                    }
                });

            Some(quote! { #(#defs)* })
        }
    }
}
