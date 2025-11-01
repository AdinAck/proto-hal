use ir::structures::{field::Field, hal::Hal, peripheral::Peripheral, register::Register};
use proc_macro2::Span;
use syn::{
    Ident, Path, parse_quote, punctuated::Punctuated, spanned::Spanned as _, token::PathSep,
};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    parsing::{
        semantic::{
            FieldItem, FieldKey, PeripheralItem, PeripheralKey, PeripheralMap, RegisterItem,
            RegisterKey, RegisterMap, Transition,
        },
        syntax::Tree,
    },
};

pub fn parse_peripheral<'args, 'hal>(
    peripheral_map: &mut PeripheralMap<'args, 'hal>,
    register_map: &mut RegisterMap<'args, 'hal>,
    tree: &'args Tree,
    model: &'hal Hal,
) -> Result<(), Diagnostics> {
    let mut diagnostics = Diagnostics::new();

    let path = tree.local_path();
    let mut segments = path.segments.iter().map(|segment| &segment.ident);
    let (peripheral, peripheral_path) = fuzzy_find_peripheral(&mut segments, path.span(), model)?;

    let peripheral_path = {
        let leading_colon = path.leading_colon;
        parse_quote! { #leading_colon #peripheral_path }
    };

    if let Some(register_segment) = segments.next() {
        // single register
        let register = find_register(register_segment, peripheral)?;

        let field_segment = segments
            .next()
            .ok_or(Diagnostic::unexpected_register(register_segment))?;

        put_field(
            register_map,
            tree,
            field_segment,
            peripheral_path,
            peripheral,
            register,
        )?;
    } else {
        // zero or many registers
        match tree {
            // many
            Tree::Branch { children, .. } => {
                for child in children {
                    if let Err(e) =
                        parse_register(register_map, child, &peripheral_path, peripheral)
                    {
                        diagnostics.extend(e);
                    }
                }
            }
            // zero
            Tree::Leaf { entry, .. } => {
                if let Some(..) = peripheral_map.insert(
                    PeripheralKey::from_model(&peripheral),
                    PeripheralItem {
                        path: path.clone(),
                        peripheral,
                        binding: entry.binding.as_ref(),
                    },
                ) {
                    Err(Diagnostic::item_already_specified(path))?
                }
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn parse_register<'args, 'hal>(
    register_map: &mut RegisterMap<'args, 'hal>,
    tree: &'args Tree,
    peripheral_path: &Path,
    peripheral: &'hal Peripheral,
) -> Result<(), Diagnostics> {
    let mut diagnostics = Diagnostics::new();

    let path = tree.local_path();
    let mut segments = path.segments.iter().map(|segment| &segment.ident);

    let register_segment = segments.next().expect("expected at least one path segment");

    let register = find_register(register_segment, peripheral)?;

    if let Some(field_segment) = segments.next() {
        // single field
        put_field(
            register_map,
            tree,
            field_segment,
            peripheral_path.clone(),
            peripheral,
            register,
        )?;
    } else {
        // zero or many fields
        match tree {
            // many
            Tree::Branch { children, .. } => {
                for child in children {
                    if let Err(e) = parse_field(
                        register_map,
                        child,
                        peripheral_path.clone(),
                        peripheral,
                        register,
                    ) {
                        diagnostics.push(e);
                    }
                }
            }
            // zero
            Tree::Leaf { .. } => Err(Diagnostic::unexpected_register(register_segment))?,
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn parse_field<'args, 'hal>(
    register_map: &mut RegisterMap<'args, 'hal>,
    tree: &'args Tree,
    peripheral_path: Path,
    peripheral: &'hal Peripheral,
    register: &'hal Register,
) -> Result<(), Diagnostic> {
    let field_segment = tree.local_path().require_ident()?;

    put_field(
        register_map,
        tree,
        field_segment,
        peripheral_path,
        peripheral,
        register,
    )
}

fn put_field<'args, 'hal>(
    register_map: &mut RegisterMap<'args, 'hal>,
    tree: &'args Tree,
    field_segment: &'args Ident,
    peripheral_path: Path,
    peripheral: &'hal Peripheral,
    register: &'hal Register,
) -> Result<(), Diagnostic> {
    let field = find_field(field_segment, register)?;

    match tree {
        Tree::Branch { path, .. } => Err(Diagnostic::path_cannot_contine(path, field_segment))?,
        Tree::Leaf { entry, .. } => {
            if let Some(..) = register_map
                .entry(RegisterKey::from_model(&peripheral, &register))
                .or_insert(RegisterItem {
                    peripheral_path,
                    peripheral,
                    register,
                    fields: Default::default(),
                })
                .fields
                .insert(
                    FieldKey::from_model(field),
                    FieldItem {
                        field,
                        binding: entry.binding.as_ref(),
                        transition: if let Some(transition) = entry.transition.as_ref() {
                            Some(Transition::parse(transition, field, field_segment)?)
                        } else {
                            None
                        },
                    },
                )
            {
                Err(Diagnostic::item_already_specified(field_segment))?
            }
        }
    }

    Ok(())
}

fn fuzzy_find_peripheral<'args, 'hal>(
    path: &mut impl Iterator<Item = &'args Ident>,
    span: Span,
    model: &'hal Hal,
) -> Result<(&'hal Peripheral, Path), Diagnostic> {
    let mut peripheral_path = Punctuated::<_, PathSep>::new();

    for ident in path {
        peripheral_path.push(ident);
        if let Some(peripheral) = model.peripherals.get(ident) {
            return Ok((peripheral, parse_quote! { #peripheral_path }));
        }
    }

    Err(Diagnostic::expected_peripheral_path(&span))
}

fn find_register<'args, 'hal>(
    ident: &'args Ident,
    peripheral: &'hal Peripheral,
) -> Result<&'hal Register, Diagnostic> {
    peripheral
        .registers
        .get(ident)
        .ok_or(Diagnostic::register_not_found(ident, peripheral))
}

fn find_field<'args, 'hal>(
    ident: &'args Ident,
    register: &'hal Register,
) -> Result<&'hal Field, Diagnostic> {
    register
        .fields
        .get(ident)
        .ok_or(Diagnostic::field_not_found(ident, register))
}
