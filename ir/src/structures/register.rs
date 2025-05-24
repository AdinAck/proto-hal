use std::collections::HashMap;

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
            ident: Ident::new(ident.as_ref(), Span::call_site()),
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
        Ident::new(
            inflector::cases::snakecase::to_snake_case(self.ident.to_string().as_str()).as_str(),
            Span::call_site(),
        )
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

                    pub unsafe fn read() -> UnsafeReader {
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

                    pub unsafe fn write_from_zero(f: impl FnOnce(&mut UnsafeWriter) -> &mut UnsafeWriter) {
                        let mut writer = UnsafeWriter { value: 0 };

                        f(&mut writer);

                        unsafe { ::core::ptr::write_volatile((super::base_addr() + OFFSET) as *mut u32, writer.value) };
                    }

                    pub unsafe fn modify(f: impl FnOnce(UnsafeReader, &mut UnsafeWriter) -> &mut UnsafeWriter) {
                        let reader = unsafe { read() };
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
                            #field_idents: <#field_idents::Reset as #field_idents::State>::conjure(),
                        )*
                    }
                }
            }
        }
    }

    fn generate_states_struct<'a>(fields: impl Iterator<Item = &'a Field> + Clone) -> TokenStream {
        let field_idents = fields.clone().map(|field| field.module_name());
        let states = fields.map(|field| field.type_name()).collect::<Vec<_>>();

        quote! {
            pub struct States<#(#states,)*> {
                #(
                    pub #field_idents: #states,
                )*
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
            let builder_name = format_ident!("{}TransitionBuilder", field.type_name());

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
        }

        body
    }

    fn generate_transition_builder<'a>(
        fields: impl Iterator<Item = &'a Field> + Clone,
    ) -> TokenStream {
        let resolvable_fields = fields.filter_map(|field| {
            if field.is_resolvable() {
                Some(field)
            } else {
                None
            }
        });

        let states = resolvable_fields
            .clone()
            .map(|field| field.type_name())
            .collect::<Vec<_>>();

        let field_module_idents = resolvable_fields.clone().map(|field| field.module_name());

        let unresolved = states.iter().map(|_| {
            let path: Path = parse_quote! {
                ::proto_hal::stasis::Unresolved
            };
            path
        });

        quote! {
            pub struct TransitionBuilder<#(#states,)*> {
                w: UnsafeWriter,
                _p: core::marker::PhantomData<(#(#states,)*)>,
            }

            type EmptyTransitionBuilder = TransitionBuilder<#(#unresolved,)*>;

            impl EmptyTransitionBuilder {
                fn new() -> Self {
                    Self {
                        w: UnsafeWriter { value: 0 },
                        _p: core::marker::PhantomData,
                    }
                }
            }
        }
    }

    fn generate_transition_gate<'a>(fields: impl Iterator<Item = &'a Field>) -> TokenStream {
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
            pub fn transition<#(#new_states,)*>(f: impl FnOnce(EmptyTransitionBuilder) -> TransitionBuilder<#(#new_states,)*>) -> States<#(#new_states,)*> {
                f(EmptyTransitionBuilder::new()).build()
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
            body.extend(Self::generate_transition_builder(self.fields.values()));
            body.extend(Self::generate_transition_gate(self.fields.values()));
        }

        tokens.extend(quote! {
            pub mod #module_name {
                #body
            }
        });
    }
}
