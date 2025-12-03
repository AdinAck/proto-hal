use std::{collections::HashMap, marker::PhantomData};

use colored::Colorize;
use derive_more::{AsMut, AsRef, Deref, DerefMut, From};
use indexmap::{IndexMap, IndexSet};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::Ident;
use ters::ters;

use crate::{
    diagnostic::{Context, Diagnostic, Diagnostics},
    structures::{
        Node,
        entitlement::{Entitlement, EntitlementIndex, Entitlements},
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

#[ters]
#[derive(Debug, Clone, Default)]
pub struct Model {
    peripherals: IndexMap<PeripheralIndex, PeripheralNode>,
    registers: Vec<RegisterNode>,
    fields: Vec<FieldNode>,
    variants: Vec<VariantNode>,

    entitlements: HashMap<EntitlementIndex, Entitlements>,

    #[get]
    interrupts: Interrupts,
}

impl Model {
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

    /// Add a peripheral to the model.
    pub fn add_peripheral<'cx>(&'cx mut self, peripheral: Peripheral) -> PeripheralEntry<'cx> {
        let index = PeripheralIndex(peripheral.module_name());

        self.peripherals.insert(
            index.clone(),
            PeripheralNode {
                peripheral,
                registers: Default::default(),
            },
        );

        Entry {
            model: self,
            index,
            _p: PhantomData,
        }
        .into()
    }

    pub fn with_interrupts(mut self, interrupts: impl IntoIterator<Item = Interrupt>) -> Self {
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

    pub fn try_get_peripheral(&self, index: PeripheralIndex) -> Option<View<'_, PeripheralNode>> {
        let Some(node) = self.peripherals.get(&index) else {
            None?
        };

        Some(View {
            model: self,
            index,
            node,
        })
    }

    pub fn get_peripheral(&self, index: PeripheralIndex) -> View<'_, PeripheralNode> {
        View {
            model: self,
            node: &self.peripherals[&index],
            index,
        }
    }

    pub fn get_register(&self, index: RegisterIndex) -> View<'_, RegisterNode> {
        View {
            model: self,
            node: &self.registers[*index],
            index,
        }
    }

    pub fn get_field(&self, index: FieldIndex) -> View<'_, FieldNode> {
        View {
            model: self,
            node: &self.fields[*index],
            index,
        }
    }

    pub fn get_variant(&self, index: VariantIndex) -> View<'_, VariantNode> {
        View {
            model: self,
            node: &self.variants[*index],
            index,
        }
    }

    pub fn try_get_entitlements(&self, index: EntitlementIndex) -> Option<View<'_, Entitlements>> {
        let Some(node) = self.entitlements.get(&index) else {
            None?
        };

        Some(View {
            model: self,
            index,
            node,
        })
    }

    pub fn peripherals<'cx>(&'cx self) -> impl Iterator<Item = View<'cx, PeripheralNode>> {
        self.peripherals.iter().map(|(index, node)| View {
            model: self,
            index: index.clone(),
            node,
        })
    }

    pub fn peripheral_count(&self) -> usize {
        self.peripherals.len()
    }

    pub fn register_count(&self) -> usize {
        self.registers.len()
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    pub fn variant_count(&self) -> usize {
        self.variants.len()
    }

    pub fn entitlement_count(&self) -> usize {
        self.entitlements.len()
    }

    pub fn interrupt_count(&self) -> usize {
        self.interrupts.len()
    }
}

impl Model {
    pub fn validate(&self) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = Context::new();

        let mut sorted_peripherals = self.peripherals().collect::<Vec<_>>();
        sorted_peripherals.sort_by(|lhs, rhs| lhs.base_addr.cmp(&rhs.base_addr));

        for window in sorted_peripherals.windows(2) {
            let lhs = &window[0];
            let rhs = &window[1];

            if lhs.base_addr + lhs.width() > rhs.base_addr {
                diagnostics.insert(Diagnostic::overlap(
                    &lhs.module_name(),
                    &rhs.module_name(),
                    &format!(
                        "0x{:08x}...0x{:08x}",
                        rhs.domain().start,
                        lhs.domain().end - 4
                    ),
                    new_context.clone(),
                ));
            }
        }

