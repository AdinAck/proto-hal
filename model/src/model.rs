use std::{collections::HashMap, marker::PhantomData};

use colored::Colorize;
use derive_more::{AsRef, Deref, DerefMut};
use indexmap::{IndexMap, IndexSet};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::Ident;
use ters::ters;

use crate::{
    Node,
    diagnostic::{self, Context, Diagnostic, Diagnostics, Rank},
    entitlement::{self, Entitlement, EntitlementIndex},
    field::{
        Field, FieldIndex, FieldNode,
        access::{self, Access},
    },
    group::{
        FieldGroupIndex, FieldGroupNode, Group, GroupNode, PeripheralGroupIndex,
        PeripheralGroupNode, RegisterGroupIndex, RegisterGroupNode,
    },
    interrupts::{Interrupt, Interrupts},
    peripheral::{PeripheralIndex, PeripheralNode},
    register::{Register, RegisterIndex, RegisterNode},
    variant::{Variant, VariantIndex, VariantNode},
};

use super::peripheral::Peripheral;

/// A model composition is the structure used to *compose* a model. A composition exposes surfaces which allow for the
/// piece-by-piece construction of a model (see [`Entry`]).
///
/// When a composition is complete, it can be consumed to produce a model, along with emitted [`Diagnostic`]s.
#[derive(Debug, Clone, Default, Deref, DerefMut)]
pub struct Composition {
    #[deref]
    #[deref_mut]
    model: Model,
    //// Diagnostics emitted during model composition.
    diagnostics: Diagnostics,
}

/// The proto-hal device model. A HAL is generated purely from this structure.
#[ters]
#[derive(Debug, Clone, Default)]
pub struct Model {
    peripherals: IndexMap<PeripheralIndex, PeripheralNode>,
    peripheral_groups: IndexMap<PeripheralGroupIndex, PeripheralGroupNode>,
    registers: Vec<RegisterNode>,
    register_groups: IndexMap<RegisterGroupIndex, RegisterGroupNode>,
    fields: Vec<FieldNode>,
    field_groups: IndexMap<FieldGroupIndex, FieldGroupNode>,
    variants: Vec<VariantNode>,

    entitlements: HashMap<EntitlementIndex, entitlement::Space>,
    reverse_statewise_entitlements: HashMap<FieldIndex, IndexSet<FieldIndex>>,
    reverse_hardware_write_entitlements: HashMap<FieldIndex, IndexSet<FieldIndex>>,

    #[get]
    interrupts: Interrupts,
}

