use std::collections::{HashMap, HashSet};

use colored::Colorize;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{parse_quote, Ident, Path};

use crate::{
    access::Access,
    structures::field::Numericity,
    utils::diagnostic::{Context, Diagnostic, Diagnostics},
};

use super::{entitlement::Entitlement, field::Field};

#[derive(Debug, Clone)]
pub struct Register {
    pub ident: Ident,
    pub offset: u32,

    pub fields: HashMap<Ident, Field>,
}

impl Register {
    pub fn new(
        ident: impl AsRef<str>,
        offset: u32,
        fields: impl IntoIterator<Item = Field>,
    ) -> Self {
        Self {
            ident: Ident::new(ident.as_ref().to_lowercase().as_str(), Span::call_site()),
            offset,
            fields: HashMap::from_iter(
                fields.into_iter().map(|field| (field.ident.clone(), field)),
            ),
        }
    }

    #[expect(unused)]
    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        todo!()
    }

    pub fn module_name(&self) -> Ident {
        self.ident.clone()
    }

    /// A register is resolvable if at least one field within it is resolvable.
    pub fn is_resolvable(&self) -> bool {
        self.fields.values().any(|field| field.is_resolvable())
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = context.clone().and(self.ident.clone().to_string());

        if self.offset % 4 != 0 {
            diagnostics.push(
                Diagnostic::error(format!(
                    "register offset must be word aligned. (offset {} does not satisfy: offset % 4 == 0)",
                    self.offset
                ))
                    .with_context(new_context.clone()),
            );
        }

        let mut fields = self.fields.values().collect::<Vec<_>>();
        fields.sort_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset));

        for window in fields.windows(2) {
            let lhs = window[0];
            let rhs = window[1];

            if lhs.offset + lhs.width > rhs.offset {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "fields [{}] and [{}] overlap.",
                        lhs.ident.to_string().bold(),
                        rhs.ident.to_string().bold()
                    ))
                    .with_context(new_context.clone()),
                );
            }
        }

        if let Some(field) = fields.last() {
            if field.offset + field.width > 32 {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "field [{}] exceeds register width.",
                        field.ident.to_string().bold()
                    ))
                    .with_context(new_context.clone()),
                );
            }
        }

        for field in fields {
            diagnostics.extend(field.validate(&new_context));
        }

        diagnostics
    }
}

