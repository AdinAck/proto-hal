pub mod return_rank;

use indexmap::{IndexMap, IndexSet};
use model::structures::{
    entitlement::Entitlement,
    field::{Field, FieldIndex},
    model::{Model, View},
    peripheral::Peripheral,
    register::Register,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    parsing::{
        semantic::{self, FieldEntry, FieldItem, PeripheralEntry, policies::Refine},
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
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>> + 'cx,
{
    fields.fold(0, |acc, field_item| {
        let field = field_item.field();
        acc | ((u32::MAX >> (32 - field.width)) << field.offset)
    })
}

pub fn scan_entitlements<'cx, PeripheralEntryPolicy, FieldEntryPolicy>(
    input: &semantic::Gate<'cx, PeripheralEntryPolicy, FieldEntryPolicy>,
    model: &'cx Model,
    diagnostics: &mut Vec<Diagnostic>,
    cx_ident: &Ident,
    entitlements: View<'cx, IndexSet<Entitlement>>,
) -> IndexMap<FieldIndex, IndexSet<&'cx Entitlement>>
where
    PeripheralEntryPolicy: Refine<'cx, Input = PeripheralEntry<'cx>>,
    FieldEntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    let mut entitlement_fields = IndexMap::new();

    for entitlement in entitlements.iter() {
        entitlement_fields
            .entry(*entitlement.field(model).index())
            .or_insert_with(IndexSet::new)
            .insert(entitlement);
    }

    for (entitlement_field_index, field_entitlements) in &entitlement_fields {
        let entitlement_field = model.get_field(*entitlement_field_index);
        let (entitlement_peripheral, entitlement_register) = entitlement_field.parents();
        if input
            .get_field(
                entitlement_peripheral.module_name().to_string(),
                entitlement_register.module_name().to_string(),
                entitlement_field.module_name().to_string(),
            )
            .is_none()
        {
            diagnostics.push(Diagnostic::missing_entitlements(
                cx_ident,
                &entitlement_peripheral.module_name(),
                &entitlement_register.module_name(),
                &entitlement_field.module_name(),
                field_entitlements
                    .iter()
                    .map(|entitlement| entitlement.variant(model).type_name()),
            ));
        };
    }

    entitlement_fields
}
