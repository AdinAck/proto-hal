mod keys;
mod transition;

use indexmap::IndexMap;
use ir::structures::{field::Field, hal::Hal, peripheral::Peripheral, register::Register};
use proc_macro2::Span;
use syn::{Ident, Path, parse_quote, punctuated::Punctuated, spanned::Spanned, token::PathSep};
use ters::ters;

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    parsing::syntax::{self, Binding, Tree},
};

pub use keys::*;
pub use transition::Transition;

type PeripheralMap<'args, 'hal> = IndexMap<PeripheralKey, PeripheralItem<'args, 'hal>>;
type RegisterMap<'args, 'hal> = IndexMap<RegisterKey, RegisterItem<'args, 'hal>>;

/// The semantically parsed gate input, with corresponding model elements.
#[ters]
pub struct Gate<'args, 'hal> {
    #[get]
    peripheral_map: PeripheralMap<'args, 'hal>,
    #[get]
    register_map: RegisterMap<'args, 'hal>,
}

impl<'args, 'hal> Gate<'args, 'hal> {
    /// Parse the gate input against the model to produce a semantic gate input.
    pub fn parse(args: &'args syntax::Gate, model: &'hal Hal) -> Result<Self, Diagnostics> {
        let mut peripheral_map = Default::default();
        let mut register_map = Default::default();

        for tree in &args.trees {
            parse_peripheral(&mut peripheral_map, &mut register_map, tree, model)?;
        }

        Ok(Self {
            peripheral_map,
            register_map,
        })
    }

    pub fn get_peripheral(&self, ident: impl Into<String>) -> Option<&PeripheralItem<'args, 'hal>> {
        self.peripheral_map.get(&PeripheralKey::from_ident(ident))
    }

    pub fn peripherals(&self) -> impl Iterator<Item = &PeripheralItem<'args, 'hal>> {
        self.peripheral_map.values()
    }

    pub fn get_register(
        &self,
        peripheral_ident: impl Into<String>,
        register_ident: impl Into<String>,
    ) -> Option<&RegisterItem<'args, 'hal>> {
        self.register_map
            .get(&RegisterKey::from_ident(peripheral_ident, register_ident))
    }

    pub fn registers(&self) -> impl Iterator<Item = &RegisterItem<'args, 'hal>> {
        self.register_map.values()
    }

    pub fn get_field(
        &self,
        peripheral_ident: impl Into<String>,
        register_ident: impl Into<String>,
        field_ident: impl Into<String>,
    ) -> Option<(&RegisterItem<'args, 'hal>, &FieldItem<'args, 'hal>)> {
        self.register_map
            .get(&RegisterKey::from_ident(peripheral_ident, register_ident))
            .and_then(|register_item| {
                register_item
                    .fields
                    .get(&FieldKey::from_ident(field_ident))
                    .map(|field_item| (register_item, field_item))
            })
    }

    pub fn fields(&self) -> impl Iterator<Item = &FieldItem<'args, 'hal>> {
        self.register_map
            .values()
            .flat_map(|register_item| register_item.fields.values())
    }
}

/// A peripheral-level item present in the gate.
#[ters]
pub struct PeripheralItem<'args, 'hal> {
    #[get]
    path: Path,
    #[get]
    peripheral: &'hal Peripheral,
    #[get]
    binding: Option<&'args Binding>,
}

/// A register-level item present in the gate.
#[ters]
pub struct RegisterItem<'args, 'hal> {
    #[get]
    peripheral_path: Path,
    #[get]
    peripheral: &'hal Peripheral,
    #[get]
    register: &'hal Register,
    #[get]
    fields: IndexMap<FieldKey, FieldItem<'args, 'hal>>,
}

/// A field-level item present in the gate.
#[ters]
pub struct FieldItem<'args, 'hal> {
    #[get]
    field: &'hal Field,
    #[get]
    binding: Option<&'args Binding>,
    #[get]
    transition: Option<Transition<'args, 'hal>>,
}

