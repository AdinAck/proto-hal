use std::{collections::HashMap, marker::PhantomData};

use colored::Colorize;
use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use crate::{
    diagnostic::{Context, Diagnostic, Diagnostics},
    structures::{
        entitlement::{Entitlement, EntitlementKey, Entitlements},
        field::{
            Field, FieldIndex, FieldNode,
            access::{self, Access},
        },
        interrupts::{Interrupt, Interrupts},
        peripheral::{PeripheralIndex, PeripheralNode},
        register::{Register, RegisterIndex, RegisterNode},
        variant::{Variant, VariantIndex, VariantNode},
    },
};

use super::peripheral::Peripheral;

#[derive(Debug, Clone)]
pub struct Hal {
    peripherals: IndexMap<PeripheralIndex, PeripheralNode>,
    registers: Vec<RegisterNode>,
    fields: Vec<FieldNode>,
    variants: Vec<VariantNode>,

    entitlements: HashMap<EntitlementKey, Entitlements>,

    interrupts: Interrupts,
}

impl Hal {
    pub fn new() -> Self {
        Self {
            peripherals: Default::default(),
            registers: Default::default(),
            fields: Default::default(),
            variants: Default::default(),
            entitlements: Default::default(),
            interrupts: Interrupts::empty(),
        }
    }

    pub fn add_peripheral(&mut self, peripheral: Peripheral) -> PeripheralIndex {
        let index = PeripheralIndex(peripheral.module_name());

        self.peripherals.insert(
            index.clone(),
            PeripheralNode {
                peripheral,
                registers: Default::default(),
            },
        );

        index
    }

    pub fn interrupts(mut self, interrupts: impl IntoIterator<Item = Interrupt>) -> Self {
        self.interrupts.extend(interrupts);
        self
    }

    pub fn render_raw(&self) -> String {
        self.to_token_stream().to_string()
    }

    pub fn render(&self) -> Result<String, String> {
        let content = self.to_token_stream().to_string();
        let parsed = syn::parse_file(content.as_str());

        match parsed {
            Ok(file) => Ok(prettyplease::unparse(&file)),
            Err(e) => {
                let start = e.span().start().column;
                let end = e.span().end().column;

                const PADDING: usize = 50;

                let lhs = &content[start - PADDING..start];
                let err = &content[start..end].red();
                let rhs = &content[end..end + PADDING];

                Err(format!("{}:\n{lhs}{err}{rhs}", e))
            }
        }
    }

    pub fn get_peripheral(&self, index: &PeripheralIndex) -> &PeripheralNode {
        &self.peripherals[index]
    }

    pub fn get_register(&self, index: RegisterIndex) -> &RegisterNode {
        &self.registers[*index]
    }

    pub fn get_field(&self, index: FieldIndex) -> &FieldNode {
        &self.fields[*index]
    }

    pub fn get_variant(&self, index: VariantIndex) -> &VariantNode {
        &self.variants[*index]
    }
}

impl Hal {
    pub fn validate(&self) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = Context::new();

        let mut sorted_peripherals = self.peripherals.values().collect::<Vec<_>>();
        sorted_peripherals.sort_by(|lhs, rhs| lhs.base_addr.cmp(&rhs.base_addr));

        for window in sorted_peripherals.windows(2) {
            let lhs = window[0];
            let rhs = window[1];

            if lhs.base_addr + lhs.width() > rhs.base_addr {
                diagnostics.insert(
                    Diagnostic::error(format!(
                        "peripherals [{}] and [{}] overlap.",
                        lhs.ident, rhs.ident
                    ))
                    .with_context(new_context.clone()),
                );
            }
        }

        for peripheral in self.peripherals.values() {
            diagnostics.extend(peripheral.validate(&Context::new()));
        }

        // collect all entitlements
        let mut entitlements = IndexMap::<Context, Vec<Entitlement>>::new();

        let context = Context::new();