// codegen
impl Register {
    fn generate_fields<'a>(fields: impl Iterator<Item = &'a Field>) -> TokenStream {
        quote! {
            #(
                #fields
            )*
        }
    }

    fn generate_layout_consts(offset: u32) -> TokenStream {
        quote! {
            pub const OFFSET: usize = #offset as _;
        }
    }

    fn generate_refined_writers<'a>(
        fields: impl Iterator<Item = &'a Field>,
        writer_idents: impl Iterator<Item = &'a Ident>,
    ) -> TokenStream {
        let mut enumerated_field_idents = Vec::new();
        let mut nested_variants = Vec::new();

        for field in fields {
            match &field.access {
                Access::Write(write) | Access::ReadWrite { read: _, write } => {
                    match &write.numericity {
                        Numericity::Enumerated { variants } => {
                            enumerated_field_idents.push(&field.ident);
                            nested_variants.push(variants.values().collect::<Vec<_>>());
                        }
                        Numericity::Numeric => todo!(),
                    }
                }
                _ => {}
            }
        }

        let variant_tys = nested_variants
            .iter()
            .map(|variants| {
                variants
                    .iter()
                    .map(|variant| variant.type_name())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let accessors = nested_variants
            .iter()
            .map(|variants| {
                variants
                    .iter()
                    .map(|variant| variant.module_name())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        quote! {
            #(
                pub struct #writer_idents<'a, W, F>
                where
                    F: FnOnce(&mut W, u32),
                {
                    w: &'a mut W,
                    f: F,
                }

                impl<'a, W, F> #writer_idents<'a, W, F>
                where
                    F: FnOnce(&mut W, u32),
                {
                    pub unsafe fn bits(self, bits: u32) -> &'a mut W {
                        (self.f)(self.w, bits);

                        self.w
                    }

                    pub fn variant(self, variant: #enumerated_field_idents::WriteVariant) -> &'a mut W {
                        unsafe { self.bits(variant as _) }
                    }

                    #(
                        pub fn #accessors(self) -> &'a mut W {
                            self.variant(#enumerated_field_idents::WriteVariant::#variant_tys)
                        }
                    )*
                }
            )*
        }
    }

    fn generate_unsafe_interface<'a>(
        fields: impl Iterator<Item = &'a Field> + Clone,
        refined_writer_idents: impl Iterator<Item = &'a Ident>,
    ) -> TokenStream {
        fn read<'a>(fields: impl Iterator<Item = &'a Field> + Clone) -> Option<TokenStream> {
            if fields.clone().any(|field| field.access.is_read()) {
                let enumerated_field_idents = fields.filter_map(|field| match &field.access {
                    Access::Read(read) | Access::ReadWrite { read, write: _ } => {
                        if matches!(read.numericity, Numericity::Enumerated { variants: _ }) {
                            Some(&field.ident)
                        } else {
                            None
                        }
                    }
                    _ => None,
                });

                Some(quote! {
                    pub struct UnsafeReader {
                        value: u32
                    }

                    impl UnsafeReader {
                        /// View the raw bits captured from the register.
                        pub fn bits(&self) -> u32 {
                            self.value
                        }

                        #(
                            pub fn #enumerated_field_idents(&self) -> #enumerated_field_idents::ReadVariant {
                                unsafe {
                                    #enumerated_field_idents::ReadVariant::from_bits({
                                        let mask = u32::MAX >> (32 - #enumerated_field_idents::WIDTH);
                                        (self.value >> #enumerated_field_idents::OFFSET) & mask
                                    })
                                }
                            }
                        )*
                    }

                    /// Read the contents of the register, ignoring any implicative effects.
                    ///
                    /// # Safety
                    ///
                    /// Invoking this function will render statically tracked operations unsound if the operation's
                    /// invariances are violated by the effects of the invocation.
                    pub unsafe fn read_untracked() -> UnsafeReader {
                        UnsafeReader {
                            value: unsafe { ::core::ptr::read_volatile((super::base_addr() + OFFSET) as *const u32) }
                        }
                    }
                })
            } else {
                None
            }
        }

        fn write<'a>(
            fields: impl Iterator<Item = &'a Field> + Clone,
            refined_writer_idents: impl Iterator<Item = &'a Ident>,
        ) -> Option<TokenStream> {
            if fields.clone().any(|field| field.access.is_write()) {
                let enumerated_field_idents = fields.filter_map(|field| match &field.access {
                    Access::Write(write) | Access::ReadWrite { read: _, write } => {
                        if matches!(write.numericity, Numericity::Enumerated { variants: _ }) {
                            Some(&field.ident)
                        } else {
                            None
                        }
                    }
                    _ => None,
                });

                Some(quote! {
                    pub struct UnsafeWriter {
                        value: u32
                    }

                    impl UnsafeWriter {
                        /// View the raw bits pending to be written to the register.
                        pub fn bits(&self) -> u32 {
                            self.value
                        }

                        #(
                            pub fn #enumerated_field_idents(&mut self) -> #refined_writer_idents<Self, impl FnOnce(&mut Self, u32)> {
                                let mask = (u32::MAX >> (32 - #enumerated_field_idents::WIDTH)) << #enumerated_field_idents::OFFSET;

                                #refined_writer_idents { w: self, f: move |w, value| w.value = (w.value & !mask) | (value << #enumerated_field_idents::OFFSET) }
                            }
                        )*
                    }

                    /// Write to fields of the register with a default value of 0, ignoring any implicative effects.
                    ///
                    /// # Safety
                    ///
                    /// Invoking this function will render statically tracked operations unsound if the operation's
                    /// invariances are violated by the effects of the invocation.
                    pub unsafe fn write_from_zero_untracked(f: impl FnOnce(&mut UnsafeWriter) -> &mut UnsafeWriter) {
                        let mut writer = UnsafeWriter { value: 0 };

                        f(&mut writer);

                        unsafe { ::core::ptr::write_volatile((super::base_addr() + OFFSET) as *mut u32, writer.value) };
                    }

                    /// Write to fields of the register with a default hardware reset value, ignoring any implicative
                    /// effects.
                    ///
                    /// # Safety
                    ///
                    /// Invoking this function will render statically tracked operations unsound if the operation's
                    /// invariances are violated by the effects of the invocation.
                    pub unsafe fn write_from_reset_untracked(f: impl FnOnce(&mut UnsafeWriter) -> &mut UnsafeWriter) {
                        let mut writer = UnsafeWriter { value: 0 };

                        unsafe { ResetTransitionBuilder::new().finish(&mut writer) };

                        f(&mut writer);

                        unsafe { ::core::ptr::write_volatile((super::base_addr() + OFFSET) as *mut u32, writer.value) };
                    }

                    /// Read the contents of a register for modification which can be written back, ignoring implicative
                    /// effects.
                    ///
                    /// # Safety
                    ///
                    /// Invoking this function will render statically tracked operations unsound if the operation's
                    /// invariances are violated by the effects of the invocation.
                    pub unsafe fn modify_untracked(f: impl FnOnce(UnsafeReader, &mut UnsafeWriter) -> &mut UnsafeWriter) {
                        let reader = unsafe { read_untracked() };
                        let mut writer = UnsafeWriter { value: reader.value };

                        f(reader, &mut writer);

                        unsafe { ::core::ptr::write_volatile((super::base_addr() + OFFSET) as *mut u32, writer.value) };
                    }
                })
            } else {
                None
            }
        }

        let read = read(fields.clone());
        let write = write(fields, refined_writer_idents);

        quote! {
            #read
            #write
        }
    }

    fn generate_reset<'a>(fields: impl Iterator<Item = &'a Field>) -> TokenStream {
        let field_idents = fields
            .filter_map(|field| field.reset.as_ref().and(Some(field.module_name())))
            .collect::<Vec<_>>();

        quote! {
            pub struct Reset {
                #(
                    pub #field_idents: #field_idents::Reset,
                )*
            }

            impl Reset {
                pub unsafe fn conjure() -> Self {
                    #[allow(unsafe_op_in_unsafe_fn)]
                    Self {
                        #(
                            #field_idents: <#field_idents::Reset as ::proto_hal::stasis::PartialState<UnsafeWriter>>::conjure(),
                        )*
                    }
                }
            }
        }
    }

    fn generate_states_struct<'a>(fields: impl Iterator<Item = &'a Field> + Clone) -> TokenStream {
        let field_idents = fields
            .clone()
            .map(|field| field.module_name())
            .collect::<Vec<_>>();
        let states = fields.map(|field| field.type_name()).collect::<Vec<_>>();

        quote! {
            pub struct States<#(#states,)*>
            where
                #(
                    #states: ::proto_hal::stasis::PartialState<UnsafeWriter>,
                )*
            {
                #(
                    pub #field_idents: #states,
                )*
            }

            impl<#(#states,)*> States<#(#states,)*>
            where
                #(
                    #states: ::proto_hal::stasis::PartialState<UnsafeWriter>,
                )*
            {
                pub unsafe fn conjure() -> Self {
                    unsafe {
                        Self {
                            #(
                                #field_idents: #states::conjure(),
                            )*
                        }
                    }
                }
            }
        }
    }

    fn generate_field_transition_builders<'a>(
        fields: impl Iterator<Item = &'a Field> + Clone,
    ) -> TokenStream {
        let fields = fields.filter(|field| field.is_resolvable() && field.access.is_write());
        let field_tys = fields
            .clone()
            .map(|field| field.type_name())
            .collect::<Vec<_>>();

        let mut body = quote! {};

        for (i, field) in fields.enumerate() {
            let field_module_ident = field.module_name();
            let builder_ident = format_ident!("{}TransitionBuilder", field.type_name());

            let prev_field_tys = field_tys.get(..i).unwrap();
            let next_field_tys = field_tys.get(i + 1..).unwrap();

            let variants = match &field.access {
                Access::Read(read) | Access::ReadWrite { read, write: _ } => {
                    let Numericity::Enumerated { variants } = &read.numericity else {
                        todo!()
                    };

                    variants
                }
                _ => unreachable!(),
            };

            body.extend(quote! {
                pub struct #builder_ident<#(#field_tys,)*>
                where
                    #(
                        #field_tys: ::proto_hal::stasis::PartialState<UnsafeWriter>,
                    )*
                {
                    _p: ::core::marker::PhantomData<(#(#field_tys,)*)>,
                }

                impl<#(#field_tys,)*> #builder_ident<#(#field_tys,)*>
                where
                    #(
                        #field_tys: ::proto_hal::stasis::PartialState<UnsafeWriter>,
                    )*
                {
                    pub unsafe fn conjure() -> Self {
                        unsafe { ::core::mem::transmute(()) }
                    }

                    pub fn generic<_NewState>(self) -> TransitionBuilder<#(#prev_field_tys,)* _NewState, #(#next_field_tys,)*>
                    where
                        _NewState: #field_module_ident::State,
                    {
                        unsafe { TransitionBuilder::conjure() }
                    }

                    /// Preserve the state being added to the builder. In other words, **do not** perform a transition
                    /// on the state inhabited by the specified field.
                    ///
                    /// This is useful when entitled states must be provided to the builder but need not be
                    /// transitioned.
                    pub fn preserve(self) -> TransitionBuilder<#(#field_tys,)*> {
                        unsafe { TransitionBuilder::conjure() }
                    }
                }
            });

            let mut body2 = quote! {};

            for (ty, accessor) in variants
                .values()
                .map(|variant| (variant.type_name(), variant.module_name()))
            {
                body2.extend(quote! {
                    pub fn #accessor(self) -> TransitionBuilder<#(#prev_field_tys,)* #field_module_ident::#ty, #(#next_field_tys,)*> {
                        self.generic()
                    }
                });
            }

            body.extend(quote! {
                impl<#(#field_tys,)*> #builder_ident<#(#field_tys,)*>
                where
                    #(
                        #field_tys: ::proto_hal::stasis::PartialState<UnsafeWriter>,
                    )*
                {
                    #body2
                }
            });
        }

        body
    }

    fn generate_transition_builder<'a>(
        fields: impl Iterator<Item = &'a Field> + Clone,
        entitlement_bounds: impl Iterator<Item = &'a TokenStream>,
    ) -> TokenStream {
        let resolvable_fields = fields
            .filter_map(|field| {
                if field.is_resolvable() {
                    Some(field)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let field_module_idents = resolvable_fields.iter().map(|field| field.module_name());
        let states = resolvable_fields
            .iter()
            .map(|field| field.type_name())
            .collect::<Vec<_>>();

        let unresolved = states.iter().map(|_| {
            let path: Path = parse_quote! {
                ::proto_hal::stasis::Unresolved
            };
            path
        });

        let mut methods = quote! {};

        for (i, field) in resolvable_fields.iter().enumerate() {
            let field_module_ident = field.module_name();
            let builder_ident = format_ident!("{}TransitionBuilder", field.type_name());

            let prev_states = states.get(..i).unwrap();
            let next_states = states.get(i + 1..).unwrap();

            methods.extend(quote! {
                pub fn #field_module_ident<_OldState>(self, #[expect(unused_variables)] state: _OldState) -> #builder_ident<#(#prev_states,)* _OldState, #(#next_states,)*>
                where
                    _OldState: #field_module_ident::State,
                {
                    unsafe { #builder_ident::conjure() }
                }
            })
        }

        quote! {
            pub struct TransitionBuilder<#(#states,)*>
            where
                #(
                    #states: ::proto_hal::stasis::PartialState<UnsafeWriter>,
                )*
            {
                _p: core::marker::PhantomData<(#(#states,)*)>,
            }

            type EmptyTransitionBuilder = TransitionBuilder<#(#unresolved,)*>;
            type ResetTransitionBuilder = TransitionBuilder<
                #(
                    #field_module_idents::Reset,
                )*
            >;

            impl ResetTransitionBuilder {
                unsafe fn new() -> Self {
                    Self {
                        _p: ::core::marker::PhantomData,
                    }
                }
            }

            impl EmptyTransitionBuilder {
                fn new() -> Self {
                    Self {
                        _p: ::core::marker::PhantomData,
                    }
                }
            }

            impl<#(#states,)*> TransitionBuilder<#(#states,)*>
            where
                #(
                    #states: ::proto_hal::stasis::PartialState<UnsafeWriter>,
                )*
            {
                pub unsafe fn conjure() -> Self {
                    unsafe { ::core::mem::transmute(()) }
                }

                fn finish(self, w: &mut UnsafeWriter)
                where
                    #(
                        #entitlement_bounds,
                    )*
                {
                    #(
                        #states::set(w);
                    )*;
                }

                #methods
            }
        }
    }

    fn create_entitlement_bounds<'a>(fields: impl Iterator<Item = &'a Field>) -> Vec<TokenStream> {
        fields
            .filter_map(|field| match &field.access {
                Access::Read(read) | Access::ReadWrite { read, write: _ } => {
                    let Numericity::Enumerated { variants } = &read.numericity else {
                        unreachable!()
                    };

                    let field_ty = field.type_name();
                    let entitled_fields = variants
                        .values()
                        .flat_map(|variant| {
                            variant.entitlements.iter().map(|entitlement| {
                                Ident::new(
                                    inflector::cases::pascalcase::to_pascal_case(
                                        entitlement.field().to_string().as_str(),
                                    )
                                    .as_str(),
                                    Span::call_site(),
                                )
                            })
                        })
                        .collect::<HashSet<_>>()
                        .into_iter();

                    Some(quote! {
                        #field_ty: #(::proto_hal::stasis::Entitled<#entitled_fields>)+*
                    })
                }
                _ => unreachable!(),
            })
            .collect()
    }

    fn generate_transition_gate<'a>(
        fields: impl Iterator<Item = &'a Field>,
        entitlement_bounds: impl Iterator<Item = &'a TokenStream>,
    ) -> TokenStream {
        let new_states = fields
            .filter_map(|field| {
                if field.is_resolvable() {
                    Some(field.type_name())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        quote! {
            pub fn transition<#(#new_states,)*>(f: impl FnOnce(EmptyTransitionBuilder) -> TransitionBuilder<#(#new_states,)*>) -> States<#(#new_states,)*>
            where
                #(
                    #new_states: ::proto_hal::stasis::PartialState<UnsafeWriter>,
                )*
                #(
                    #entitlement_bounds,
                )*
            {
                unsafe { modify_untracked(|_, w| { f(EmptyTransitionBuilder::new()).finish(w); w }) };

                unsafe { States::conjure() }
            }
        }
    }
}