impl Composition {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_group<'ncx>(&'ncx mut self, name: impl AsRef<str>) -> PeripheralGroupEntry<'ncx> {
        let ident = Ident::new(name.as_ref(), Span::call_site());
        let index = PeripheralGroupIndex(ident.clone());

        if self.peripheral_groups.contains_key(&index) {
            self.diagnostics
                .insert(Diagnostic::exists(&name.as_ref(), Context::new()));
        }

        self.peripheral_groups.insert(
            index.clone(),
            GroupNode {
                parent: (),
                group: Group {
                    ident: ident.clone(),
                },
                members: Default::default(),
            },
        );

        Entry {
            model: self,
            index,
            context: Context::with_path([ident.to_string()]),
            _p: PhantomData,
        }
    }

    fn add_peripheral_inner<'cx>(
        &'cx mut self,
        peripheral: Peripheral,
        group: Option<PeripheralGroupIndex>,
    ) -> PeripheralEntry<'cx> {
        let index = PeripheralIndex(peripheral.ident());
        let name = peripheral.ident().to_string();

        if self.peripherals.contains_key(&index) {
            self.diagnostics
                .insert(Diagnostic::exists(&name, Context::new()));
        }

        if let Some(group_index) = &group {
            let group = self.peripheral_groups.get_mut(group_index).unwrap();

            group.members.insert(peripheral.ident(), index.clone());
        }

        self.peripherals.insert(
            index.clone(),
            PeripheralNode {
                peripheral,
                registers: Default::default(),
                group,
            },
        );

        Entry {
            model: self,
            index,
            context: Context::with_path([name]),
            _p: PhantomData,
        }
    }

    fn add_register_inner<'ncx>(
        &'ncx mut self,
        register: Register,
        peripheral_index: PeripheralIndex,
        group: Option<RegisterGroupIndex>,
        context: Context,
    ) -> RegisterEntry<'ncx> {
        let index = RegisterIndex(self.registers.len());
        let name = register.ident().to_string();

        // update parent
        let peripheral = self.model.peripherals.get_mut(&peripheral_index).unwrap();

        if peripheral.registers.contains_key(&register.ident()) {
            self.diagnostics
                .insert(Diagnostic::exists(&name, context.clone()));
        }

        if let Some(group_index) = &group {
            let group = self.model.register_groups.get_mut(group_index).unwrap();

            group.members.insert(register.ident(), index);
        }

        peripheral.add_child_index(index, register.ident());

        // insert child
        self.model.registers.push(RegisterNode {
            parent: peripheral_index.clone(),
            register,
            fields: Default::default(),
            group,
        });

        Entry {
            model: self,
            index,
            context: context.and(name),
            _p: PhantomData,
        }
    }

    fn add_field_inner<'ncx, Meta>(
        &'ncx mut self,
        field: Field,
        access: Access,
        register_index: RegisterIndex,
        group: Option<FieldGroupIndex>,
        context: Context,
    ) -> FieldEntry<'ncx, Meta> {
        let index = FieldIndex(self.fields.len());
        let name = field.ident().to_string();

        let register = self.model.registers.get_mut(*register_index).unwrap();

        if register.fields.contains_key(&field.ident()) {
            self.diagnostics
                .insert(Diagnostic::exists(&field.ident(), context.clone()));
        }

        if let Some(group_index) = &group {
            let group = self.model.field_groups.get_mut(group_index).unwrap();

            group.members.insert(field.ident(), index);
        }

        register.add_child_index(index, field.ident());

        self.fields.push(FieldNode {
            parent: register_index,
            field,
            access,
            group,
        });

        Entry {
            model: self,
            index,

            context: context.and(name),
            _p: PhantomData,
        }
    }

    // TODO: check for duplicates now instead of later?
    /// Add the provided interrupts to the composition.
    pub fn add_interrupts(&mut self, interrupts: impl IntoIterator<Item = Interrupt>) {
        self.interrupts.extend(interrupts);
    }

    /// Manually insert a diagnostic into the composition to be emitted during validation.
    pub fn add_diagnostic(&mut self, rank: Rank, message: impl Into<String>) {
        self.diagnostics.insert(Diagnostic::new(
            rank,
            diagnostic::Kind::Custom,
            message,
            Context::new(),
        ));
    }

    /// Capture the composition and produce the resulting model.
    ///
    /// This method also produces diagnostics emitted from both:
    /// 1. the composition itself
    /// 1. post-composition validation
    pub fn finish(self) -> (Model, Diagnostics) {
        let mut diagnostics = self.diagnostics;
        diagnostics.extend(self.model.validate());

        (self.model, diagnostics)
    }

    /// Capture the composition and produce the resulting *unvalidated* model.
    pub fn release(self) -> Model {
        self.model
    }
}

impl Model {
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