        for peripheral in &sorted_peripherals {
            diagnostics.extend(peripheral.validate(&Context::new()));
        }

        // ensure entitlements reside within resolvable fields
        for (index, entitlements) in &self.entitlements {
            for entitlement in entitlements {
                let field = entitlement.field(self);

                if !field.is_resolvable() {
                    diagnostics.insert(Diagnostic::unresolvable(
                        self,
                        entitlement,
                        &field,
                        index.into_context(self),
                    ));

                    continue;
                };
            }
        }

        diagnostics.extend(self.interrupts.validate());

        diagnostics
    }
}

// codegen
impl Model {
    fn generate_peripherals(&self) -> TokenStream {
        self.peripherals().fold(quote! {}, |mut acc, peripheral| {
            acc.extend(peripheral.generate());

            acc
        })
    }

    fn generate_peripherals_struct<'cx>(
        &'cx self,
        peripherals: &Vec<View<'cx, PeripheralNode>>,
    ) -> TokenStream {
        let mut fundamental_peripheral_idents = Vec::new();
        let mut conditional_peripheral_idents = Vec::new();

        for peripheral in peripherals {
            if self
                .entitlements
                .contains_key(&EntitlementIndex::Peripheral(peripheral.index.clone()))
            {
                conditional_peripheral_idents.push(peripheral.module_name());
            } else {
                fundamental_peripheral_idents.push(peripheral.module_name());
            }
        }

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

impl ToTokens for Model {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let peripherals = self.peripherals().collect();

        tokens.extend(self.generate_peripherals());
        tokens.extend(self.generate_peripherals_struct(&peripherals));
        self.interrupts.to_tokens(tokens);
    }
}

#[derive(Debug, Deref, DerefMut, AsRef, AsMut, From)]
pub struct PeripheralEntry<'cx>(Entry<'cx, PeripheralIndex, ()>);

#[derive(Debug, Deref, DerefMut, AsRef, AsMut, From)]
pub struct RegisterEntry<'cx>(Entry<'cx, RegisterIndex, ()>);

#[derive(Debug, Deref, DerefMut, AsRef, AsMut, From)]
pub struct FieldEntry<'cx, AccessModality>(Entry<'cx, FieldIndex, AccessModality>);

#[derive(Debug, Deref, DerefMut, AsRef, AsMut, From)]
pub struct VariantEntry<'cx>(Entry<'cx, VariantIndex, ()>);

#[derive(Debug)]
pub struct Entry<'cx, Index, Meta> {
    model: &'cx mut Model,
    index: Index,
    _p: PhantomData<Meta>,
}

