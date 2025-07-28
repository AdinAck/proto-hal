use std::collections::{HashMap, HashSet};

use colored::Colorize;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{parse_quote, Ident, Path};

use crate::{
    access::{Access, ReadWrite},
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

    fn generate_refined_writers<'a>(
        fields: impl Iterator<Item = &'a Field>,
    ) -> Option<TokenStream> {
        let fields = fields
            .filter(|field| field.access.is_write())
            .collect::<Vec<_>>();

        let field_idents = fields
            .iter()
            .map(|field| field.module_name())
            .collect::<Vec<_>>();

        let field_tys = fields
            .iter()
            .map(|field| field.type_name())
            .collect::<Vec<_>>();

        // unresolvable numeric fields don't use a refined writer
        if fields.iter().all(|field| {
            !field.is_resolvable()
                && matches!(
                    field
                        .access
                        .get_write()
                        .expect("all fields should be writable by this point")
                        .numericity,
                    Numericity::Numeric
                )
        }) {
            None?
        }

        let mut writers = quote! {};

        for (i, field) in fields.iter().enumerate() {
            let field_ident = field.module_name();
            let field_ty = field.type_name();
            let refined_writer_ident = format_ident!("{}Writer", field.type_name());

            let prev_field_idents = field_idents.get(..i).unwrap();
            let next_field_idents = field_idents.get(i + 1..).unwrap();
            let prev_field_tys = field_tys.get(..i).unwrap();
            let next_field_tys = field_tys.get(i + 1..).unwrap();

            let unused = fields.iter().map(|f| {
                if f.module_name() == field.module_name()
                    && !f.is_resolvable()
                    && matches!(
                        f.access
                            .get_write()
                            .expect("field should be writable by this point")
                            .numericity,
                        Numericity::Enumerated { .. }
                    )
                {
                    Some(quote! { #[expect(unused)] })
                } else {
                    None
                }
            });

            writers.extend(quote! {
                #[allow(clippy::type_complexity)]
                #[doc(hidden)]
                pub struct #refined_writer_ident<#(#field_tys,)*>
                where
                    #(#field_tys: ::proto_hal::stasis::Position<#field_idents::Field>,)*
                {
                    #(#unused #field_idents: #field_tys,)*
                }
            });

            let mut static_accessors = quote! {};
            let mut dynamic_accessors = quote! {};

            if field.is_resolvable() {
                static_accessors.extend(quote! {
                    #[allow(clippy::type_complexity)]
                    pub fn generic<_NewState>(self) -> Writer<#(#prev_field_tys,)* _NewState, #(#next_field_tys,)*>
                    where
                        #(#prev_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                        _NewState: ::proto_hal::stasis::Incoming<#field_ident::Field> +
                        ::proto_hal::stasis::Emplace<UnsafeWriter>,
                        #(#next_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                    {
                        Writer {
                            #(#prev_field_idents: self.#prev_field_idents,)*
                            #field_ident: unsafe { _NewState::conjure() },
                            #(#next_field_idents: self.#next_field_idents,)*
                        }
                    }

                    /// Preserve the state being added to the builder. In other words, **do not** perform a transition
                    /// on the state inhabited by the specified field.
                    ///
                    /// This is useful when entitled states must be provided to the builder but need not be
                    /// transitioned.
                    #[allow(clippy::type_complexity)]
                    pub fn preserve(self) -> Writer<#(#field_tys,)*>
                    where
                        #(#prev_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                        #field_ty: ::proto_hal::stasis::Incoming<#field_ident::Field> +
                        ::proto_hal::stasis::Emplace<UnsafeWriter>,
                        #(#next_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                    {
                        Writer {
                            #(#field_idents: self.#field_idents,)*
                        }
                    }
                });
            }

            match &field
                .access
                .get_write()
                .expect("all fields should be writable by this point")
                .numericity
            {
                Numericity::Numeric => {
                    dynamic_accessors.extend(quote! {
                        #[allow(clippy::type_complexity)]
                        pub fn value(self, value: impl Into<#field_ident::Numeric>) -> Writer<#(#prev_field_tys,)* #field_ident::Numeric, #(#next_field_tys,)*>
                        where
                            #(#prev_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                            #(#next_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                        {
                            Writer {
                                #(#prev_field_idents: self.#prev_field_idents,)*
                                #field_ident: value.into(),
                                #(#next_field_idents: self.#next_field_idents,)*
                            }
                        }
                    });

                    if field.is_resolvable() {
                        static_accessors.extend(quote! {
                            pub fn value<const N: u32>(self) -> Writer<#(#prev_field_tys,)* #field_ident::Value<N>, #(#next_field_tys,)*>
                            where
                                #(#prev_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                                #field_ident::Value<N>: ::proto_hal::stasis::Emplace<UnsafeWriter>,
                                #(#next_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                            {
                                self.generic()
                            }
                        })
                    }
                }
                Numericity::Enumerated { variants } => {
                    dynamic_accessors.extend(quote! {
                        #[allow(clippy::type_complexity)]
                        pub fn variant(self, variant: #field_ident::WriteVariant) -> Writer<#(#prev_field_tys,)* #field_ident::WriteVariant, #(#next_field_tys,)*>
                        where
                            #(#prev_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                            #(#next_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                        {
                            Writer {
                                #(#prev_field_idents: self.#prev_field_idents,)*
                                #field_ident: variant,
                                #(#next_field_idents: self.#next_field_idents,)*
                            }
                        }
                    });

                    for (ty, accessor) in variants
                        .values()
                        .map(|variant| (variant.type_name(), variant.module_name()))
                    {
                        dynamic_accessors.extend(quote! {
                            #[allow(clippy::type_complexity)]
                            pub fn #accessor(self) -> Writer<#(#prev_field_tys,)* #field_ident::WriteVariant, #(#next_field_tys,)*>
                            where
                                #(#prev_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                                #(#next_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                            {
                                self.variant(#field_ident::WriteVariant::#ty)
                            }
                        });

                        if field.is_resolvable() {
                            static_accessors.extend(quote! {
                                #[allow(clippy::type_complexity)]
                                pub fn #accessor(self) -> Writer<#(#prev_field_tys,)* #field_ident::#ty, #(#next_field_tys,)*>
                                where
                                    #(#prev_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                                    #field_ident::#ty: ::proto_hal::stasis::Emplace<UnsafeWriter>,
                                    #(#next_field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter>,)*
                                {
                                    self.generic()
                                }
                            });
                        }
                    }
                }
            };

            if !dynamic_accessors.is_empty() {
                writers.extend(quote! {
                    impl<#(#prev_field_tys,)* #(#next_field_tys,)*> #refined_writer_ident<#(#prev_field_tys,)* &mut #field_ident::Dynamic, #(#next_field_tys,)*>
                    where
                        #(#prev_field_tys: ::proto_hal::stasis::Position<#prev_field_idents::Field>,)*
                        #(#next_field_tys: ::proto_hal::stasis::Position<#next_field_idents::Field>,)*
                    {
                        #dynamic_accessors
                    }
                });
            }

            if !static_accessors.is_empty() {
                writers.extend(quote! {
                    impl<#(#field_tys,)*> #refined_writer_ident<#(#field_tys,)*>
                    where
                        #(#field_tys: ::proto_hal::stasis::Position<#field_idents::Field>,)*
                        #field_ty: ::proto_hal::stasis::Outgoing<#field_ident::Field>,
                    {
                        #static_accessors
                    }
                });
            }
        }

        Some(writers)
    }

    fn generate_unsafe_interface<'a>(
        fields: impl Iterator<Item = &'a Field> + Clone,
    ) -> TokenStream {
        fn read<'a>(fields: impl Iterator<Item = &'a Field> + Clone) -> Option<TokenStream> {
            if fields.clone().any(|field| field.access.is_read()) {
                let enumerated_field_idents =
                    fields.clone().filter_map(|field| match &field.access {
                        Access::Read(read)
                        | Access::ReadWrite(
                            ReadWrite::Symmetrical(read) | ReadWrite::Asymmetrical { read, .. },
                        ) => {
                            if matches!(read.numericity, Numericity::Enumerated { variants: _ }) {
                                Some(field.module_name())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    });

                let numeric_field_idents = fields.filter_map(|field| match &field.access {
                    Access::Read(read)
                    | Access::ReadWrite(
                        ReadWrite::Symmetrical(read) | ReadWrite::Asymmetrical { read, .. },
                    ) => {
                        if matches!(read.numericity, Numericity::Numeric) {
                            Some(field.module_name())
                        } else {
                            None
                        }
                    }
                    _ => None,
                });

                Some(quote! {
                    #[derive(Clone, Copy)]
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
            if !fields.clone().any(|field| field.access.is_write()) {
                None?
            }

            let fields = fields.filter(|field| field.access.is_write());

            let enumerated_field_idents = fields
                .clone()
                .filter_map(
                    |field| match &field.access.get_write().unwrap().numericity {
                        Numericity::Enumerated { .. } => Some(field.module_name()),
                        _ => None,
                    },
                )
                .collect::<Vec<_>>();

            let numeric_field_idents = fields
                .filter_map(
                    |field| match &field.access.get_write().unwrap().numericity {
                        Numericity::Numeric => Some(field.module_name()),
                        _ => None,
                    },
                )
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

                    /// Place a direct bit value into the writer.
                    pub fn set_bits(&mut self, bits: u32) {
                        self.value = bits;
                    }

                    #(
                        pub fn #enumerated_field_idents(&mut self, variant: #enumerated_field_idents::WriteVariant) -> &mut Self {
                            let mask = (u32::MAX >> (32 - #enumerated_field_idents::WIDTH)) << #enumerated_field_idents::OFFSET;
                            self.value = (self.value & !mask) | ((variant as u32) << #enumerated_field_idents::OFFSET);

                            self
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
                                f(<ResetWriter as ::proto_hal::stasis::Conjure>::conjure().finish(w))
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
                pub unsafe fn modify_untracked(#[expect(unused)] cs: ::proto_hal::critical_section::CriticalSection<'_>, f: impl FnOnce(UnsafeReader, &mut UnsafeWriter) -> &mut UnsafeWriter) -> UnsafeReader {
                    let reader = unsafe { read_untracked() };
                    let mut writer = UnsafeWriter { value: reader.value };

                    f(reader, &mut writer);

                    unsafe { ::core::ptr::write_volatile((super::base_addr() + OFFSET) as *mut u32, writer.value) };

                    reader
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
        let accessors = fields.filter_map(|field| match &field.access {
            Access::Read(read) | Access::ReadWrite(ReadWrite::Symmetrical(read) | ReadWrite::Asymmetrical { read, .. }) => {
                let ident = field.module_name();

                Some(match &read.numericity {
                    Numericity::Enumerated { variants: _ } => {
                        quote! {
                            pub fn #ident(&self, #[expect(unused)] instance: &mut #ident::Dynamic) -> #ident::ReadVariant {
                                self.r.#ident()
                            }
                        }
                    },
                    Numericity::Numeric => {
                        quote! {
                            pub fn #ident(&self, #[expect(unused)] instance: &mut #ident::Dynamic) -> u32 {
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
        entitlement_bounds: impl Iterator<Item = &'a TokenStream>,
    ) -> Option<TokenStream> {
        let fields = fields
            .filter(|field| field.access.is_write())
            .collect::<Vec<_>>();

        if fields.is_empty() {
            None?
        }

        let entitlement_bounds = entitlement_bounds.collect::<Vec<_>>();

        let field_idents = fields
            .iter()
            .map(|field| field.module_name())
            .collect::<Vec<_>>();
        let field_tys = fields
            .iter()
            .map(|field| field.type_name())
            .collect::<Vec<_>>();

        let unresolved = fields
            .iter()
            .map(|_| {
                let path: Path = parse_quote! {
                    ::proto_hal::stasis::Unresolved
                };
                path
            })
            .collect::<Vec<_>>();

        let mut accessors = quote! {};

        for (i, field) in fields.iter().enumerate() {
            let field_ident = field.module_name();
            let refined_writer_ident = format_ident!("{}Writer", field.type_name());

            let prev_fields = fields.get(..i).unwrap();
            let next_fields = fields.get(i + 1..).unwrap();

            let overlapping = |lhs: &Field, rhs: &Field| {
                lhs.offset + lhs.width > rhs.offset && lhs.offset < rhs.offset + rhs.width
            };

            let prev_field_tys = prev_fields.iter().map(|other| {
                if overlapping(field, other) {
                    quote! { ::proto_hal::stasis::Unavailable }
                } else {
                    other.type_name().to_token_stream()
                }
            });

            let next_field_tys = next_fields.iter().map(|other| {
                if overlapping(field, other) {
                    quote! { ::proto_hal::stasis::Unavailable }
                } else {
                    other.type_name().to_token_stream()
                }
            });

            let struct_entries = prev_fields.iter().chain(next_fields).map(|other| {
                let ident = other.module_name();
                if overlapping(field, other) {
                    quote! { #ident: ::proto_hal::stasis::Unavailable }
                } else {
                    quote! { #ident: self.#ident }
                }
            });

            accessors.extend(match (
                field.is_resolvable(),
                &field
                    .access
                    .get_write()
                    .expect("fields at this point must be writable")
                    .numericity,
            ) {
                (true, _) => quote! {
                    #[allow(clippy::type_complexity)]
                    pub fn #field_ident<_OldState>(self, state: _OldState) -> #refined_writer_ident<#(#prev_field_tys,)* _OldState, #(#next_field_tys,)*>
                    where
                        _OldState: ::proto_hal::stasis::Position<#field_ident::Field>,
                    {
                        #refined_writer_ident {
                            #field_ident: state,
                            #(#struct_entries,)*
                        }
                    }
                },
                (false, Numericity::Numeric) => {
                    quote! {
                        #[allow(clippy::type_complexity)]
                        pub fn #field_ident(self, #[expect(unused)] instance: &mut #field_ident::Dynamic, value: impl Into<#field_ident::Numeric>) -> Writer<#(#prev_field_tys,)* #field_ident::Numeric, #(#next_field_tys,)*> {
                            Writer {
                                #field_ident: value.into(),
                                #(#struct_entries,)*
                            }
                        }
                    }
                },
                (false, Numericity::Enumerated { .. }) => {
                    quote! {
                        #[allow(clippy::type_complexity)]
                        pub fn #field_ident(self, instance: &mut #field_ident::Dynamic) -> #refined_writer_ident<#(#prev_field_tys,)* &mut #field_ident::Dynamic, #(#next_field_tys,)*> {
                            #refined_writer_ident {
                                #field_ident: instance,
                                #(#struct_entries,)*
                            }
                        }
                    }
                },
            });
        }

        let (inert_tys, inert_values) = fields
            .iter()
            .map(|field| {
                if let Some(inert_ident) = field.access.get_write().and_then(|write| {
                    if let Numericity::Enumerated { variants } = &write.numericity {
                        variants.values().find_map(|variant| {
                            if variant.inert {
                                Some(variant.type_name())
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                }) {
                    let field_ident = field.module_name();
                    (
                        quote! { #field_ident::WriteVariant },
                        quote! { #field_ident::WriteVariant::#inert_ident },
                    )
                } else {
                    (
                        quote! { ::proto_hal::stasis::Unresolved },
                        quote! { ::proto_hal::stasis::Unresolved },
                    )
                }
            })
            .collect::<(Vec<_>, Vec<_>)>();

        let reset_tys = fields
            .iter()
            .map(|field| {
                if field.is_resolvable() {
                    let ident = field.module_name();
                    quote! { #ident::Reset }
                } else {
                    quote! { ::proto_hal::stasis::Unresolved }
                }
            })
            .collect::<Vec<_>>();

        let mut out = quote! {
            #[allow(clippy::type_complexity)]
            #[doc(hidden)]
            pub struct Writer<#(#field_tys,)*>
            where
                #(
                    #field_tys: ::proto_hal::stasis::Position<#field_idents::Field> +
                    ::proto_hal::stasis::Emplace<UnsafeWriter>,
                )*
            {
                #(
                    #field_idents: #field_tys,
                )*
            }

            type EmptyWriter = Writer<#(#unresolved,)*>;
            type InertWriter = Writer<#(#inert_tys,)*>;
            type ResetWriter = Writer<#(#reset_tys,)*>;

            #[allow(clippy::new_without_default)]
            impl EmptyWriter {
                pub fn empty() -> Self {
                    Self {
                        #(#field_idents: #unresolved,)*
                    }
                }
            }

            #[allow(clippy::new_without_default)]
            impl InertWriter {
                pub fn inert() -> Self {
                    Self {
                        #(#field_idents: #inert_values,)*
                    }
                }
            }

            impl ::proto_hal::stasis::Conjure for ResetWriter {
                unsafe fn conjure() -> Self {
                    unsafe {
                        Self {
                            #(#field_idents: #reset_tys::conjure(),)*
                        }
                    }
                }
            }

            impl<#(#field_tys,)*> Writer<#(#field_tys,)*>
            where
                #(
                    #field_tys: ::proto_hal::stasis::Position<#field_idents::Field> +
                    ::proto_hal::stasis::Emplace<UnsafeWriter>,
                )*
            {
                #accessors

                fn finish(self, w: &mut UnsafeWriter) -> &mut UnsafeWriter
                where
                    #(
                        #entitlement_bounds,
                    )*
                {
                    #(self.#field_idents.set(w);)*

                    w
                }
            }
        };

        // gates

        let resolvable_field_tys = fields
            .iter()
            .filter_map(|field| {
                if field.is_resolvable() {
                    Some(field.type_name())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let (states_return, states_conjure) = if fields.iter().any(|field| field.is_resolvable()) {
            (
                Some(quote! { -> States<#(#resolvable_field_tys,)*> }),
                Some(quote! { unsafe { States::conjure() } }),
            )
        } else {
            (None, None)
        };

        if fields.iter().any(|field| field.access.is_read()) {
            out.extend(quote! {
                #[allow(clippy::type_complexity)]
                pub fn modify<#(#field_tys,)*>(cs: ::proto_hal::critical_section::CriticalSection<'_>, gate: impl FnOnce(Reader, EmptyWriter) -> Writer<#(#field_tys,)*>) #states_return
                where
                    #(
                        #field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter> +
                        ::proto_hal::stasis::Position<#field_idents::Field>,
                    )*
                    #(
                        #resolvable_field_tys: ::proto_hal::stasis::Conjure,
                    )*
                    #(
                        #entitlement_bounds,
                    )*
                {
                    unsafe { modify_untracked(cs, |r, w| gate(Reader { r }, Writer::empty()).finish(w)) };

                    #states_conjure
                }
            })
        }

        out.extend(quote! {
            #[allow(clippy::type_complexity)]
            pub fn write<#(#field_tys,)*>(gate: impl FnOnce(InertWriter) -> Writer<#(#field_tys,)*>) #states_return
            where
                #(
                    #field_tys: ::proto_hal::stasis::Emplace<UnsafeWriter> +
                    ::proto_hal::stasis::Position<#field_idents::Field> +
                    ::proto_hal::stasis::Corporeal,
                )*
                #(
                    #resolvable_field_tys: ::proto_hal::stasis::Conjure,
                )*
                #(
                    #entitlement_bounds,
                )*
            {
                unsafe { write_from_zero_untracked(|w| gate(Writer::inert()).finish(w)) };

                #states_conjure
            }
        });

        Some(out)
    }

    fn generate_reset<'a>(fields: impl Iterator<Item = &'a Field>) -> TokenStream {
        let field_idents = fields.map(|field| field.module_name()).collect::<Vec<_>>();

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
                            #field_idents: unsafe { <#field_idents::Reset as ::proto_hal::stasis::Conjure>::conjure() },
                        )*
                    }
                }
            }
        }
    }

    fn generate_states_struct<'a>(
        fields: impl Iterator<Item = &'a Field> + Clone,
    ) -> Option<TokenStream> {
        let fields = fields.filter(|field| field.is_resolvable());
        let field_idents = fields
            .clone()
            .map(|field| field.module_name())
            .collect::<Vec<_>>();
        let states = fields.map(|field| field.type_name()).collect::<Vec<_>>();

        if states.is_empty() {
            None?
        }

        Some(quote! {
            pub struct States<#(#states,)*>
            where
                #(
                    #states: ::proto_hal::stasis::Position<#field_idents::Field>,
                )*
            {
                #(
                    pub #field_idents: #states,
                )*
            }

            impl<#(#states,)*> States<#(#states,)*>
            where
                #(
                    #states: ::proto_hal::stasis::Position<#field_idents::Field> +
                    ::proto_hal::stasis::Conjure,
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
        })
    }

    fn create_entitlement_bounds<'a>(fields: impl Iterator<Item = &'a Field>) -> Vec<TokenStream> {
        fields
            .filter_map(|field| {
                if !field.is_resolvable() {
                    None?
                }

                match &field.access {
                    Access::Read(read)
                    | Access::ReadWrite(
                        ReadWrite::Symmetrical(read) | ReadWrite::Asymmetrical { read, .. },
                    ) => {
                        let Numericity::Enumerated { variants } = &read.numericity else {
                            // note: how could numeric fields express particular values having entitlements?
                            None?
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
}

impl ToTokens for Register {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut body = quote! {};

        let module_name = self.module_name();

        body.extend(Self::generate_fields(self.fields.values()));
        body.extend(Self::generate_layout_consts(self.offset));
        body.extend(Self::generate_unsafe_interface(self.fields.values()));
        body.extend(Self::generate_refined_writers(self.fields.values()));
        body.extend(Self::maybe_generate_reader(self.fields.values()));

        let entitlement_bounds = Self::create_entitlement_bounds(self.fields.values());

        body.extend(Self::maybe_generate_writer(
            self.fields.values(),
            entitlement_bounds.iter(),
        ));
        body.extend(Self::generate_reset(self.fields.values()));
        body.extend(Self::generate_states_struct(self.fields.values()));

        let docs = &self.docs;
        tokens.extend(quote! {
            #(#[doc = #docs])*
            pub mod #module_name {
                #body
            }
        });
    }
}