        for peripheral in self.peripherals.values() {
            let context = context.clone().and(peripheral.module_name().to_string());

            entitlements
                .entry(context.clone())
                .or_default()
                .extend(peripheral.entitlements.clone());

            for register in peripheral.registers.values() {
                let context = context.clone().and(register.module_name().to_string());

                for field in register.fields.values() {
                    let context = context.clone().and(field.module_name().to_string());

                    entitlements
                        .entry(context.clone())
                        .or_default()
                        .extend(field.entitlements.clone());

                    let accesses = [field.access.get_read(), field.access.get_write()];

                    for access in accesses.iter().flatten() {
                        entitlements
                            .entry(context.clone())
                            .or_default()
                            .extend(access.entitlements.clone());

                        if let Numericity::Enumerated { variants } = &access.numericity {
                            for variant in variants.values() {
                                let context = context.clone().and(variant.type_name().to_string());

                                entitlements
                                    .entry(context)
                                    .or_default()
                                    .extend(variant.entitlements.clone());
                            }
                        }
                    }
                }
            }
        }

        // traverse the hal tree given the entitlement path and ensure the path exists
        for (context, entitlements) in entitlements {
            for entitlement in entitlements {
                let Some(peripheral) = self.peripherals.get(entitlement.peripheral()) else {
                    diagnostics.insert(
                        Diagnostic::error(format!(
                            "entitlement peripheral [{}] does not exist",
                            entitlement.peripheral().to_string().bold()
                        ))
                        .with_context(context.clone()),
                    );

                    continue;
                };

                let Some(register) = peripheral.registers.get(entitlement.register()) else {
                    diagnostics.insert(
                        Diagnostic::error(format!(
                            "entitlement register [{}] does not exist",
                            entitlement.register().to_string().bold()
                        ))
                        .with_context(context.clone()),
                    );

                    continue;
                };

                let Some(field) = register.fields.get(entitlement.field()) else {
                    diagnostics.insert(
                        Diagnostic::error(format!(
                            "entitlement field [{}] does not exist",
                            entitlement.field().to_string().bold()
                        ))
                        .with_context(context.clone()),
                    );

                    continue;
                };

                let Some(read) = field.resolvable() else {
                    diagnostics.insert(
                        Diagnostic::error(format!(
                            "entitlement [{}] resides within unresolvable field [{}] and as such cannot be entitled to",
                            entitlement.to_string().bold(),
                            entitlement.field().to_string().bold()
                        ))
                            .with_context(context.clone()),
                    );

                    continue;
                };

                let Numericity::Enumerated { variants } = &read.numericity else {
                    diagnostics.insert(
                        Diagnostic::error(format!("entitlement path [{}] targets numeric field which cannot be entitled to", entitlement.to_string().bold()))
                            .with_context(context.clone()),
                    );

                    continue;
                };

                let Some(_variant) = variants.get(entitlement.variant()) else {
                    diagnostics.insert(
                        Diagnostic::error(format!(
                            "entitlement variant [{}] does not exist",
                            entitlement.variant().to_string().bold()
                        ))
                        .with_context(context.clone()),
                    );

                    continue;
                };
            }
        }

        diagnostics.extend(self.interrupts.validate());

        diagnostics
    }
}

// codegen
impl Hal {
    fn generate_peripherals(&self) -> TokenStream {
        self.peripherals
            .values()
            .fold(quote! {}, |mut acc, peripheral| {
                acc.extend(peripheral.generate());

                acc
            })
    }

    fn generate_peripherals_struct<'a>(
        peripherals: impl Iterator<Item = &'a Peripheral> + Clone,
    ) -> TokenStream {
        let fundamental_peripheral_idents = peripherals
            .clone()
            .filter_map(|peripheral| {
                if peripheral.entitlements.is_empty() {
                    Some(peripheral.module_name())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let conditional_peripheral_idents = peripherals
            .filter_map(|peripheral| {
                if !peripheral.entitlements.is_empty() {
                    Some(peripheral.module_name())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        quote! {
            pub struct Peripherals {
                // fundamental
                #(
                    pub #fundamental_peripheral_idents: #fundamental_peripheral_idents::Reset,
                )*

                // conditional
                #(
                    pub #conditional_peripheral_idents: #conditional_peripheral_idents::Masked,
                )*
            }

            /// # Safety
            /// This function assumes and requires all of the following:
            /// 1. The peripherals are in the reset state.
            /// 1. The peripherals are not accessed anywhere else.
            ///
            /// These invariances can easily be achieved by limiting the call-site of this function to one place
            /// and ensuring no other binaries are running on the target.
            pub unsafe fn peripherals() -> Peripherals {
                Peripherals {
                    // fundamental
                    #(
                        #fundamental_peripheral_idents: unsafe { <#fundamental_peripheral_idents::Reset as ::proto_hal::stasis::Conjure>::conjure() },
                    )*

                    // conditional
                    #(
                        #conditional_peripheral_idents: unsafe { <#conditional_peripheral_idents::Masked as ::proto_hal::stasis::Conjure>::conjure() },
                    )*
                }
            }
        }
    }
}

impl ToTokens for Hal {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(self.generate_peripherals());
        tokens.extend(Self::generate_peripherals_struct(self.peripherals.values()));
        self.interrupts.to_tokens(tokens);
    }
}