fn parse_peripheral<'args, 'hal>(
    peripheral_map: &mut IndexMap<PeripheralKey, PeripheralItem<'args, 'hal>>,
    register_map: &mut IndexMap<RegisterKey, RegisterItem<'args, 'hal>>,
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
    register_map: &mut IndexMap<RegisterKey, RegisterItem<'args, 'hal>>,
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
    register_map: &mut IndexMap<RegisterKey, RegisterItem<'args, 'hal>>,
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
    register_map: &mut IndexMap<RegisterKey, RegisterItem<'args, 'hal>>,
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

#[cfg(test)]
mod tests {
    use ir::{
        access::Access,
        structures::{
            field::{Field, Numericity},
            hal::Hal,
            peripheral::Peripheral,
            register::Register,
        },
    };
    use quote::quote;
    use syn::{Ident, Path, parse_quote};

    use crate::codegen::macros::{
        diagnostic,
        parsing::{semantic::Gate, syntax},
    };

    #[test]
    fn single_peripheral() {
        let peripheral_name = "foo";
        let peripheral_path = parse_quote! { ::external::foo };
        let peripheral_binding = quote! { some_foo };
        let model = Hal::new([Peripheral::new(peripheral_name, 0, [])]);
        let tokens = quote! {
            #peripheral_path(#peripheral_binding)
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactical parsing should succeed");
        let gate = Gate::parse(&args, &model).expect("semantic parsing should succeed");

        let peripheral = gate
            .get_peripheral(peripheral_name)
            .expect("peripheral should exist");

        assert_eq!(peripheral.path(), &peripheral_path);
        assert_eq!(
            peripheral.binding(),
            &Some(&parse_quote! { #peripheral_binding })
        );
        assert_eq!(peripheral.peripheral().ident, peripheral_name);
    }

    #[test]
    fn multiple_peripherals() {
        let peripheral0_name = "foo";
        let peripheral0_path = parse_quote! { ::external::foo };
        let peripheral0_binding = quote! { some_foo };
        let peripheral1_name = "bar";
        let peripheral1_path = parse_quote! { external::stuff::bar };
        let model = Hal::new([
            Peripheral::new(peripheral0_name, 0, []),
            Peripheral::new(peripheral1_name, 0, []),
        ]);
        let tokens = quote! {
            #peripheral0_path(#peripheral0_binding),
            #peripheral1_path,
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactical parsing should succeed");
        let gate = Gate::parse(&args, &model).expect("semantic parsing should succeed");

        let peripheral0 = gate
            .get_peripheral(peripheral0_name)
            .expect("peripheral should exist");

        let peripheral1 = gate
            .get_peripheral(peripheral1_name)
            .expect("peripheral should exist");

        assert_eq!(peripheral0.path(), &peripheral0_path);
        assert_eq!(
            peripheral0.binding(),
            &Some(&parse_quote! { #peripheral0_binding })
        );
        assert_eq!(peripheral0.peripheral().ident, peripheral0_name);

        assert_eq!(peripheral1.path(), &peripheral1_path);
        assert!(peripheral1.binding().is_none());
        assert_eq!(peripheral1.peripheral().ident, peripheral1_name);
    }

    #[test]
    fn single_register() {
        let peripheral_name = "foo";
        let peripheral_path: Path = parse_quote! { ::external::foo };
        let register_name = "bar";
        let register_ident: Ident = parse_quote! { bar };
        let model = Hal::new([Peripheral::new(
            peripheral_name,
            0,
            [Register::new(register_name, 0, [])],
        )]);
        let tokens = quote! {
            #peripheral_path::#register_ident
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactical parsing should succeed");
        let gate = Gate::parse(&args, &model);

        assert!(gate.is_err_and(|diagnostics| {
            diagnostics
                .iter()
                .any(|diagnostic| matches!(diagnostic.kind(), diagnostic::Kind::UnexpectedRegister))
        }))
    }

    #[test]
    fn field_must_be_writable() {
        let peripheral_name = "foo";
        let peripheral_path: Path = parse_quote! { ::external::foo };
        let register_name = "bar";
        let register_ident: Ident = parse_quote! { bar };
        let field_name = "baz";
        let field_ident: Ident = parse_quote! { baz };
        let model = Hal::new([Peripheral::new(
            peripheral_name,
            0,
            [Register::new(
                register_name,
                0,
                [Field::new(
                    field_name,
                    0,
                    0,
                    Access::read(Numericity::Numeric),
                )],
            )],
        )]);
        let tokens = quote! {
            #peripheral_path::#register_ident::#field_ident(&mut my_field) => Foo,
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactical parsing should succeed");
        let gate = Gate::parse(&args, &model);

        assert!(gate.is_err_and(|diagnostics| {
            diagnostics.iter().any(|diagnostic| {
                matches!(diagnostic.kind(), diagnostic::Kind::FieldMustBeWritable)
            })
        }))
    }

    #[test]
    fn single_field() {
        let peripheral_name = "foo";
        let peripheral_path: Path = parse_quote! { ::external::foo };
        let register_name = "bar";
        let register_ident: Ident = parse_quote! { bar };
        let field_name = "baz";
        let field_ident: Ident = parse_quote! { baz };
        let model = Hal::new([Peripheral::new(
            peripheral_name,
            0,
            [Register::new(
                register_name,
                0,
                [Field::new(
                    field_name,
                    0,
                    0,
                    Access::write(Numericity::Numeric),
                )],
            )],
        )]);
        let tokens = quote! {
            #peripheral_path::#register_ident::#field_ident(&mut my_field) => Foo,
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactical parsing should succeed");
        let gate = Gate::parse(&args, &model).expect("semantic parsing should succeed");

        let (register, field) = gate
            .get_field(peripheral_name, register_name, field_name)
            .expect("field should exist");

        assert_eq!(register.peripheral_path(), &peripheral_path);
        assert_eq!(register.peripheral().ident, peripheral_name);
        assert_eq!(register.register().ident, register_name);
        assert!(field.binding().is_some_and(|binding| binding.is_dynamic()));
        assert!(field.transition().is_some());
        assert_eq!(field.field().ident, field_name);
    }
}