    pub fn get_peripheral(&self, index: PeripheralIndex) -> View<'_, PeripheralNode> {
        self.try_get_peripheral(index).unwrap()
    }

    pub fn try_get_peripheral(&self, index: PeripheralIndex) -> Option<View<'_, PeripheralNode>> {
        Some(View {
            model: self,
            node: self.peripherals.get(&index)?,
            index,
        })
    }

    pub fn get_peripheral_group(
        &self,
        index: PeripheralGroupIndex,
    ) -> View<'_, PeripheralGroupNode> {
        self.try_get_peripheral_group(index).unwrap()
    }

    pub fn try_get_peripheral_group(
        &self,
        index: PeripheralGroupIndex,
    ) -> Option<View<'_, PeripheralGroupNode>> {
        Some(View {
            model: self,
            node: self.peripheral_groups.get(&index)?,
            index,
        })
    }

    pub fn get_register(&self, index: RegisterIndex) -> View<'_, RegisterNode> {
        View {
            model: self,
            node: &self.registers[*index],
            index,
        }
    }

    pub fn get_register_group(&self, index: RegisterGroupIndex) -> View<'_, RegisterGroupNode> {
        self.try_get_register_group(index).unwrap()
    }

    pub fn try_get_register_group(
        &self,
        index: RegisterGroupIndex,
    ) -> Option<View<'_, RegisterGroupNode>> {
        Some(View {
            model: self,
            node: self.register_groups.get(&index)?,
            index,
        })
    }

    pub fn get_field(&self, index: FieldIndex) -> View<'_, FieldNode> {
        View {
            model: self,
            node: &self.fields[*index],
            index,
        }
    }

    pub fn get_field_group(&self, index: FieldGroupIndex) -> View<'_, FieldGroupNode> {
        View {
            model: self,
            node: &self.field_groups[&index],
            index,
        }
    }

    pub fn try_get_field_group(&self, index: FieldGroupIndex) -> Option<View<'_, FieldGroupNode>> {
        Some(View {
            model: self,
            node: self.field_groups.get(&index)?,
            index,
        })
    }

    pub fn get_variant(&self, index: VariantIndex) -> View<'_, VariantNode> {
        View {
            model: self,
            node: &self.variants[*index],
            index,
        }
    }

    pub fn try_get_entitlements(
        &self,
        index: EntitlementIndex,
    ) -> Option<View<'_, entitlement::Space>> {
        let Some(node) = self.entitlements.get(&index) else {
            None?
        };

        Some(View {
            model: self,
            index,
            node,
        })
    }

    pub fn try_get_reverse_statewise_entitlements(
        &self,
        index: &FieldIndex,
    ) -> Option<&IndexSet<FieldIndex>> {
        self.reverse_statewise_entitlements.get(index)
    }

    pub fn try_get_reverse_hardware_write_entitlements(
        &self,
        index: &FieldIndex,
    ) -> Option<&IndexSet<FieldIndex>> {
        self.reverse_hardware_write_entitlements.get(index)
    }

    pub fn peripherals<'cx>(&'cx self) -> impl Iterator<Item = View<'cx, PeripheralNode>> {
        self.peripherals.iter().map(|(index, node)| View {
            model: self,
            index: index.clone(),
            node,
        })
    }

    pub fn peripheral_groups<'cx>(
        &'cx self,
    ) -> impl Iterator<Item = View<'cx, PeripheralGroupNode>> {
        self.peripheral_groups.iter().map(|(index, node)| View {
            model: self,
            index: index.clone(),
            node,
        })
    }

    pub fn register_groups<'cx>(&'cx self) -> impl Iterator<Item = View<'cx, RegisterGroupNode>> {
        self.register_groups.iter().map(|(index, node)| View {
            model: self,
            index: index.clone(),
            node,
        })
    }

    pub fn field_groups<'cx>(&'cx self) -> impl Iterator<Item = View<'cx, FieldGroupNode>> {
        self.field_groups.iter().map(|(index, node)| View {
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
        sorted_peripherals.sort_by_key(|peripheral| peripheral.base_addr);

        for window in sorted_peripherals.windows(2) {
            let lhs = &window[0];
            let rhs = &window[1];

            if lhs.base_addr + lhs.width() > rhs.base_addr {
                diagnostics.insert(Diagnostic::overlap(
                    &lhs.ident(),
                    &rhs.ident(),
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

        // TODO: check if volatile store hardware affordance refers to purely
        // states in itself
        // ensure entitlements reside within resolvable fields
        for (index, space) in &self.entitlements {
            for entitlement in space.entitlements() {
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
        let standalone = self
            .peripherals()
            .filter(|peripheral| peripheral.group.is_none())
            .fold(quote! {}, |mut acc, peripheral| {
                acc.extend(peripheral.generate());

                acc
            });

        let grouped = self.peripheral_groups().fold(quote! {}, |mut acc, group| {
            acc.extend(group.generate());

            acc
        });

        quote! {
            #standalone
            #grouped
        }
    }

    fn generate_peripherals_struct<'cx>(
        &'cx self,
        peripherals: &Vec<View<'cx, PeripheralNode>>,
    ) -> TokenStream {
        let mut fundamental_peripheral_idents = Vec::new();
        let mut fundamental_peripheral_paths = Vec::new();
        let mut conditional_peripheral_idents = Vec::new();
        let mut conditional_peripheral_paths = Vec::new();

        for peripheral in peripherals {
            if self
                .entitlements
                .contains_key(&EntitlementIndex::Peripheral(peripheral.index.clone()))
            {
                conditional_peripheral_idents.push(peripheral.ident());
                conditional_peripheral_paths.push(peripheral.path());
            } else {
                fundamental_peripheral_idents.push(peripheral.ident());
                fundamental_peripheral_paths.push(peripheral.path());
            }
        }

        quote! {
            pub struct Dynamic {
                // fundamental
                #(
                    pub #fundamental_peripheral_idents: #fundamental_peripheral_paths::Dynamic,
                )*

                // conditional
                #(
                    pub #conditional_peripheral_idents: #conditional_peripheral_paths::Masked,
                )*
            }

            pub struct Reset {
                // fundamental
                #(
                    pub #fundamental_peripheral_idents: #fundamental_peripheral_paths::Reset,
                )*

                // conditional
                #(
                    pub #conditional_peripheral_idents: #conditional_peripheral_paths::Masked,
                )*
            }

            /// Acquire the device peripherals for use. Any previous configuration
            /// will persist and may be retained or overridden.
            ///
            /// # Safety
            /// This function assumes and requires no more than one instance of
            /// the device to exist at any time.
            ///
            /// An example of satisfying this precondition is to call [`acquire`]
            /// once on a single core microcontroller running bare-metal firmware.
            pub unsafe fn acquire() -> Dynamic {
                Dynamic {
                    // fundamental
                    #(
                        #fundamental_peripheral_idents: unsafe { ::proto_hal::stasis::Conjure::conjure() },
                    )*

                    // conditional
                    #(
                        #conditional_peripheral_idents: unsafe { ::proto_hal::stasis::Conjure::conjure() },
                    )*
                }
            }

            /// Acquire the device peripherals for use, assuming the peripherals
            /// are in their respective reset configurations.
            ///
            /// # Safety
            /// This function inherits the safety precondition of [`acquire`].
            ///
            /// Additionally, assumes and requires that all peripherals
            /// are in their respective reset state according to the
            /// [device model](TODO).
            pub unsafe fn assume_reset() -> Reset {
                Reset {
                    // fundamental
                    #(
                        #fundamental_peripheral_idents: unsafe { ::proto_hal::stasis::Conjure::conjure() },
                    )*

                    // conditional
                    #(
                        #conditional_peripheral_idents: unsafe { ::proto_hal::stasis::Conjure::conjure() },
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

pub trait AddPeripheral {
    /// Add a peripheral to the parent.
    fn add_peripheral<'ncx>(&'ncx mut self, peripheral: Peripheral) -> PeripheralEntry<'ncx>;
}

pub trait AddRegister {
    /// Add a register to the parent.
    fn add_register<'ncx>(&'ncx mut self, register: Register) -> RegisterEntry<'ncx>;
}

pub trait AddField {
    /// Add a field to the parent with [`Read`](access::Read) access.
    fn add_read_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Read>;

    /// Add a field to the parent with [`Write`](access::Write) access.
    fn add_write_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Write>;

    /// Add a field to the parent with [`ReadWrite`](access::ReadWrite) access.
    fn add_read_write_field<'ncx>(
        &'ncx mut self,
        field: Field,
    ) -> FieldEntry<'ncx, access::ReadWrite>;

    /// Add a field to the parent with [`Store`](access::Store) access.
    fn add_store_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Store>;

    /// Add a field to the parent with [`VolatileStore`](access::VolatileStore) access.
    fn add_volatile_store_field<'ncx>(
        &'ncx mut self,
        field: Field,
    ) -> FieldEntry<'ncx, access::VolatileStore>;
}