pub struct Entry<'cx, Index, Meta> {
    model: &'cx mut Hal,
    index: Index,
    _p: PhantomData<Meta>,
}

impl<'cx> Entry<'cx, PeripheralIndex, ()> {
    /// Add a register to the peripheral.
    pub fn add_register(&'cx mut self, register: Register) -> Entry<'_, RegisterIndex, ()> {
        let index = RegisterIndex(self.model.registers.len());

        // update parent
        self.model
            .peripherals
            .get_mut(&self.index)
            .unwrap()
            .add_child_index(index.clone(), register.module_name());

        // insert child
        self.model.registers.push(RegisterNode {
            parent: self.index.clone(),
            register,
            fields: Default::default(),
        });

        Entry {
            model: self.model,
            index,
            _p: PhantomData,
        }
    }

    /// Add [presence entitlements](TODO) to the peripheral.
    pub fn presence_entitlements(
        &'cx mut self,
        entitlements: impl IntoIterator<Item = Entitlement>,
    ) {
        self.model.entitlements.insert(
            EntitlementKey::Peripheral(self.index.clone()),
            entitlements.into_iter().collect(),
        );
    }
}

impl<'cx> Entry<'cx, RegisterIndex, ()> {
    fn new_index_and_add_to_parent(&mut self, field: &Field) -> FieldIndex {
        let index = FieldIndex(self.model.fields.len());

        // update parent
        self.model
            .registers
            .get_mut(*self.index)
            .unwrap()
            .add_child_index(index.clone(), field.module_name());

        index
    }

    fn insert_child_with_access(&mut self, field: Field, access: Access) {
        self.model.fields.push(FieldNode {
            parent: self.index.clone(),
            field,
            access,
        });
    }

