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
    pub docs: Vec<String>,
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
                fields.into_iter().map(|field| (field.module_name(), field)),
            ),
            docs: Vec::new(),
        }
    }

    #[expect(unused)]
    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        todo!()
    }

    pub fn docs<I>(mut self, docs: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.docs
            .extend(docs.into_iter().map(|doc| doc.as_ref().to_string()));

        self
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
        let new_context = context.clone().and(self.module_name().to_string());

        if self.offset % 4 != 0 {
            diagnostics.insert(
                Diagnostic::error(format!(
                    "register offset must be word aligned. (offset {} does not satisfy: offset % 4 == 0)",
                    self.offset
                ))
                    .with_context(new_context.clone()),
            );
        }

        let mut fields = self.fields.values().collect::<Vec<_>>();
        fields.sort_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset));

        for (i, field) in fields.iter().enumerate() {
            let remaining = &fields[i + 1..];

            for other in remaining {
                if field.offset + field.width <= other.offset {
                    break;
                }

                // unfortunate workaround for `is_disjoint` behavior when sets are empty
                if !field.entitlements.is_empty()
                    && !other.entitlements.is_empty()
                    && field.entitlements.is_disjoint(&other.entitlements)
                {
                    continue;
                }

                diagnostics.insert(
                    Diagnostic::error(format!(
                        "fields [{}] and [{}] overlap.",
                        field.module_name().to_string().bold(),
                        other.module_name().to_string().bold()
                    ))
                    .with_context(new_context.clone())
                    .notes(
                        if !field.entitlements.is_empty() || !other.entitlements.is_empty() {
                            vec![format!(
                                "overlapping fields have non-trivial intersecting entitlement spaces {:?} and {:?}",
                                field.entitlements.iter().map(|entitlement| entitlement.to_string()).collect::<Vec<_>>(),
                                other.entitlements.iter().map(|entitlement| entitlement.to_string()).collect::<Vec<_>>(),
                            )]
                        } else {
                            vec![]
                        },
                    ),
                );
            }
        }

        if let Some(field) = fields.last() {
            if field.offset + field.width > 32 {
                diagnostics.insert(
                    Diagnostic::error(format!(
                        "field [{}] exceeds register width.",
                        field.module_name().to_string().bold()
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

    fn generate_refined_writers<'a>(fields: impl Iterator<Item = &'a Field>) -> TokenStream {
        let mut writers = quote! {};

        for field in fields {
            if let Access::Write(write) | Access::ReadWrite { read: _, write } = &field.access {
                let Numericity::Enumerated { variants } = &write.numericity else {
                    continue;
                };

                let field_ident = field.module_name();
                let writer_ident = field.writer_ident();

                let variant_tys = variants
                    .values()
                    .map(|variant| variant.type_name())
                    .collect::<Vec<_>>();

                let accessors = variants
                    .values()
                    .map(|variant| variant.module_name())
                    .collect::<Vec<_>>();

                writers.extend(quote! {
                    #[doc(hidden)]
                    pub struct #writer_ident<'a, W, F>
                    where
                        F: FnOnce(&mut W, u32),
                    {
                        w: &'a mut W,
                        f: F,
                    }

                    impl<'a, W, F> #writer_ident<'a, W, F>
                    where
                        F: FnOnce(&mut W, u32),
                    {
                        /// # Safety
                        /// If the usage of this function violates any invariances of the
                        /// corresponding field, any logic dependent on that field will
                        /// be rendered unsound.
                        pub unsafe fn bits(self, bits: u32) -> &'a mut W {
                            (self.f)(self.w, bits);

                            self.w
                        }

                        pub fn variant(self, variant: #field_ident::WriteVariant) -> &'a mut W {
                            unsafe { self.bits(variant as _) }
                        }

                        #(
                            pub fn #accessors(self) -> &'a mut W {
                                self.variant(#field_ident::WriteVariant::#variant_tys)
                            }
                        )*
                    }
                });
            };
        }

        writers
    }

    fn generate_unsafe_interface<'a>(
        fields: impl Iterator<Item = &'a Field> + Clone,
    ) -> TokenStream {
        fn read<'a>(fields: impl Iterator<Item = &'a Field> + Clone) -> Option<TokenStream> {
            if fields.clone().any(|field| field.access.is_read()) {
                let enumerated_field_idents =
                    fields.clone().filter_map(|field| match &field.access {
                        Access::Read(read) | Access::ReadWrite { read, write: _ } => {
                            if matches!(read.numericity, Numericity::Enumerated { variants: _ }) {
                                Some(field.module_name())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    });

                let numeric_field_idents = fields.filter_map(|field| match &field.access {
                    Access::Read(read) | Access::ReadWrite { read, write: _ } => {
                        if matches!(read.numericity, Numericity::Numeric) {
                            Some(field.module_name())
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

                        #(
                            pub fn #numeric_field_idents(&self) -> u32 {
                                let mask = u32::MAX >> (32 - #numeric_field_idents::WIDTH);
                                (self.value >> #numeric_field_idents::OFFSET) & mask
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

        fn write<'a>(fields: impl Iterator<Item = &'a Field> + Clone) -> Option<TokenStream> {
            if fields.clone().any(|field| field.access.is_write()) {
                let enumerated_fields = fields.clone().filter(|field| match &field.access {
                    Access::Write(write) | Access::ReadWrite { read: _, write } => {
                        matches!(write.numericity, Numericity::Enumerated { variants: _ })
                    }
                    _ => false,
                });

                let numeric_field_idents = fields
                    .filter_map(|field| match &field.access {
                        Access::Write(write) | Access::ReadWrite { read: _, write } => {
                            if matches!(write.numericity, Numericity::Numeric) {
                                Some(field.module_name())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                let enumerated_field_idents = enumerated_fields
                    .clone()
                    .map(|field| field.module_name())
                    .collect::<Vec<_>>();
                let refined_writer_idents = enumerated_fields
                    .map(|field| field.writer_ident())
                    .collect::<Vec<_>>();

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

                        #(
                            pub fn #numeric_field_idents(&mut self, value: impl Into<u32>) -> &mut Self {
                                let mask = (u32::MAX >> (32 - #numeric_field_idents::WIDTH)) << #numeric_field_idents::OFFSET;
                                self.value = (self.value & !mask) | (value.into() << #numeric_field_idents::OFFSET);

                                self
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
                })
            } else {
                None
            }
        }

        fn modify<'a>(mut fields: impl Iterator<Item = &'a Field> + Clone) -> Option<TokenStream> {
            if !fields
                .clone()
                .any(|field| field.access.is_read() && field.access.is_write())
            {
                None?
            }

            let mut out = quote! {};

            if fields.any(|field| field.is_resolvable()) {
                out.extend(quote! {
                    /// Write to fields of the register with a default hardware reset value, ignoring any implicative
                    /// effects.
                    ///
                    /// # Safety
                    ///
                    /// Invoking this function will render statically tracked operations unsound if the operation's
                    /// invariances are violated by the effects of the invocation.
                    pub unsafe fn write_from_reset_untracked(f: impl FnOnce(&mut UnsafeWriter) -> &mut UnsafeWriter) {
                        unsafe {
                            write_from_zero_untracked(|w| {
                                ResetTransitionBuilder::new().finish(w);
                                f(w)
                            })
                        }
                    }
                });
            }

            out.extend(quote! {
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
            });

            Some(out)
        }

        let read = read(fields.clone());
        let write = write(fields.clone());
        let modify = modify(fields);

        quote! {
            #read
            #write
            #modify
        }
    }

    fn maybe_generate_reader<'a>(
        fields: impl Iterator<Item = &'a Field> + Clone,
    ) -> Option<TokenStream> {
        let fields = fields.filter(|field| !field.is_resolvable());

        let accessors = fields.filter_map(|field| match &field.access {
            Access::Read(read) | Access::ReadWrite { read, write: _ } => {
                let ident = field.module_name();
                let marker_ty = field.type_name();

                let (entitlement_generics, entitlement_args, entitlement_where) = if field.entitlements.is_empty() {
                    (None, None, None)
                } else {
                    let entitlement_tys = field.entitlements.iter().map(|entitlement| entitlement.variant()).collect::<Vec<_>>();
                    let entitlement_idents = field.entitlements.iter().map(|entitlement|
                        Ident::new(
                            inflector::cases::snakecase::to_snake_case(
                                entitlement.variant().to_string().as_str()).as_str(),
                            Span::call_site()
                        ));

                    (
                        Some(quote! {
                            <#(#entitlement_tys),*>
                        }),
                        Some(quote! {
                            , #(#[expect(unused)] #entitlement_idents: &#entitlement_tys),*
                        }),
                        Some(quote! {
                            where
                                #(
                                    #ident::#marker_ty: ::proto_hal::stasis::Entitled<#entitlement_tys>,
                                )*
                        })
                    )
                };

                Some(match &read.numericity {
                    Numericity::Enumerated { variants: _ } => {
                        quote! {
                            pub fn #ident #entitlement_generics (&self #entitlement_args) -> #ident::ReadVariant
                            #entitlement_where
                            {
                                self.r.#ident()
                            }
                        }
                    },
                    Numericity::Numeric => {
                        quote! {
                            pub fn #ident #entitlement_generics (&self #entitlement_args) -> u32
                            #entitlement_where
                            {
                                self.r.#ident()
                            }
                        }
                    },
                })
            }
            _ => None,
        }).collect::<Vec<_>>();

        if accessors.is_empty() {
            None?
        }

        Some(quote! {
            pub struct Reader {
                r: UnsafeReader,
            }

            impl Reader {
                #(#accessors)*
            }

            // TODO: track potential effects
            pub fn read() -> Reader {
                Reader { r: unsafe { read_untracked() } }
            }
        })
    }

    fn maybe_generate_writer<'a>(
        fields: impl Iterator<Item = &'a Field> + Clone,
    ) -> Option<TokenStream> {
        let fields = fields.filter(|field| !field.is_resolvable());

        let accessors = fields.filter_map(|field| match &field.access {
            Access::Write(write) | Access::ReadWrite { read: _, write } => {
                let ident = field.module_name();
                let marker_ty = field.type_name();

                let (entitlement_generics, entitlement_args, entitlement_where) = if field.entitlements.is_empty() {
                    (None, None, None)
                } else {
                    let entitlement_tys = field.entitlements.iter().map(|entitlement| entitlement.variant()).collect::<Vec<_>>();
                    let entitlement_idents = field.entitlements.iter().map(|entitlement|
                        Ident::new(
                            inflector::cases::snakecase::to_snake_case(
                                entitlement.variant().to_string().as_str()).as_str(),
                            Span::call_site()
                        ));

                    (
                        Some(quote! {
                            <#(#entitlement_tys),*>
                        }),
                        Some(quote! {
                            , #(#[expect(unused)] #entitlement_idents: &#entitlement_tys),*
                        }),
                        Some(quote! {
                            where
                                #(
                                    #ident::#marker_ty: ::proto_hal::stasis::Entitled<#entitlement_tys>,
                                )*
                        })
                    )
                };

                Some(match &write.numericity {
                    Numericity::Enumerated { variants: _ } => {
                        let refined_writer_ident = field.writer_ident();

                        // TODO: this should be improved, reduce duplicate code
                        quote! {
                            pub fn #ident #entitlement_generics (&mut self #entitlement_args) -> #refined_writer_ident<Self, impl FnOnce(&mut Self, u32)>
                            #entitlement_where
                            {
                                let mask = (u32::MAX >> (32 - #ident::WIDTH)) << #ident::OFFSET;

                                #refined_writer_ident { w: self, f: move |w, value| w.value = (w.value & !mask) | (value << #ident::OFFSET) }
                            }
                        }
                    },
                    Numericity::Numeric => {
                        quote! {
                            pub fn #ident #entitlement_generics (&mut self #entitlement_args, value: impl Into<u32>) -> &mut Self
                            #entitlement_where
                            {
                                let mask = (u32::MAX >> (32 - #ident::WIDTH)) << #ident::OFFSET;
                                self.value = (self.value & !mask) | (value.into() << #ident::OFFSET);

                                self
                            }
                        }
                    },
                })
            }
            _ => None,
        }).collect::<Vec<_>>();

        if accessors.is_empty() {
            None?
        }

        Some(quote! {
            pub struct Writer {
                value: u32
            }

            impl Writer {
                #(#accessors)*
            }

            pub fn write(f: impl FnOnce(&mut Writer) -> &mut Writer) {
                unsafe { write_from_zero_untracked(|w| {
                    let mut writer = Writer { value: 0 };
                    f(&mut writer);
                    w.value = writer.value;
                    w
                })};
            }
        })
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
                /// # Safety
                /// TODO: link to conjure docs.
                pub unsafe fn conjure() -> Self {
                    #[allow(unsafe_op_in_unsafe_fn)]
                    Self {
                        #(
                            #field_idents: unsafe { <#field_idents::Reset as ::proto_hal::stasis::PartialState<UnsafeWriter>>::conjure() },
                        )*
                    }
                }
            }
        }
    }

    fn generate_states_struct<'a>(fields: impl Iterator<Item = &'a Field> + Clone) -> TokenStream {
        let fields = fields.filter(|field| field.is_resolvable());
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
                /// # Safety
                /// TODO: link to conjure docs.
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
                #[allow(clippy::type_complexity)]
                #[doc(hidden)]
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
                    /// # Safety
                    /// TODO: link to conjure docs.
                    pub unsafe fn conjure() -> Self {
                        unsafe { ::core::mem::transmute(()) }
                    }

                    #[allow(clippy::type_complexity)]
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
                    #[allow(clippy::type_complexity)]
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
                    #[allow(clippy::type_complexity)]
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
            .filter(|field| field.is_resolvable())
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
                #[allow(clippy::type_complexity)]
                pub fn #field_module_ident<_OldState>(self, #[expect(unused_variables)] state: _OldState) -> #builder_ident<#(#prev_states,)* _OldState, #(#next_states,)*>
                where
                    _OldState: #field_module_ident::State,
                {
                    unsafe { #builder_ident::conjure() }
                }
            })
        }

        quote! {
            #[allow(clippy::type_complexity)]
            #[doc(hidden)]
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
                /// # Safety
                /// TODO: link to conjure docs.
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
            .filter_map(|field| {
                if !field.is_resolvable() {
                    None?
                }

                match &field.access {
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
                            .collect::<HashSet<_>>();

                        if entitled_fields.is_empty() {
                            None?
                        }

                        let entitled_fields = entitled_fields.into_iter();

                        Some(quote! {
                            #field_ty: #(::proto_hal::stasis::Entitled<#entitled_fields>)+*
                        })
                    }
                    _ => unreachable!(),
                }
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
            #[allow(clippy::type_complexity)]
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

        let mut fields = self.fields.values().collect::<Vec<_>>();
        fields.sort_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset));

        body.extend(Self::generate_fields(fields.iter().copied()));
        body.extend(Self::generate_layout_consts(self.offset));
        body.extend(Self::generate_refined_writers(fields.iter().copied()));
        body.extend(Self::generate_unsafe_interface(fields.iter().copied()));
        body.extend(Self::maybe_generate_reader(fields.iter().copied()));
        body.extend(Self::maybe_generate_writer(fields.iter().copied()));
        if self.is_resolvable() {
            body.extend(Self::generate_reset(fields.iter().copied()));
            body.extend(Self::generate_states_struct(fields.iter().copied()));
            body.extend(Self::generate_field_transition_builders(
                fields.iter().copied(),
            ));

            let entitlement_bounds = Self::create_entitlement_bounds(fields.iter().copied());

            body.extend(Self::generate_transition_builder(
                fields.iter().copied(),
                entitlement_bounds.iter(),
            ));
            body.extend(Self::generate_transition_gate(
                fields.iter().copied(),
                entitlement_bounds.iter(),
            ));
        }

        let docs = &self.docs;
        tokens.extend(quote! {
            #(#[doc = #docs])*
            pub mod #module_name {
                #body
            }
        });
    }
}