pub type GroupEntry<'cx, I> = Entry<'cx, I, ()>;
pub type PeripheralEntry<'cx> = Entry<'cx, PeripheralIndex, ()>;
pub type PeripheralGroupEntry<'cx> = GroupEntry<'cx, PeripheralGroupIndex>;
pub type RegisterEntry<'cx> = Entry<'cx, RegisterIndex, ()>;
pub type RegisterGroupEntry<'cx> = GroupEntry<'cx, RegisterGroupIndex>;
pub type FieldEntry<'cx, AccessModality> = Entry<'cx, FieldIndex, AccessModality>;
pub type FieldGroupEntry<'cx> = GroupEntry<'cx, FieldGroupIndex>;
pub type VariantEntry<'cx> = Entry<'cx, VariantIndex, ()>;

#[derive(Debug)]
pub struct Entry<'cx, Index, Meta> {
    model: &'cx mut Composition,
    index: Index,
    context: Context,
    _p: PhantomData<Meta>,
}

impl AddPeripheral for Composition {
    fn add_peripheral<'ncx>(&'ncx mut self, peripheral: Peripheral) -> PeripheralEntry<'ncx> {
        self.add_peripheral_inner(peripheral, None)
    }
}

impl<'cx, Index, Meta> Entry<'cx, Index, Meta> {
    fn add_entitlement_space(
        &mut self,
        entitlements: impl IntoIterator<Item = impl IntoIterator<Item = Entitlement>>,
        index: EntitlementIndex,
    ) {
        match entitlement::Space::from_iter(self.model, entitlements) {
            Ok(space) => {
                let mut tautology_diagnostics = Diagnostics::new();
                for pattern in space
                    .patterns()
                    .filter(|pattern| pattern.is_tautology(self.model))
                {
                    let diagnostic = Diagnostic::tautological_entitlements(
                        self.model,
                        pattern,
                        self.context.clone(),
                    );
                    tautology_diagnostics.insert(diagnostic);
                }

                self.model.diagnostics.extend(tautology_diagnostics);
                self.model.entitlements.insert(index, space);
            }
            Err(entitlement::pattern::Error::Contradicts {
                pattern,
                space,
                axis,
            }) => {
                self.model
                    .diagnostics
                    .insert(Diagnostic::invalid_entitlements(
                        self.model,
                        &pattern,
                        &axis,
                        &space,
                        self.context.clone(),
                    ));
            }
            Err(entitlement::pattern::Error::StructuralContradiction) => {
                unreachable!("user-defined patterns should not be structural contradictions")
            }
        }
    }
}

