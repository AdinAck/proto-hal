pub mod return_rank;

use ir::structures::{field::Field, peripheral::Peripheral, register::Register};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use crate::codegen::macros::{
    diagnostic::Diagnostics,
    parsing::{
        semantic::{FieldEntryRefinementInput, FieldItem, policies::Refine},
        syntax,
    },
};

pub fn unique_register_ident(peripheral: &Peripheral, register: &Register) -> Ident {
    format_ident!("{}_{}", peripheral.module_name(), register.module_name(),)
}

pub fn unique_field_ident(peripheral: &Peripheral, register: &Register, field: &Field) -> Ident {
    format_ident!(
        "{}_{}_{}",
        peripheral.module_name(),
        register.module_name(),
        field.module_name()
    )
}

pub fn render_diagnostics(diagnostics: Diagnostics) -> TokenStream {
    let errors = diagnostics
        .into_iter()
        .map(|e| syn::Error::from(e).to_compile_error());

    quote! {
        #(
            #errors
        )*
    }
}

pub fn suggestions(args: &syntax::Gate, diagnostics: &Diagnostics) -> Option<TokenStream> {
    fn tree_to_import(tree: &syntax::Tree) -> TokenStream {
        let path = &tree.path;
        match &tree.node {
            syntax::Node::Branch(children) => {
                let paths = children.iter().map(tree_to_import);

                quote! {
                    #path::{#(#paths),*}
                }
            }
            syntax::Node::Leaf(..) => quote! {
                #path as _
            },
        }
    }

    if diagnostics.is_empty() {
        None
    } else {
        Some(
            args.trees
                .iter()
                .map(|tree| {
                    let path = tree_to_import(tree);

                    quote! {
                        #[allow(unused_imports)]
                        use #path;
                    }
                })
                .collect(),
        )
    }
}

pub fn mask<'cx, EntryPolicy>(fields: impl Iterator<Item = &'cx FieldItem<'cx, EntryPolicy>>) -> u32
where
    EntryPolicy: Refine<'cx, Input = FieldEntryRefinementInput<'cx>> + 'cx,
{
    fields.fold(0, |acc, field_item| {
        let field = field_item.field();
        acc | ((u32::MAX >> (32 - field.width)) << field.offset)
    })
}