impl ToTokens for Register {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut body = quote! {};

        let module_name = self.module_name();

        let refined_writer_idents = self
            .fields
            .values()
            .map(|field| format_ident!("{}Writer", field.type_name()))
            .collect::<Vec<_>>();

        body.extend(Self::generate_fields(self.fields.values()));
        body.extend(Self::generate_layout_consts(self.offset));
        body.extend(Self::generate_refined_writers(
            self.fields.values(),
            refined_writer_idents.iter(),
        ));
        body.extend(Self::generate_unsafe_interface(
            self.fields.values(),
            refined_writer_idents.iter(),
        ));
        if self.is_resolvable() {
            body.extend(Self::generate_reset(self.fields.values()));
            body.extend(Self::generate_states_struct(self.fields.values()));
            body.extend(Self::generate_field_transition_builders(
                self.fields.values(),
            ));

            let entitlement_bounds = Self::create_entitlement_bounds(self.fields.values());

            body.extend(Self::generate_transition_builder(
                self.fields.values(),
                entitlement_bounds.iter(),
            ));
            body.extend(Self::generate_transition_gate(
                self.fields.values(),
                entitlement_bounds.iter(),
            ));
        }

        tokens.extend(quote! {
            pub mod #module_name {
                #body
            }
        });
    }
}
