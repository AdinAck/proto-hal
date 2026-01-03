pub mod return_rank;

use std::{num::NonZeroU32, ops::Deref};

use model::{
    entitlement::{self, EntitlementIndex},
    field::{Field, FieldNode, numericity::Numericity},
    model::{Model, View},
    peripheral::Peripheral,
    register::Register,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use crate::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    parsing::{
        semantic::{
            self, FieldEntry, FieldItem, Gate, PeripheralEntry, RegisterItem,
            policies::{self, Refine, field::RequireBinding},
        },
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

pub fn module_suggestions(args: &syntax::Gate, diagnostics: &Diagnostics) -> Option<TokenStream> {
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

pub fn binding_suggestions(args: &syntax::Gate, diagnostics: &Diagnostics) -> Option<TokenStream> {
    fn bindings_in_tree<'t>(
        tree: &'t syntax::Tree,
    ) -> Box<dyn Iterator<Item = &'t syntax::Binding> + 't> {
        match &tree.node {
            syntax::Node::Branch(children) => Box::new(children.iter().flat_map(bindings_in_tree)),
            syntax::Node::Leaf(entry) => Box::new(entry.binding.iter()),
        }
    }

    if diagnostics.is_empty() {
        None
    } else {
        Some(
            args.trees
                .iter()
                .flat_map(|tree| bindings_in_tree(tree))
                .map(Deref::deref)
                .map(|binding| quote! { let _ = #binding; })
                .collect(),
        )
    }
}

/// Creates the mask used to occlude all provided fields.
///
/// If a field domain is [5:3], then the first byte of the mask would be:
/// `00111000`.
pub fn mask<'cx, EntryPolicy>(
    fields: impl Iterator<Item = &'cx FieldItem<'cx, EntryPolicy>>,
) -> Option<NonZeroU32>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>> + 'cx,
{
    NonZeroU32::new(fields.fold(0, |acc, field_item| {
        let field = field_item.field();
        acc | ((u32::MAX >> (32 - field.width)) << field.offset)
    }))
}

pub fn validate_entitlement_presence<'cx, PeripheralEntryPolicy, FieldEntryPolicy>(
    input: &semantic::Gate<'cx, PeripheralEntryPolicy, FieldEntryPolicy>,
    model: &'cx Model,
    cx_ident: &Ident,
    diagnostics: &mut Diagnostics,
    space: View<'cx, entitlement::Space>,
) where
    PeripheralEntryPolicy: Refine<'cx, Input = PeripheralEntry<'cx>>,
    FieldEntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    for pattern in space.patterns() {
        if pattern.fields(model).all(|field| {
            let (p, r) = field.parents();
            input
                .get_field(
                    p.module_name().to_string(),
                    r.module_name().to_string(),
                    field.module_name().to_string(),
                )
                .is_some()
        }) {
            return;
        }
    }

    diagnostics.push(Diagnostic::missing_entitlements(
        cx_ident,
        input
            .visit_fields()
            .map(|field| field.field().module_name().to_string()),
        space.patterns().map(|pattern| pattern.render(model)),
    ));
}

/// Creates the correct initial value for writing to a register without reading from it first.
pub fn static_initial<'cx>(
    model: &'cx Model,
    register_item: &RegisterItem<'cx, RequireBinding<'cx>>,
) -> Option<NonZeroU32> {
    let inert = register_item
        .register()
        .fields()
        .filter_map(|field| {
            let intert_variant = match field.access.get_write()? {
                Numericity::Numeric(..) => None?,
                Numericity::Enumerated(enumerated) => enumerated.some_inert(model)?,
            };

            Some((field, intert_variant))
        })
        .fold(0, |acc, (field, variant)| {
            acc | (variant.bits << field.offset)
        });

    // mask out values to be filled in by user
    let mask = mask(register_item.fields().values());

    // fill in statically known values from fields being statically transitioned
    let statics = register_item
        .fields()
        .values()
        .flat_map(|field_item| {
            let bits = match field_item.entry() {
                RequireBinding::View(..)
                | RequireBinding::Dynamic(..)
                | RequireBinding::DynamicTransition(..)
                | RequireBinding::Consumed(..) => None?,
                RequireBinding::Static(.., transition) => match transition {
                    semantic::Transition::Variant(.., variant) => variant.bits,
                    semantic::Transition::Expr(..) => None?,
                    semantic::Transition::Lit(lit_int) => lit_int
                        .base10_parse::<u32>()
                        .expect("lit int should be valid"),
                },
            };

            Some(bits << field_item.field().offset)
        })
        .reduce(|acc, value| acc | value)
        .unwrap_or(0);

    NonZeroU32::new((inert & !mask.map(|value| value.get()).unwrap_or(0)) | statics)
}

pub fn field_is_entangled<'cx, EntryPolicy>(
    model: &'cx Model,
    input: &semantic::Gate<'cx, policies::peripheral::ForbidPath, EntryPolicy>,
    field: &View<'cx, FieldNode>,
) -> bool
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    for other_field_item in input.visit_fields() {
        let other_field_numericity = other_field_item.field().resolvable();

        for entitlement_set in other_field_item
            .field()
            .write_entitlements()
            .into_iter()
            .chain(
                other_field_item
                    .field()
                    .ontological_entitlements()
                    .into_iter(),
            )
            .chain(
                other_field_item
                    .field()
                    .hardware_write_entitlements()
                    .into_iter(),
            )
            .chain(
                other_field_numericity
                    .iter()
                    .flat_map(|numericity| numericity.variants(model))
                    .flatten()
                    .flat_map(|variant| variant.statewise_entitlements().into_iter()),
            )
        {
            for entitlement_field in entitlement_set.entitlement_fields() {
                if entitlement_field.index() == field.index() {
                    return true;
                }
            }
        }
    }

    false
}

pub fn validate_entitlements<'cx>(
    input: &Gate<'cx, policies::peripheral::ForbidPath, policies::field::RequireBinding<'cx>>,
    model: &'cx Model,
    diagnostics: &mut Diagnostics,
) {
    for field in input.visit_fields() {
        let (RequireBinding::DynamicTransition(..) | RequireBinding::Static(..)) = field.entry()
        else {
            continue;
        };

        // check for write entitlements
        if let Some(write_entitlements) =
            model.try_get_entitlements(EntitlementIndex::Write(*field.field().index()))
        {
            validate_entitlement_presence(
                input,
                model,
                field.ident(),
                diagnostics,
                write_entitlements,
            );
        }

        // check for statewise entitlements
        let Some(Numericity::Enumerated(enumerated)) = field.field().resolvable() else {
            continue;
        };

        for variant in enumerated.variants(model) {
            if let Some(statewise_entitlements) =
                model.try_get_entitlements(EntitlementIndex::Variant(*variant.index()))
            {
                validate_entitlement_presence(
                    input,
                    model,
                    field.ident(),
                    diagnostics,
                    statewise_entitlements,
                );

                if let RequireBinding::DynamicTransition(..) = field.entry() {
                    diagnostics.push(Diagnostic::entangled_dynamic_transition(field.ident()));
                }
            }
        }
    }
}
