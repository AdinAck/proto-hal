pub mod return_rank;

use std::{collections::HashMap, num::NonZeroU32, ops::Deref};

use indexmap::IndexSet;
use model::{
    entitlement,
    field::{Field, FieldIndex, FieldNode, numericity::Numericity},
    model::{Model, View},
    peripheral::Peripheral,
    register::Register,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, Path};

use crate::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::fragments,
    parsing::{
        semantic::{
            self, FieldEntry, FieldItem, Gate, PeripheralEntry, RegisterItem,
            policies::{self, Refine, field::GateEntry},
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

/// Ensure the provided entitlement spaces are satisfiable by the gate input.
pub fn validate_entitlement_dependency_presence<'cx, PeripheralEntryPolicy, FieldEntryPolicy>(
    input: &semantic::Gate<'cx, PeripheralEntryPolicy, FieldEntryPolicy>,
    model: &'cx Model,
    cx_ident: &Ident,
    diagnostics: &mut Diagnostics,
    spaces: impl IntoIterator<Item = View<'cx, entitlement::Space>>,
) where
    PeripheralEntryPolicy: Refine<'cx, Input = PeripheralEntry<'cx>>,
    FieldEntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    let mut unsatisfiable_spaces = Vec::new();

    'spaces: for space in spaces {
        // look for satisfiable patterns in each space
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
                // the space is satisfiable if at least one pattern is satisfiable, continue
                continue 'spaces;
            }
        }

        if !unsatisfiable_spaces.contains(&*space) {
            unsatisfiable_spaces.push(*space);
        }
    }

    if unsatisfiable_spaces.is_empty() {
        return;
    }

    diagnostics.push(Diagnostic::missing_entitlement_dependencies(
        model,
        cx_ident,
        input
            .visit_fields()
            .map(|field| field.field().module_name().to_string()),
        unsatisfiable_spaces.iter().map(Deref::deref),
    ));
}

/// Ensure the provided entitlement dependents are present in the gate.
pub fn validate_entitlement_dependent_presence<'cx, PeripheralEntryPolicy, FieldEntryPolicy>(
    input: &semantic::Gate<'cx, PeripheralEntryPolicy, FieldEntryPolicy>,
    model: &'cx Model,
    cx_ident: &Ident,
    diagnostics: &mut Diagnostics,
    dependents: &IndexSet<FieldIndex>,
) where
    PeripheralEntryPolicy: Refine<'cx, Input = PeripheralEntry<'cx>>,
    FieldEntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    let mut missing_dependents = IndexSet::new();

    for &dependent in dependents {
        let field = model.get_field(dependent);
        let (p, r) = field.parents();

        if input
            .get_field(
                p.module_name().to_string(),
                r.module_name().to_string(),
                field.module_name().to_string(),
            )
            .is_none()
        {
            missing_dependents.insert(field.module_name().to_string());
        }
    }

    if !missing_dependents.is_empty() {
        diagnostics.push(Diagnostic::missing_entitlement_dependents(
            cx_ident,
            missing_dependents.into_iter(),
        ));
    }
}

