use model::{
    field::FieldNode,
    model::{Model, View},
    peripheral::PeripheralNode,
    register::RegisterNode,
};
use syn::{Ident, Path, parse_quote, punctuated::Punctuated, token::PathSep};

use crate::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    parsing::{
        semantic::{
            FieldEntry, FieldItem, FieldKey, PeripheralItem, PeripheralKey, PeripheralMap,
            RegisterItem, RegisterKey, RegisterMap, entry::PeripheralEntry, policies::Refine,
        },
        syntax::{Node, Tree},
    },
};

pub fn parse_peripheral<'cx, PeripheralEntryPolicy, FieldEntryPolicy>(
    model: &'cx Model,
    peripheral_map: &mut PeripheralMap<'cx, PeripheralEntryPolicy, FieldEntryPolicy>,
    tree: &'cx Tree,
) -> Result<(), Diagnostics>
where
    PeripheralEntryPolicy: Refine<'cx, Input = PeripheralEntry<'cx>>,
    FieldEntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    let mut diagnostics = Diagnostics::new();

    let path = &tree.path;
    let mut segments = path.segments.iter().map(|segment| &segment.ident);

    let (peripheral_path, peripheral_ident, peripheral) = take_until(&mut segments, |ident| {
        model.try_get_peripheral(ident.clone().into())
    })?
    .expect("node path should not be empty");

    let peripheral_path = {
        let leading_colon = path.leading_colon;
        parse_quote! { #leading_colon #peripheral_path }
    };

    let peripheral_key = PeripheralKey::from_model(&peripheral);

    if let Some(existing) = peripheral_map.get(&peripheral_key)
        && existing.entry().is_some()
    {
        Err(Diagnostic::item_already_specified(path))?
    }

    let peripheral_item = peripheral_map
        .entry(peripheral_key)
        .or_insert(PeripheralItem {
            path: peripheral_path,
            ident: peripheral_ident,
            peripheral: peripheral.clone(),
            entry: None,
            registers: Default::default(),
        });

    let Some((register_path, register_ident, register)) =
        take_until(&mut segments, |ident| peripheral.try_get_register(ident))?
    else {
        // path ends on peripheral item
        match &tree.node {
            Node::Leaf(entry) => {
                peripheral_item.entry.replace(PeripheralEntryPolicy::refine(
                    peripheral_ident,
                    PeripheralEntry::parse(entry, peripheral_ident)?,
                )?);
            }
            Node::Branch(children) => {
                for child in children {
                    if let Err(e) = parse_register(
                        model,
                        &mut peripheral_item.registers,
                        child,
                        peripheral.clone(),
                    ) {
                        diagnostics.extend(e);
                    }
                }
            }
        }

        return if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        };
    };

    let Some((field_path, field_ident, field)) =
        take_until(&mut segments, |ident| register.try_get_field(ident))?
    else {
        // path ends on register item

        match &tree.node {
            Node::Leaf(..) => {
                peripheral_item.registers.insert(
                    RegisterKey::from_model(&register),
                    RegisterItem {
                        path: register_path,
                        ident: register_ident,
                        peripheral,
                        register,
                        fields: Default::default(),
                    },
                );
            }
            Node::Branch(children) => {
                for child in children {
                    if let Err(e) = parse_field(
                        model,
                        &mut peripheral_item.registers,
                        child,
                        peripheral.clone(),
                        register_ident,
                        register_path.clone(),
                        register.clone(),
                    ) {
                        diagnostics.extend(e);
                    }
                }
            }
        };

        return if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        };
    };

    put_field(
        model,
        &mut peripheral_item.registers,
        tree,
        field_ident,
        field_path,
        field,
        peripheral,
        register_ident,
        register_path,
        register,
    )?;

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn parse_register<'cx, EntryPolicy>(
    model: &'cx Model,
    register_map: &mut RegisterMap<'cx, EntryPolicy>,
    tree: &'cx Tree,
    peripheral: View<'cx, PeripheralNode>,
) -> Result<(), Diagnostics>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    let mut diagnostics = Diagnostics::new();

    let path = &tree.path;
    let mut segments = path.segments.iter().map(|segment| &segment.ident);

    let (register_path, register_ident, register) =
        take_until(&mut segments, |ident| peripheral.try_get_register(ident))?
            .expect("node path should not be empty");

    if let Some((field_path, field_ident, field)) =
        take_until(&mut segments, |ident| register.try_get_field(ident))?
    {
        // single field
        put_field(
            model,
            register_map,
            tree,
            field_ident,
            field_path,
            field,
            peripheral,
            register_ident,
            register_path,
            register,
        )?;
    } else {
        // zero or many fields
        match &tree.node {
            // many
            Node::Branch(children) => {
                for child in children {
                    if let Err(e) = parse_field(
                        model,
                        register_map,
                        child,
                        peripheral.clone(),
                        register_ident,
                        register_path.clone(),
                        register.clone(),
                    ) {
                        diagnostics.extend(e);
                    }
                }
            }
            // zero
            Node::Leaf(..) => {
                register_map.insert(
                    RegisterKey::from_model(&register),
                    RegisterItem {
                        path: register_path,
                        ident: register_ident,
                        peripheral,
                        register,
                        fields: Default::default(),
                    },
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn parse_field<'cx, EntryPolicy>(
    model: &'cx Model,
    register_map: &mut RegisterMap<'cx, EntryPolicy>,
    tree: &'cx Tree,
    peripheral: View<'cx, PeripheralNode>,
    register_ident: &'cx Ident,
    register_path: Path,
    register: View<'cx, RegisterNode>,
) -> Result<(), Diagnostics>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    let path = &tree.path;
    let mut segments = path.segments.iter().map(|segment| &segment.ident);

    let (field_path, field_ident, field) =
        take_until(&mut segments, |ident| register.try_get_field(ident))?
            .expect("node path should not be empty");

    put_field(
        model,
        register_map,
        tree,
        field_ident,
        field_path,
        field,
        peripheral,
        register_ident,
        register_path,
        register,
    )
}

#[allow(clippy::too_many_arguments)]
fn put_field<'cx, EntryPolicy>(
    model: &'cx Model,
    register_map: &mut RegisterMap<'cx, EntryPolicy>,
    tree: &'cx Tree,
    field_ident: &'cx Ident,
    field_path: Path,
    field: View<'cx, FieldNode>,
    peripheral: View<'cx, PeripheralNode>,
    register_ident: &'cx Ident,
    register_path: Path,
    register: View<'cx, RegisterNode>,
) -> Result<(), Diagnostics>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    match &tree.node {
        Node::Branch(..) => Err(Diagnostic::path_cannot_contine(&tree.path, field_ident))?,
        Node::Leaf(entry) => {
            if register_map
                .entry(RegisterKey::from_model(&register))
                .or_insert(RegisterItem {
                    path: register_path,
                    ident: register_ident,
                    peripheral,
                    register,
                    fields: Default::default(),
                })
                .fields
                .insert(
                    FieldKey::from_model(&field),
                    FieldItem {
                        path: field_path,
                        ident: field_ident,
                        entry: EntryPolicy::refine(
                            field_ident,
                            FieldEntry::parse(model, entry, &field, field_ident)?,
                        )?,
                        field,
                    },
                )
                .is_some()
            {
                Err(Diagnostic::item_already_specified(field_ident))?
            }
        }
    }

    Ok(())
}

/// Take from a tree node path until the provided predicate is met, or the path ends.
///
/// If the predicate is satisfied, the path region consumed up to and including the satisfying ident is returned along
/// with the satisfying ident.
///
/// If the predicate is *not* satisfied, and the path region consumed is empty, `None` is returned.
///
/// If the predicate is *not* satisfied, and the path region consumed is *not* empty, an "item not found" diagnostic is
/// returned.
fn take_until<'cx, T>(
    segments: &mut impl Iterator<Item = &'cx Ident>,
    predicate: impl Fn(&Ident) -> Option<T>,
) -> Result<Option<(Path, &'cx Ident, T)>, Diagnostic> {
    let mut path = Punctuated::<_, PathSep>::new();

    for segment in segments {
        path.push(segment);

        if let Some(t) = predicate(segment) {
            return Ok(Some((parse_quote! { #path }, segment, t)));
        }
    }

    if path.is_empty() {
        Ok(None)
    } else {
        Err(Diagnostic::item_not_found(&path))
    }
}