impl<'cx> Entry<'cx, PeripheralIndex, ()> {
    /// Add a register to the peripheral.
    pub fn add_register<'ncx>(&'ncx mut self, register: Register) -> RegisterEntry<'ncx> {
        let index = RegisterIndex(self.model.registers.len());

        // update parent
        self.model
            .peripherals
            .get_mut(&self.index)
            .unwrap()
            .add_child_index(index, register.module_name());

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
        .into()
    }

    /// Add [ontological entitlements](TODO) to the peripheral.
    pub fn ontological_entitlements(
        &mut self,
        entitlements: impl IntoIterator<Item = Entitlement>,
    ) {
        self.model.entitlements.insert(
            EntitlementIndex::Peripheral(self.index.clone()),
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
            .add_child_index(index, field.module_name());

        index
    }

    fn insert_child_with_access(&mut self, field: Field, access: Access) {
        self.model.fields.push(FieldNode {
            parent: self.index,
            field,
            access,
        });
    }

    fn make_child_entry<'ncx, Meta>(&'ncx mut self, index: FieldIndex) -> FieldEntry<'ncx, Meta> {
        Entry {
            model: self.model,
            index,
            _p: PhantomData,
        }
        .into()
    }

    /// Add a field to the register with [`Read`](access::Read) access.
    pub fn add_read_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Read> {
        let index = self.new_index_and_add_to_parent(&field);
        self.insert_child_with_access(field, Access::Read(Default::default()));
        self.make_child_entry(index)
    }

    /// Add a field to the register with [`Write`](access::Write) access.
    pub fn add_write_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Write> {
        let index = self.new_index_and_add_to_parent(&field);
        self.insert_child_with_access(field, Access::Write(Default::default()));
        self.make_child_entry(index)
    }

    /// Add a field to the register with [`ReadWrite`](access::ReadWrite) access.
    pub fn add_read_write_field<'ncx>(
        &'ncx mut self,
        field: Field,
    ) -> FieldEntry<'ncx, access::ReadWrite> {
        let index = self.new_index_and_add_to_parent(&field);
        self.insert_child_with_access(field, Access::ReadWrite(Default::default()));
        self.make_child_entry(index)
    }

    /// Add a field to the register with [`Store`](access::Store) access.
    pub fn add_store_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Store> {
        let index = self.new_index_and_add_to_parent(&field);
        self.insert_child_with_access(field, Access::Store(Default::default()));
        self.make_child_entry(index)
    }

    /// Add a field to the register with [`VolatileStore`](access::VolatileStore) access.
    pub fn add_volatile_store_field<'ncx>(
        &'ncx mut self,
        field: Field,
    ) -> FieldEntry<'ncx, access::VolatileStore> {
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
    ) -> VariantEntry<'_> {
        // insert child
        self.model.variants.push(VariantNode {
            parent: self.index,
            variant,
        });

        Entry {
            model: self.model,
            index,
            _p: PhantomData,
        }
        .into()
    }

    /// Add a variant to the field.
    ///
    /// If the field's access modality exposes both *read* and *write* access,
    /// this will add the variant to *both*.
    pub fn add_variant<'ncx>(&'ncx mut self, variant: Variant) -> VariantEntry<'ncx> {
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

    /// Add [ontological entitlements](TODO) to the field.
    pub fn ontological_entitlements(
        &'cx mut self,
        entitlements: impl IntoIterator<Item = Entitlement>,
    ) {
        self.model.entitlements.insert(
            EntitlementIndex::Field(self.index),
            entitlements.into_iter().collect(),
        );
    }
}

impl<'cx> Entry<'cx, FieldIndex, access::ReadWrite> {
    /// Add a variant to the field for read access **only**.
    pub fn add_read_variant<'ncx>(&'ncx mut self, variant: Variant) -> VariantEntry<'ncx> {
        let (index, access) = self.new_index_and_get_access();

        // update parent
        access
            .get_read_mut()
            .expect("expected read access")
            .add_child(&variant, index);

        self.insert_child_and_make_entry(index, variant)
    }

    /// Add a variant to the field for write access **only**.
    pub fn add_write_variant<'ncx>(&'ncx mut self, variant: Variant) -> VariantEntry<'ncx> {
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
            EntitlementIndex::HardwareWrite(self.index),
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
            EntitlementIndex::Write(self.index),
            entitlements.into_iter().collect(),
        );
    }
}

impl<'cx> Entry<'cx, VariantIndex, ()> {
    /// Produce an [entitlement](TODO) from the variant.
    pub fn make_entitlement(&self) -> Entitlement {
        Entitlement(self.index)
    }

    /// Add [statewise entitlements](TODO) to the variant.
    pub fn statewise_entitlements(
        &'cx mut self,
        entitlements: impl IntoIterator<Item = Entitlement>,
    ) {
        let entitlements = entitlements.into_iter().collect::<IndexSet<Entitlement>>();

        // reverse map
        for entitlement in &entitlements {
            self.model
                .entitlements
                .entry(EntitlementIndex::Variant(entitlement.index()))
                .or_default()
                .insert(Entitlement(self.index));
        }

        if !entitlements.is_empty() {
            self.model
                .entitlements
                .insert(EntitlementIndex::Variant(self.index), entitlements);
        }
    }
}