impl<'cx> PeripheralEntry<'cx> {
    pub fn add_group<'ncx>(&'ncx mut self, name: impl AsRef<str>) -> RegisterGroupEntry<'ncx> {
        let ident = Ident::new(name.as_ref(), Span::call_site());
        let index = RegisterGroupIndex(ident.clone());

        if self.model.register_groups.contains_key(&index) {
            self.model
                .diagnostics
                .insert(Diagnostic::exists(&name.as_ref(), self.context.clone()));
        }

        self.model.register_groups.insert(
            index.clone(),
            GroupNode {
                parent: self.index.clone(),
                group: Group {
                    ident: ident.clone(),
                },
                members: Default::default(),
            },
        );

        Entry {
            model: self.model,
            index,
            context: self.context.clone().and(ident.to_string()),
            _p: PhantomData,
        }
    }

    /// Add [ontological entitlements](TODO) to the peripheral.
    pub fn ontological_entitlements(
        &mut self,
        entitlements: impl IntoIterator<Item = impl IntoIterator<Item = Entitlement>>,
    ) {
        self.add_entitlement_space(
            entitlements,
            EntitlementIndex::Peripheral(self.index.clone()),
        );
    }

    /// Modify the peripheral this entry pertains to.
    pub fn modify(self, f: impl FnOnce(Peripheral) -> Peripheral) -> Self {
        let node = self.model.peripherals.get_mut(&self.index).unwrap();

        node.peripheral = f(node.peripheral.clone());

        self
    }
}

