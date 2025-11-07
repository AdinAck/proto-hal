use ir::structures::{field::Field, hal::Hal, peripheral::Peripheral, register::Register};
use proc_macro2::Span;
use syn::{
    Ident, Path, parse_quote, punctuated::Punctuated, spanned::Spanned as _, token::PathSep,
};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    parsing::{
        semantic::{
            Entry, FieldEntryRefinementInput, FieldItem, FieldKey, PeripheralItem, PeripheralKey,
            PeripheralMap, RegisterItem, RegisterKey, RegisterMap,
            policies::{Filter, Refine},
        },
        syntax::{Node, Tree},
    },
};

pub fn parse_peripheral<'cx, PeripheralPolicy, EntryPolicy>(
    peripheral_map: &mut PeripheralMap<'cx>,
    register_map: &mut RegisterMap<'cx, EntryPolicy>,
    tree: &'cx Tree,
    model: &'cx Hal,
) -> Result<(), Diagnostics>
where
    PeripheralPolicy: Filter,
    EntryPolicy: Refine<'cx, Input = FieldEntryRefinementInput<'cx>>,
{
    let mut diagnostics = Diagnostics::new();

    let path = &tree.path;
    let mut segments = path.segments.iter().map(|segment| &segment.ident);
    let (peripheral, peripheral_path, peripheral_ident) =
        fuzzy_find_peripheral(&mut segments, path.span(), model)?;

    let peripheral_path: Path = {
        let leading_colon = path.leading_colon;
        parse_quote! { #leading_colon #peripheral_path }
    };

    let Some(register_ident) = segments.next() else {
        // path ends on peripheral item
        match &tree.node {
            Node::Leaf(entry) => {
                // peripheral entry
                if !PeripheralPolicy::accepted() {
                    Err(Diagnostic::unexpected_peripheral(&peripheral.module_name()))?
                }

                if let Some(..) = peripheral_map.insert(
                    PeripheralKey::from_model(&peripheral),
                    PeripheralItem {
                        path: path.clone(),
                        ident: peripheral_ident,
                        peripheral,
                        binding: entry.binding.as_ref(),
                    },
                ) {
                    Err(Diagnostic::item_already_specified(path))?
                }
            }
            Node::Branch(children) => {
                for child in children {
                    if let Err(e) =
                        parse_register(register_map, child, &peripheral_path, peripheral)
                    {
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

    let register = find_register(register_ident, peripheral)?;

    let Some(field_ident) = segments.next() else {
        // path ends on register item

        match &tree.node {
            Node::Leaf(..) => Err(Diagnostic::unexpected_register(register_ident))?,
            Node::Branch(children) => {
                for child in children {
                    if let Err(e) = parse_field(
                        register_map,
                        child,
                        peripheral_path.clone(),
                        peripheral,
                        register_ident,
                        register,
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
        register_map,
        tree,
        field_ident,
        peripheral_path,
        peripheral,
        register_ident,
        register,
    )?;

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn parse_register<'cx, EntryPolicy>(
    register_map: &mut RegisterMap<'cx, EntryPolicy>,
    tree: &'cx Tree,
    peripheral_path: &Path,
    peripheral: &'cx Peripheral,
) -> Result<(), Diagnostics>
where
    EntryPolicy: Refine<'cx, Input = FieldEntryRefinementInput<'cx>>,
{
    let mut diagnostics = Diagnostics::new();

    let path = &tree.path;
    let mut segments = path.segments.iter().map(|segment| &segment.ident);

    let register_ident = segments.next().expect("expected at least one path segment");

    let register = find_register(register_ident, peripheral)?;

    if let Some(field_ident) = segments.next() {
        // single field
        put_field(
            register_map,
            tree,
            field_ident,
            peripheral_path.clone(),
            peripheral,
            register_ident,
            register,
        )?;
    } else {
        // zero or many fields
        match &tree.node {
            // many
            Node::Branch(children) => {
                for child in children {
                    if let Err(e) = parse_field(
                        register_map,
                        child,
                        peripheral_path.clone(),
                        peripheral,
                        register_ident,
                        register,
                    ) {
                        diagnostics.extend(e);
                    }
                }
            }
            // zero
            Node::Leaf(..) => Err(Diagnostic::unexpected_register(register_ident))?,
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn parse_field<'cx, EntryPolicy>(
    register_map: &mut RegisterMap<'cx, EntryPolicy>,
    tree: &'cx Tree,
    peripheral_path: Path,
    peripheral: &'cx Peripheral,
    register_ident: &'cx Ident,
    register: &'cx Register,
) -> Result<(), Diagnostics>
where
    EntryPolicy: Refine<'cx, Input = FieldEntryRefinementInput<'cx>>,
{
    let field_segment = tree
        .path
        .require_ident()
        .map_err(Into::<Diagnostic>::into)?;

    put_field(
        register_map,
        tree,
        field_segment,
        peripheral_path,
        peripheral,
        register_ident,
        register,
    )
}

fn put_field<'cx, EntryPolicy>(
    register_map: &mut RegisterMap<'cx, EntryPolicy>,
    tree: &'cx Tree,
    field_ident: &'cx Ident,
    peripheral_path: Path,
    peripheral: &'cx Peripheral,
    register_ident: &'cx Ident,
    register: &'cx Register,
) -> Result<(), Diagnostics>
where
    EntryPolicy: Refine<'cx, Input = FieldEntryRefinementInput<'cx>>,
{
    let field = find_field(field_ident, register)?;

    match &tree.node {
        Node::Branch(..) => Err(Diagnostic::path_cannot_contine(&tree.path, field_ident))?,
        Node::Leaf(entry) => {
            if let Some(..) = register_map
                .entry(RegisterKey::from_model(&peripheral, &register))
                .or_insert(RegisterItem {
                    peripheral_path,
                    ident: register_ident,
                    peripheral,
                    register,
                    fields: Default::default(),
                })
                .fields
                .insert(
                    FieldKey::from_model(field),
                    FieldItem {
                        ident: field_ident,
                        field,
                        entry: EntryPolicy::refine((
                            field_ident,
                            Entry::parse(entry, field, field_ident)?,
                        ))?,
                    },
                )
            {
                Err(Diagnostic::item_already_specified(field_ident))?
            }
        }
    }

    Ok(())
}

fn fuzzy_find_peripheral<'input, 'model>(
    path: &mut impl Iterator<Item = &'input Ident>,
    span: Span,
    model: &'model Hal,
) -> Result<(&'model Peripheral, Path, &'input Ident), Diagnostic> {
    let mut peripheral_path = Punctuated::<_, PathSep>::new();

    for ident in path {
        peripheral_path.push(ident);
        if let Some(peripheral) = model.peripherals.get(ident) {
            return Ok((peripheral, parse_quote! { #peripheral_path }, ident));
        }
    }

    Err(Diagnostic::expected_peripheral_path(&span))
}

fn find_register<'input, 'model>(
    ident: &'input Ident,
    peripheral: &'model Peripheral,
) -> Result<&'model Register, Diagnostic> {
    peripheral
        .registers
        .get(ident)
        .ok_or(Diagnostic::register_not_found(ident, peripheral))
}

fn find_field<'input, 'model>(
    ident: &'input Ident,
    register: &'model Register,
) -> Result<&'model Field, Diagnostic> {
    register
        .fields
        .get(ident)
        .ok_or(Diagnostic::field_not_found(ident, register))
}