    fn make_child_entry<Meta>(&'cx mut self, index: FieldIndex) -> Entry<'_, FieldIndex, Meta> {
        Entry {
            model: self.model,
            index,
            _p: PhantomData,
        }
    }

    /// Add a field to the register with [`Read`](access::Read) access.
    pub fn add_read_field(&'cx mut self, field: Field) -> Entry<'_, FieldIndex, access::Read> {
        let index = self.new_index_and_add_to_parent(&field);
        self.insert_child_with_access(field, Access::Read(Default::default()));
        self.make_child_entry(index)
    }

    /// Add a field to the register with [`Write`](access::Write) access.
    pub fn add_write_field(&'cx mut self, field: Field) -> Entry<'_, FieldIndex, access::Write> {
        let index = self.new_index_and_add_to_parent(&field);
        self.insert_child_with_access(field, Access::Write(Default::default()));
        self.make_child_entry(index)
    }

    /// Add a field to the register with [`ReadWrite`](access::ReadWrite) access.
    pub fn add_read_write_field(
        &'cx mut self,
        field: Field,
    ) -> Entry<'cx, FieldIndex, access::ReadWrite> {
        let index = self.new_index_and_add_to_parent(&field);
        self.insert_child_with_access(field, Access::ReadWrite(Default::default()));
        self.make_child_entry(index)
    }

    /// Add a field to the register with [`Store`](access::Store) access.
    pub fn add_store_field(&'cx mut self, field: Field) -> Entry<'_, FieldIndex, access::Store> {
        let index = self.new_index_and_add_to_parent(&field);
        self.insert_child_with_access(field, Access::Store(Default::default()));
        self.make_child_entry(index)
    }

    /// Add a field to the register with [`VolatileStore`](access::VolatileStore) access.
    pub fn add_volatile_store_field(
        &'cx mut self,
        field: Field,
    ) -> Entry<'cx, FieldIndex, access::VolatileStore> {
        let index = self.new_index_and_add_to_parent(&field);
        self.insert_child_with_access(field, Access::VolatileStore(Default::default()));
        self.make_child_entry(index)
    }
}

impl<'cx, Meta> Entry<'cx, FieldIndex, Meta> {
    fn new_index_and_get_access(&mut self) -> (VariantIndex, &mut Access) {
        let index = VariantIndex(self.model.variants.len());

        (
            index,
            &mut self.model.fields.get_mut(*self.index).unwrap().access,
        )
    }

    fn insert_child_and_make_entry(
        &mut self,
        index: VariantIndex,
        variant: Variant,
    ) -> Entry<'_, VariantIndex, ()> {
        // insert child
        self.model.variants.push(VariantNode {
            parent: self.index.clone(),
            variant,
        });

        Entry {
            model: self.model,
            index,
            _p: PhantomData,
        }
    }

    /// Add a variant to the field.
    ///
    /// If the field's access modality exposes both *read* and *write* access,
    /// this will add the variant to *both*.
    pub fn add_variant(&'cx mut self, variant: Variant) -> Entry<'_, VariantIndex, ()> {
        let (index, access) = self.new_index_and_get_access();

        // update parent
        if let Some(numericity) = access.get_read_mut() {
            numericity.add_child(&variant, index);
        }

        if let Some(numericity) = access.get_write_mut() {
            numericity.add_child(&variant, index);
        }

        self.insert_child_and_make_entry(index, variant)
    }

    /// Add [presence entitlements](TODO) to the field.
    pub fn presence_entitlements(
        &'cx mut self,
        entitlements: impl IntoIterator<Item = Entitlement>,
    ) {
        self.model.entitlements.insert(
            EntitlementKey::Field(self.index),
            entitlements.into_iter().collect(),
        );
    }
}

impl<'cx> Entry<'cx, FieldIndex, access::ReadWrite> {
    /// Add a variant to the field for read access **only**.
    pub fn add_read_variant(&'cx mut self, variant: Variant) -> Entry<'cx, VariantIndex, ()> {
        let (index, access) = self.new_index_and_get_access();

        // update parent
        access
            .get_read_mut()
            .expect("expected read access")
            .add_child(&variant, index);

        self.insert_child_and_make_entry(index, variant)
    }

    /// Add a variant to the field for write access **only**.
    pub fn add_write_variant(&'cx mut self, variant: Variant) -> Entry<'cx, VariantIndex, ()> {
        let (index, access) = self.new_index_and_get_access();

        // update parent
        access
            .get_write_mut()
            .expect("expected write access")
            .add_child(&variant, index);

        self.insert_child_and_make_entry(index, variant)
    }
}

impl<'cx> Entry<'cx, FieldIndex, access::VolatileStore> {
    /// Add [hardware write access entitlements](TODO) to the field.
    pub fn hardware_write_entitlements(
        &'cx mut self,
        entitlements: impl IntoIterator<Item = Entitlement>,
    ) {
        self.model.entitlements.insert(
            EntitlementKey::HardwareWrite(self.index.clone()),
            entitlements.into_iter().collect(),
        );
    }
}

impl<'cx, Meta> Entry<'cx, FieldIndex, Meta>
where
    Meta: access::IsWrite,
{
    /// Add [write access entitlements](TODO) to the field.
    pub fn write_entitlements(&'cx mut self, entitlements: impl IntoIterator<Item = Entitlement>) {
        self.model.entitlements.insert(
            EntitlementKey::Write(self.index.clone()),
            entitlements.into_iter().collect(),
        );
    }
}

impl<'cx> Entry<'cx, VariantIndex, ()> {
    /// Produce an [entitlement](TODO) from the variant.
    pub fn make_entitlement(&self) -> Entitlement {
        Entitlement(self.index)
    }
}