impl<'cx> AddRegister for PeripheralEntry<'cx> {
    fn add_register<'ncx>(&'ncx mut self, register: Register) -> RegisterEntry<'ncx> {
        self.model
            .add_register_inner(register, self.index.clone(), None, self.context.clone())
    }
}

impl<'cx> RegisterEntry<'cx> {
    pub fn add_group<'ncx>(&'ncx mut self, name: impl AsRef<str>) -> FieldGroupEntry<'ncx> {
        let ident = Ident::new(name.as_ref(), Span::call_site());
        let index = FieldGroupIndex(ident.clone());

        if self.model.field_groups.contains_key(&index) {
            self.model
                .diagnostics
                .insert(Diagnostic::exists(&name.as_ref(), self.context.clone()));
        }

        self.model.field_groups.insert(
            index.clone(),
            GroupNode {
                parent: self.index,
                group: Group {
                    ident: ident.clone(),
                },
                members: Default::default(),
            },
        );

        Entry {
            model: self.model,
            index,
            context: self.context.clone().and(ident.to_string()),
            _p: PhantomData,
        }
    }

    pub fn docs<I>(self, docs: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.modify(|r| r.docs(docs))
    }

    /// Modify the register this entry pertains to.
    pub fn modify(self, f: impl FnOnce(Register) -> Register) -> Self {
        let node = self.model.registers.get_mut(*self.index).unwrap();

        node.register = f(node.register.clone());

        self
    }
}

impl<'cx> AddField for RegisterEntry<'cx> {
    fn add_read_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Read> {
        self.model.add_field_inner(
            field,
            Access::Read(Default::default()),
            self.index,
            None,
            self.context.clone(),
        )
    }

    fn add_write_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Write> {
        self.model.add_field_inner(
            field,
            Access::Write(Default::default()),
            self.index,
            None,
            self.context.clone(),
        )
    }

    fn add_read_write_field<'ncx>(
        &'ncx mut self,
        field: Field,
    ) -> FieldEntry<'ncx, access::ReadWrite> {
        self.model.add_field_inner(
            field,
            Access::ReadWrite(Default::default()),
            self.index,
            None,
            self.context.clone(),
        )
    }

    fn add_store_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Store> {
        self.model.add_field_inner(
            field,
            Access::Store(Default::default()),
            self.index,
            None,
            self.context.clone(),
        )
    }

    fn add_volatile_store_field<'ncx>(
        &'ncx mut self,
        field: Field,
    ) -> FieldEntry<'ncx, access::VolatileStore> {
        self.model.add_field_inner(
            field,
            Access::VolatileStore(Default::default()),
            self.index,
            None,
            self.context.clone(),
        )
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
        let name = variant.type_name().to_string();

        // insert child
        self.model.variants.push(VariantNode {
            parent: self.index,
            variant,
        });

        Entry {
            model: self.model,
            index,

            context: self.context.clone().and(name),
            _p: PhantomData,
        }
    }

    /// Add a variant to the field.
    ///
    /// If the field's access modality exposes both *read* and *write* access,
    /// this will add the variant to *both*.
    pub fn add_variant<'ncx>(&'ncx mut self, variant: Variant) -> VariantEntry<'ncx> {
        let mut diagnostics = Diagnostics::new();
        let context = self.context.clone();
        let (index, access) = self.new_index_and_get_access();

        // update parent

        access.visit_numericities(|numericity| {
            diagnostics.extend(numericity.add_child(&variant, index, context.clone()));
        });

        self.model.diagnostics.extend(diagnostics);
        self.insert_child_and_make_entry(index, variant)
    }

    /// Add [ontological entitlements](TODO) to the field.
    pub fn ontological_entitlements(
        &mut self,
        entitlements: impl IntoIterator<Item = impl IntoIterator<Item = Entitlement>>,
    ) {
        self.add_entitlement_space(entitlements, EntitlementIndex::Field(self.index));
    }

    /// Modify the field this entry pertains to.
    pub fn modify(self, f: impl FnOnce(Field) -> Field) -> Self {
        let node = self.model.fields.get_mut(*self.index).unwrap();

        node.field = f(node.field.clone());

        self
    }
}