/// A view into the device model at a single node.
#[ters]
#[derive(Debug, Clone, Deref, AsRef)]
pub struct View<'cx, N: Node> {
    pub(super) model: &'cx Model,
    #[get]
    pub(super) index: N::Index,
    #[deref]
    #[as_ref]
    node: &'cx N,
}

impl<'cx> View<'cx, PeripheralNode> {
    /// Use the model context to lookup all child registers.
    pub fn registers(&self) -> impl Iterator<Item = View<'cx, RegisterNode>> {
        self.node
            .registers
            .values()
            .map(|index| self.model.get_register(*index))
    }

    pub fn try_get_register(&self, ident: &Ident) -> Option<View<'cx, RegisterNode>> {
        let Some(index) = self.registers.get(ident) else {
            None?
        };

        Some(self.model.get_register(*index))
    }

    pub fn ontological_entitlements(&self) -> Option<View<'cx, Entitlements>> {
        self.model
            .try_get_entitlements(EntitlementIndex::Peripheral(self.index.clone()))
    }
}

impl<'cx> View<'cx, RegisterNode> {
    /// Use the model context to lookup all child fields.
    pub fn fields(&self) -> impl Iterator<Item = View<'cx, FieldNode>> {
        self.node
            .fields
            .values()
            .map(|index| self.model.get_field(*index))
    }

    /// Try to get a child field by identifier. Returns [`None`] if there is no field
    /// with the provided identifier.
    pub fn try_get_field(&self, ident: &Ident) -> Option<View<'cx, FieldNode>> {
        let Some(index) = self.fields.get(ident) else {
            None?
        };

        Some(self.model.get_field(*index))
    }

    /// View the parent peripheral.
    pub fn parent(&self) -> View<'cx, PeripheralNode> {
        self.model.get_peripheral(self.parent.clone())
    }
}

impl<'cx> View<'cx, FieldNode> {
    pub fn ontological_entitlements(&self) -> Option<View<'cx, Entitlements>> {
        self.model
            .try_get_entitlements(EntitlementIndex::Field(self.index))
    }

    pub fn write_entitlements(&self) -> Option<View<'cx, Entitlements>> {
        self.model
            .try_get_entitlements(EntitlementIndex::Write(self.index))
    }

    pub fn hardware_write_entitlements(&self) -> Option<View<'cx, Entitlements>> {
        self.model
            .try_get_entitlements(EntitlementIndex::HardwareWrite(self.index))
    }

    /// View the parent register and peripheral.
    pub fn parents(&self) -> (View<'cx, PeripheralNode>, View<'cx, RegisterNode>) {
        let register = self.model.get_register(self.parent);
        let peripheral = register.parent();

        (peripheral, register)
    }
}

impl<'cx> View<'cx, VariantNode> {
    pub fn statewise_entitlements(&self) -> Option<View<'cx, Entitlements>> {
        self.model
            .try_get_entitlements(EntitlementIndex::Variant(self.index))
    }

    /// View the parent field, register, and peripheral.
    pub fn parents(
        &self,
    ) -> (
        View<'cx, PeripheralNode>,
        View<'cx, RegisterNode>,
        View<'cx, FieldNode>,
    ) {
        let field = self.model.get_field(self.parent);
        let (peripheral, register) = field.parents();

        (peripheral, register, field)
    }
}

impl<'cx> View<'cx, Entitlements> {
    pub fn entitlements(&self) -> impl Iterator<Item = &'cx Entitlement> {
        self.node.iter()
    }

    /// View the fields containing these entitlements.
    pub fn entitlement_fields(&self) -> impl Iterator<Item = View<'cx, FieldNode>> {
        let mut fields = IndexMap::new();

        for entitlement in self.node {
            let field = entitlement.field(self.model);
            fields.insert(field.index, field);
        }

        fields.into_values()
    }
}
