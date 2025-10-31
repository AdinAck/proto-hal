use indexmap::IndexMap;
use ir::structures::{field::Field, hal::Hal, peripheral::Peripheral, register::Register};
use proc_macro2::Span;
use syn::{Ident, Path, parse_quote, spanned::Spanned};

use crate::codegen::macros::parsing::lexical::{Entry, Tree};

pub struct PeripheralItem<'args, 'hal> {
    path: Path,
    peripheral: &'hal Peripheral,
    entry: &'args Entry,
}

pub struct RegisterItem<'args, 'hal> {
    peripheral_path: Path,
    peripheral: &'hal Peripheral,
    register: &'hal Register,
    fields: IndexMap<Ident, FieldItem<'args, 'hal>>,
}

fn fuzzy_find_peripheral<'args, 'hal>(
    path: &mut impl Iterator<Item = &'args Ident>,
    span: Span,
    model: &'hal Hal,
) -> Result<&'hal Peripheral, syn::Error> {
    path.find_map(|ident| model.peripherals.get(ident))
        .ok_or(syn::Error::new(
            span,
            format!("expected path to peripheral"),
        ))
}

fn find_register<'args, 'hal>(
    ident: &'args Ident,
    peripheral: &'hal Peripheral,
) -> Result<&'hal Register, syn::Error> {
    peripheral
        .registers
        .get(ident)
        .ok_or(syn::Error::new_spanned(
            ident,
            format!(
                "register \"{ident}\" not found within peripheral \"{}\"",
                peripheral.module_name()
            ),
        ))
}

fn find_field<'args, 'hal>(
    ident: &'args Ident,
    register: &'hal Register,
) -> Result<&'hal Field, syn::Error> {
    register.fields.get(ident).ok_or(syn::Error::new_spanned(
        ident,
        format!(
            "field \"{ident}\" not found within register \"{}\"",
            register.module_name()
        ),
    ))
}

fn parse_peripheral<'args, 'hal>(
    peripheral_map: &mut IndexMap<Key, PeripheralItem<'args, 'hal>>,
    register_map: &mut IndexMap<Key, RegisterItem<'args, 'hal>>,
    tree: &'args Tree,
    model: &'hal Hal,
) -> Result<(), Vec<syn::Error>> {
    let mut errors = Vec::new();

    let path = tree.local_path();
    let mut segments = path.segments.iter().map(|segment| &segment.ident);
    let peripheral =
        fuzzy_find_peripheral(&mut segments, path.span(), model).map_err(|e| vec![e])?;

    let peripheral_path = {
        let leading_colon = path.leading_colon;
        let path = segments.clone();
        parse_quote! { #leading_colon #(#path)::* }
    };

    if let Some(register_segment) = segments.next() {
        // single register
        let register = find_register(register_segment, peripheral).map_err(|e| vec![e])?;

        let field_segment = segments
            .next()
            .ok_or(syn::Error::new_spanned(
                register_segment,
                format!("unexpected register at end of path"),
            ))
            .map_err(|e| vec![e])?;

        put_field(
            register_map,
            tree,
            field_segment,
            peripheral_path,
            peripheral,
            register,
        )
        .map_err(|e| vec![e])?;
    } else {
        // zero or many registers
        match tree {
            // many
            Tree::Branch { children, .. } => {
                for child in children {
                    if let Err(e) =
                        parse_register(register_map, child, &peripheral_path, peripheral)
                    {
                        errors.extend(e);
                    }
                }
            }
            // zero
            Tree::Leaf { entry, .. } => {
                if let Some(..) = peripheral_map.insert(
                    Key::from_peripheral(&peripheral),
                    PeripheralItem {
                        path: path.clone(),
                        peripheral,
                        entry,
                    },
                ) {
                    Err(vec![syn::Error::new_spanned(
                        path,
                        "this item has already been specified",
                    )])?
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn parse_register<'args, 'hal>(
    register_map: &mut IndexMap<Key, RegisterItem<'args, 'hal>>,
    tree: &'args Tree,
    peripheral_path: &Path,
    peripheral: &'hal Peripheral,
) -> Result<(), Vec<syn::Error>> {
    let mut errors = Vec::new();

    let path = tree.local_path();
    let mut segments = path.segments.iter().map(|segment| &segment.ident);

    let register_segment = segments.next().expect("expected at least one path segment");

    let register = find_register(register_segment, peripheral).map_err(|e| vec![e])?;

    if let Some(field_segment) = segments.next() {
        // single field
        put_field(
            register_map,
            tree,
            field_segment,
            peripheral_path.clone(),
            peripheral,
            register,
        )
        .map_err(|e| vec![e])?;
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
                        errors.push(e);
                    }
                }
            }
            // zero
            Tree::Leaf { .. } => Err(vec![syn::Error::new_spanned(
                register_segment,
                "unexpected register at end of path",
            )])?,
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn parse_field<'args, 'hal>(
    register_map: &mut IndexMap<Key, RegisterItem<'args, 'hal>>,
    tree: &'args Tree,
    peripheral_path: Path,
    peripheral: &'hal Peripheral,
    register: &'hal Register,
) -> Result<(), syn::Error> {
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
    register_map: &mut IndexMap<Key, RegisterItem<'args, 'hal>>,
    tree: &'args Tree,
    field_segment: &Ident,
    peripheral_path: Path,
    peripheral: &'hal Peripheral,
    register: &'hal Register,
) -> Result<(), syn::Error> {
    let field = find_field(field_segment, register)?;

    match tree {
        Tree::Branch { path, .. } => Err(syn::Error::new_spanned(
            path,
            "paths cannot continue within fields",
        ))?,
        Tree::Leaf { entry, .. } => {
            if let Some(..) = register_map
                .entry(Key::from_register(&peripheral, &register))
                .or_insert(RegisterItem {
                    peripheral_path,
                    peripheral,
                    register,
                    fields: Default::default(),
                })
                .fields
                .insert(field.module_name(), FieldItem { field, entry })
            {
                Err(syn::Error::new_spanned(
                    field_segment,
                    "this item has already been specified",
                ))?
            }
        }
    }

    Ok(())
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Key(String);

impl Key {
    pub fn from_peripheral(peripheral: &Peripheral) -> Self {
        Self(peripheral.module_name().to_string())
    }

    pub fn from_register(peripheral: &Peripheral, register: &Register) -> Self {
        Self(format!(
            "{}{}",
            peripheral.module_name(),
            register.module_name()
        ))
    }

    pub fn from_field(field: &Field) -> Self {
        Self(field.module_name().to_string())
    }
}

pub struct FieldItem<'args, 'hal> {
    field: &'hal Field,
    entry: &'args Entry,
}