/// Creates the correct initial value for writing to a register without reading from it first.
pub fn static_initial<'cx>(
    model: &'cx Model,
    register_item: &RegisterItem<'cx, GateEntry<'cx>>,
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
                GateEntry::View(..) | GateEntry::Dynamic(..) | GateEntry::DynamicTransition(..) => {
                    None?
                }
                GateEntry::Static(.., transition) => match transition {
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

/// Determine if the provided field is an entitlement dependency of any other field in the gate.
///
/// This is important for the **modify** gate for determining which fields should be read dynamically.
/// Fields which are read will be available as runtime values for other fields being dynamically written to.
/// Additionaly, read fields will be part of the return from the modify gate. Fields which are present *purely* to
/// satisfy an entitlement of another field, **must** be omitted from the afformentioned process, as they are not
/// *dynamic*. This is important because the modify gate must know which type to accept (`Dynamic` vs a static state).
///
/// Some example scenarios:
///
/// Let field A have a write entitlement to a variant of field B.
///
/// ```ignore
/// modify! {
///     a(a) => b,
///     b(&b),
/// }
/// ```
///
/// This does not compile because field B is not read, it's static state is leveraged to perform the write to field A.
///
/// ```ignore
/// modify! {
///     a(&mut a),
///     b(&b),
/// }
/// ```
///
/// This not only compiles, but also *does* read field B since it is not an entitlement dependency of anything.
pub fn field_is_dependency<'cx>(
    model: &'cx Model,
    input: &semantic::Gate<'cx, policies::peripheral::ForbidPath, policies::field::GateEntry<'cx>>,
    field: &View<'cx, FieldNode>,
) -> bool {
    for other_field_item in input.visit_fields() {
        if other_field_item.ident() == &&field.ident {
            continue;
        }

        let other_field_numericity = other_field_item.field().resolvable();

        // if the other field is being written to, and the provided field supplies that write entitlement, the provided
        // field is a dependency
        if other_field_item.entry().transition().is_some()
            && let Some(write_space) = other_field_item.field().write_entitlements()
            && write_space
                .entitlement_fields()
                .any(|f| f.index() == field.index())
        {
            return true;
        }

        for entitlement_set in other_field_item
            .field()
            .write_entitlements()
            .into_iter()
            .chain(other_field_item.field().hardware_write_entitlements())
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
    input: &Gate<'cx, policies::peripheral::ForbidPath, policies::field::GateEntry<'cx>>,
    model: &'cx Model,
    diagnostics: &mut Diagnostics,
) {
    for field in input.visit_fields() {
        let (GateEntry::DynamicTransition(..) | GateEntry::Static(..)) = field.entry() else {
            continue;
        };

        // check for write entitlements
        if let Some(write_entitlements) = field.field().write_entitlements() {
            validate_entitlement_dependency_presence(
                input,
                model,
                field.ident(),
                diagnostics,
                [write_entitlements],
            );
        }

        let mut statewise_entangled = false;

        // check for statewise entitlements
        let mut statewise_entitlement_spaces = field.field().statewise_entitlements();

        if statewise_entitlement_spaces.next().is_some() {
            validate_entitlement_dependency_presence(
                input,
                model,
                field.ident(),
                diagnostics,
                statewise_entitlement_spaces,
            );

            statewise_entangled = true;
        }

        // reverse entitlements

        for dependents in model
            .try_get_reverse_statewise_entitlements(field.field().index())
            .iter()
            .chain(
                model
                    .try_get_reverse_hardware_write_entitlements(field.field().index())
                    .iter(),
            )
        {
            validate_entitlement_dependent_presence(
                input,
                model,
                field.ident(),
                diagnostics,
                dependents,
            );
        }

        if model
            .try_get_reverse_statewise_entitlements(field.field().index())
            .is_some()
        {
            statewise_entangled = true;
        }

        if statewise_entangled && let GateEntry::DynamicTransition(..) = field.entry() {
            diagnostics.push(Diagnostic::entangled_dynamic_transition(field.ident()));
        }
    }
}

pub fn input_field_states<'cx>(
    input: &semantic::Gate<'cx, policies::peripheral::ForbidPath, policies::field::GateEntry<'cx>>,
    field_dependencies: &HashMap<&FieldIndex, bool>,
) -> HashMap<FieldIndex, TokenStream> {
    HashMap::from_iter(input.visit_peripherals().flat_map(|peripheral_item| {
        peripheral_item
            .registers()
            .values()
            .flat_map(|register_item| {
                register_item.fields().values().map(|field_item| {
                    let generics = fragments::generics(
                        register_item,
                        field_item,
                        *field_dependencies.get(field_item.field().index()).unwrap(),
                    );

                    (
                        *field_item.field().index(),
                        fragments::input_ty(
                            peripheral_item.path(),
                            register_item.ident(),
                            field_item.ident(),
                            field_item.field(),
                            generics.input.as_ref(),
                        ),
                    )
                })
            })
    }))
}

pub fn field_states_after_register<'cx>(
    field_states: &HashMap<FieldIndex, TokenStream>,
    field_dependencies: &HashMap<&FieldIndex, bool>,
    peripheral_path: &Path,
    register_item: &RegisterItem<'cx, GateEntry<'cx>>,
) -> HashMap<FieldIndex, TokenStream> {
    // field states after this register
    let mut post_field_states = field_states.clone();

    for field_item in register_item.fields().values() {
        let generics = fragments::generics(
            register_item,
            field_item,
            *field_dependencies.get(field_item.field().index()).unwrap(),
        );

        let Some(transition_return_ty) = fragments::transition_return_ty(
            peripheral_path,
            register_item.ident(),
            field_item.entry(),
            field_item.field(),
            field_item.ident(),
            generics.output.as_ref(),
        ) else {
            continue;
        };

        post_field_states.insert(*field_item.field().index(), transition_return_ty);
    }

    post_field_states
}