impl<'cx> Entry<'cx, FieldIndex, access::ReadWrite> {
    /// Add a variant to the field for read access **only**.
    pub fn add_read_variant<'ncx>(&'ncx mut self, variant: Variant) -> VariantEntry<'ncx> {
        let mut diagnostics = Diagnostics::new();
        let context = self.context.clone();
        let (index, access) = self.new_index_and_get_access();

        // update parent
        diagnostics.extend(
            access
                .get_read_mut()
                .expect("expected read access")
                .add_child(&variant, index, context),
        );

        self.model.diagnostics.extend(diagnostics);
        self.insert_child_and_make_entry(index, variant)
    }

    /// Add a variant to the field for write access **only**.
    pub fn add_write_variant<'ncx>(&'ncx mut self, variant: Variant) -> VariantEntry<'ncx> {
        let mut diagnostics = Diagnostics::new();
        let context = self.context.clone();
        let (index, access) = self.new_index_and_get_access();

        // update parent
        diagnostics.extend(
            access
                .get_write_mut()
                .expect("expected write access")
                .add_child(&variant, index, context),
        );

        self.model.diagnostics.extend(diagnostics);
        self.insert_child_and_make_entry(index, variant)
    }
}

impl<'cx> Entry<'cx, FieldIndex, access::VolatileStore> {
    /// Add [hardware write access entitlements](TODO) to the field.
    pub fn hardware_write_entitlements(
        &mut self,
        entitlements: impl IntoIterator<Item = impl IntoIterator<Item = Entitlement>> + Clone,
    ) {
        self.add_entitlement_space(
            entitlements.clone(),
            EntitlementIndex::HardwareWrite(self.index),
        );

        for entitlement in entitlements.into_iter().flatten() {
            let entitlement_field_index = entitlement.field(self.model).index;

            self.model
                .reverse_hardware_write_entitlements
                .entry(entitlement_field_index)
                .or_default()
                .insert(self.index);
        }
    }
}

impl<'cx, Meta> Entry<'cx, FieldIndex, Meta>
where
    Meta: access::IsWrite,
{
    /// Add [write access entitlements](TODO) to the field.
    pub fn write_entitlements(
        &mut self,
        entitlements: impl IntoIterator<Item = impl IntoIterator<Item = Entitlement>>,
    ) {
        self.add_entitlement_space(entitlements, EntitlementIndex::Write(self.index));
    }
}

impl<'cx> Entry<'cx, VariantIndex, ()> {
    /// Produce an [entitlement](TODO) from the variant.
    pub fn make_entitlement(&self) -> Entitlement {
        Entitlement(self.index)
    }

    /// Add [statewise entitlements](TODO) to the variant.
    pub fn statewise_entitlements(
        &mut self,
        entitlements: impl IntoIterator<Item = impl IntoIterator<Item = Entitlement>> + Clone,
    ) {
        self.add_entitlement_space(entitlements.clone(), EntitlementIndex::Variant(self.index));

        for entitlement in entitlements.into_iter().flatten() {
            let parent_index = self.model.get_variant(self.index).parent;
            let entitlement_field_index = entitlement.field(self.model).index;

            self.model
                .reverse_statewise_entitlements
                .entry(entitlement_field_index)
                .or_default()
                .insert(parent_index);
        }
    }
}

// group entries

impl<'cx> AddPeripheral for PeripheralGroupEntry<'cx> {
    fn add_peripheral<'ncx>(&'ncx mut self, peripheral: Peripheral) -> PeripheralEntry<'ncx> {
        self.model
            .add_peripheral_inner(peripheral, Some(self.index.clone()))
    }
}

impl<'cx> AddRegister for RegisterGroupEntry<'cx> {
    fn add_register<'ncx>(&'ncx mut self, register: Register) -> RegisterEntry<'ncx> {
        let group = self.model.get_register_group(self.index.clone());

        self.model.add_register_inner(
            register,
            group.parent.clone(),
            Some(self.index.clone()),
            self.context.clone(),
        )
    }
}

impl<'cx> AddField for FieldGroupEntry<'cx> {
    fn add_read_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Read> {
        let group = self.model.get_field_group(self.index.clone());

        self.model.add_field_inner(
            field,
            Access::Read(Default::default()),
            group.parent,
            Some(self.index.clone()),
            self.context.clone(),
        )
    }

    fn add_write_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Write> {
        let group = self.model.get_field_group(self.index.clone());

        self.model.add_field_inner(
            field,
            Access::Write(Default::default()),
            group.parent,
            Some(self.index.clone()),
            self.context.clone(),
        )
    }

    fn add_read_write_field<'ncx>(
        &'ncx mut self,
        field: Field,
    ) -> FieldEntry<'ncx, access::ReadWrite> {
        let group = self.model.get_field_group(self.index.clone());

        self.model.add_field_inner(
            field,
            Access::ReadWrite(Default::default()),
            group.parent,
            Some(self.index.clone()),
            self.context.clone(),
        )
    }

    fn add_store_field<'ncx>(&'ncx mut self, field: Field) -> FieldEntry<'ncx, access::Store> {
        let group = self.model.get_field_group(self.index.clone());

        self.model.add_field_inner(
            field,
            Access::Store(Default::default()),
            group.parent,
            Some(self.index.clone()),
            self.context.clone(),
        )
    }

    fn add_volatile_store_field<'ncx>(
        &'ncx mut self,
        field: Field,
    ) -> FieldEntry<'ncx, access::VolatileStore> {
        let group = self.model.get_field_group(self.index.clone());

        self.model.add_field_inner(
            field,
            Access::VolatileStore(Default::default()),
            group.parent,
            Some(self.index.clone()),
            self.context.clone(),
        )
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

    pub fn ontological_entitlements(&self) -> Option<View<'cx, entitlement::Space>> {
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
    pub fn ontological_entitlements(&self) -> Option<View<'cx, entitlement::Space>> {
        self.model
            .try_get_entitlements(EntitlementIndex::Field(self.index))
    }

    pub fn write_entitlements(&self) -> Option<View<'cx, entitlement::Space>> {
        self.model
            .try_get_entitlements(EntitlementIndex::Write(self.index))
    }

    pub fn hardware_write_entitlements(&self) -> Option<View<'cx, entitlement::Space>> {
        self.model
            .try_get_entitlements(EntitlementIndex::HardwareWrite(self.index))
    }

    pub fn statewise_entitlements(&self) -> impl Iterator<Item = View<'cx, entitlement::Space>> {
        self.resolvable().into_iter().flat_map(|numericity| {
            numericity
                .variants(self.model)
                .into_iter()
                .flatten()
                .filter_map(|variant| {
                    self.model
                        .try_get_entitlements(EntitlementIndex::Variant(*variant.index()))
                })
        })
    }

    /// View the parent register and peripheral.
    pub fn parents(&self) -> (View<'cx, PeripheralNode>, View<'cx, RegisterNode>) {
        let register = self.model.get_register(self.parent);
        let peripheral = register.parent();

        (peripheral, register)
    }
}

impl<'cx> View<'cx, VariantNode> {
    pub fn statewise_entitlements(&self) -> Option<View<'cx, entitlement::Space>> {
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

impl<'cx> View<'cx, entitlement::Space> {
    /// View the fields containing these entitlements.
    pub fn entitlement_fields(&self) -> impl Iterator<Item = View<'cx, FieldNode>> + use<'cx> {
        let mut fields = IndexMap::new();

        for entitlement in self.node.entitlements() {
            let field = entitlement.field(self.model);
            fields.insert(field.index, field);
        }

        fields.into_values()
    }
}
