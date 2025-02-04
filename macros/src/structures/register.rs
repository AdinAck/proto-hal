use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};

use darling::{util::SpannedValue, FromMeta};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote_spanned, ToTokens};
use syn::{parse_quote, spanned::Spanned as _, Attribute, Expr, Ident, Index, Item, Path};
use tiva::Validator;

use crate::{
    access::{Access, AccessArgs},
    utils::{
        extract_items_from, require_module, FieldOffset, RegisterOffset, Spanned,
        SynErrorCombinator, Width,
    },
};

use super::{
    field::{Field, FieldArgs, FieldSpec},
    field_array::{FieldArray, FieldArrayArgs},
    schema::{Numericity, Schema, SchemaArgs, SchemaSpec},
    Args,
};

#[derive(Debug, Clone, Default, FromMeta)]
#[darling(default)]
pub struct RegisterArgs {
    pub offset: Option<RegisterOffset>,

    #[darling(default)]
    pub auto_increment: bool,

    // field args to inherit
    pub width: Option<SpannedValue<Width>>,
    pub schema: Option<Ident>,
    pub read: Option<SpannedValue<AccessArgs>>,
    pub write: Option<SpannedValue<AccessArgs>>,
    pub reset: Option<Expr>,
}

impl Args for RegisterArgs {
    const NAME: &str = "register";
}

impl RegisterArgs {
    pub fn check_conflict_and_inherit(&self, field_args: &mut FieldArgs) -> syn::Result<()> {
        let mut errors = SynErrorCombinator::new();

        let msg = "property is inherited from register";

        if let Some(inherited_width) = &self.width {
            if let Some(width) = &field_args.width {
                errors.push(syn::Error::new(width.span(), msg));
            } else {
                field_args.width.replace(*inherited_width);
            }
        }

        if let Some(inherited_schema) = &self.schema {
            if let Some(schema) = &field_args.schema {
                errors.push(syn::Error::new(schema.span(), msg));
            } else {
                field_args.schema.replace(inherited_schema.clone());
            }
        }

        if let Some(inherited_read) = &self.read {
            if let Some(read) = &field_args.read {
                errors.push(syn::Error::new(read.span(), msg));
            } else {
                field_args.read.replace(inherited_read.clone());
            }
        }

        if let Some(inherited_write) = &self.write {
            if let Some(write) = &field_args.write {
                errors.push(syn::Error::new(write.span(), msg));
            } else {
                field_args.write.replace(inherited_write.clone());
            }
        }

        if let Some(inherited_reset) = &self.reset {
            if let Some(reset) = &field_args.reset {
                errors.push(syn::Error::new(reset.span(), msg));
            } else {
                field_args.reset.replace(inherited_reset.clone());
            }
        }

        errors.coalesce()
    }
}

#[derive(Debug)]
pub struct RegisterSpec {
    pub args: Spanned<RegisterArgs>,
    pub ident: Ident,
    pub offset: RegisterOffset,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Register {
    spec: RegisterSpec,
}

impl Deref for Register {
    type Target = RegisterSpec;

    fn deref(&self) -> &Self::Target {
        &self.spec
    }
}

impl RegisterSpec {
    pub fn parse<'a>(
        ident: Ident,
        schemas: &mut HashMap<Ident, Schema>,
        offset: RegisterOffset,
        args: Spanned<RegisterArgs>,
        items: impl Iterator<Item = &'a Item>,
    ) -> syn::Result<Self> {
        let mut errors = SynErrorCombinator::new();

        let mut register = Self {
            args: args.clone(),
            ident,
            offset,
            fields: Vec::new(),
        };

        let mut field_offset = 0 as FieldOffset;

        for item in items {
            let module = require_module(item)?;

            let get_args = || {
                Ok((
                    SchemaArgs::get(module.attrs.iter())?,
                    FieldArgs::get(module.attrs.iter())?,
                    FieldArrayArgs::get(module.attrs.iter())?,
                ))
            };

            errors.try_maybe_then(get_args(), |arg_collection| {
                // TODO: this isn't the most flexible solution
                // but it does work for now.
                // args should be dispatched procedurally.
                match arg_collection {
                    (Some(schema_args), None, None) => {
                        let schema = Schema::validate(SchemaSpec::parse(
                            module.ident.clone(),
                            schema_args,
                            extract_items_from(module)?.iter(),
                        )?)?;

                        schemas.insert(schema.ident.clone(), schema);

                        Ok(())
                    }
                    (None, Some(mut field_args), None) => {
                        args.check_conflict_and_inherit(&mut field_args)?;

                        let field = Field::validate(FieldSpec::parse(
                            module.ident.clone(),
                            field_args.offset.unwrap_or(field_offset),
                            schemas,
                            field_args,
                            extract_items_from(module)?.iter(),
                        )?)?;

                        field_offset = field.offset + field.width();
                        register.fields.push(field);

                        Ok(())
                    }
                    (None, None, Some(mut field_array_args)) => {
                        args.check_conflict_and_inherit(&mut field_array_args.field)?;

                        let field_array = FieldArray::parse(
                            module.ident.clone(),
                            field_array_args.field.offset.unwrap_or(field_offset),
                            schemas,
                            field_array_args,
                            extract_items_from(module)?.iter(),
                        )?;

                        field_offset = field_array.inherited.offset
                            + field_array.inherited.width() * field_array.count() as FieldOffset;
                        register.fields.extend(field_array.to_fields()?);

                        Ok(())
                    }
                    (None, None, None) => Err(syn::Error::new_spanned(module, "extraneous item"))?,
                    (schema_args, field_args, field_array_args) => {
                        let mut errors = SynErrorCombinator::new();

                        let msg = "only one module annotation is permitted";

                        for span in [
                            schema_args.map(|args| args.span()),
                            field_args.map(|args| args.span()),
                            field_array_args.map(|args| args.span()),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            errors.push(syn::Error::new(span, msg));
                        }

                        errors.coalesce()
                    }
                }
            });
        }

        errors.coalesce()?;

        Ok(register)
    }

    pub fn is_resolvable(&self) -> bool {
        self.fields.iter().any(|field| field.is_resolvable())
    }
}

impl Validator<RegisterSpec> for Register {
    type Error = syn::Error;

    fn validate(spec: RegisterSpec) -> Result<Self, Self::Error> {
        let mut errors = SynErrorCombinator::new();

        for field in &spec.fields {
            if field.args.offset.is_none() && !spec.args.auto_increment {
                errors.push(syn::Error::new(
                    field.args.span(),
                    "field offset must be specified. to infer offsets, use `auto_increment`",
                ));
            }
        }

        for slice in spec.fields.windows(2) {
            let lhs = slice.first().unwrap();
            let rhs = slice.last().unwrap();
            if lhs.offset + lhs.width() > rhs.offset {
                let msg = format!(
                    "{} {{ domain: {}..{} }}, {} {{ domain: {}..{} }}",
                    lhs.ident,
                    lhs.offset,
                    lhs.offset + lhs.width(),
                    rhs.ident,
                    rhs.offset,
                    rhs.offset + rhs.width(),
                );

                let mut e = syn::Error::new(
                    spec.args.span(),
                    format!("field domains overlapping or unordered. {msg}"),
                );

                e.combine(syn::Error::new(
                    lhs.ident.span(),
                    format!(
                        "field '{}' is overlapping or out of order with '{}'. {}",
                        lhs.ident, rhs.ident, msg,
                    ),
                ));

                e.combine(syn::Error::new(
                    rhs.ident.span(),
                    format!(
                        "field '{}' is overlapping or out of order with '{}'. {}",
                        rhs.ident, lhs.ident, msg,
                    ),
                ));

                errors.push(e);
            }
        }

        errors.coalesce()?;

        Ok(Self { spec })
    }
}

pub enum AccessMarker {
    Read,
    Write,
}

enum Filter {
    Resolvable,
    Unresolvable,
    Writable,
    Readable,
    Numeric(AccessMarker),
    Enumerated(AccessMarker),
}

impl Filter {
    fn retains(&self, field: &Field) -> bool {
        match self {
            Self::Resolvable => field.is_resolvable(),
            Self::Unresolvable => !field.is_resolvable(),
            Self::Writable => field.access.is_write(),
            Self::Readable => field.access.is_read(),
            // TODO: quite a lot of repeat code here...
            Self::Numeric(marker) => match marker {
                AccessMarker::Read => {
                    let schema = match &field.access {
                        Access::Read(read) | Access::ReadWrite { read, write: _ } => &read.schema,
                        _ => return false,
                    };

                    schema.numericity.is_numeric()
                }
                AccessMarker::Write => {
                    let schema = match &field.access {
                        Access::Write(write) | Access::ReadWrite { read: _, write } => {
                            &write.schema
                        }
                        _ => return false,
                    };

                    schema.numericity.is_numeric()
                }
            },
            Self::Enumerated(marker) => match marker {
                AccessMarker::Read => {
                    let schema = match &field.access {
                        Access::Read(read) | Access::ReadWrite { read, write: _ } => &read.schema,
                        _ => return false,
                    };

                    schema.numericity.is_enumerated()
                }
                AccessMarker::Write => {
                    let schema = match &field.access {
                        Access::Write(write) | Access::ReadWrite { read: _, write } => {
                            &write.schema
                        }
                        _ => return false,
                    };

                    schema.numericity.is_enumerated()
                }
            },
        }
    }
}

struct FieldIter<'a, I>
where
    I: Iterator<Item = &'a Field>,
{
    iter: I,
    filters: Vec<Filter>,
}

impl<'a, I> FieldIter<'a, I>
where
    I: Iterator<Item = &'a Field>,
{
    const fn new(iter: I) -> Self {
        Self {
            iter,
            filters: Vec::new(),
        }
    }

    fn resolvable(mut self) -> Self {
        self.filters.push(Filter::Resolvable);

        self
    }

    fn unresolvable(mut self) -> Self {
        self.filters.push(Filter::Unresolvable);

        self
    }

    fn writable(mut self) -> Self {
        self.filters.push(Filter::Writable);

        self
    }

    fn readable(mut self) -> Self {
        self.filters.push(Filter::Readable);

        self
    }

    fn numeric(mut self, access: AccessMarker) -> Self {
        self.filters.push(Filter::Numeric(access));

        self
    }

    fn enumerated(mut self, access: AccessMarker) -> Self {
        self.filters.push(Filter::Enumerated(access));

        self
    }

    fn idents(self) -> impl Iterator<Item = &'a Ident> + use<'a, I> {
        self.map(|field| &field.ident)
    }

    fn tys(self) -> impl Iterator<Item = Ident> + use<'a, I> {
        self.map(|field| {
            Ident::new(
                &inflector::cases::pascalcase::to_pascal_case(&field.ident.to_string()),
                Span::call_site(),
            )
        })
    }
}

impl<'a, I> Iterator for FieldIter<'a, I>
where
    I: Iterator<Item = &'a Field>,
{
    type Item = &'a Field;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(field) = self.iter.next() {
                if self.filters.iter().all(|filter| filter.retains(field)) {
                    break Some(field);
                } else {
                    continue;
                }
            } else {
                break None;
            }
        }
    }
}

impl Register {
    fn fields(&self) -> FieldIter<'_, impl Iterator<Item = &'_ Field>> {
        FieldIter::new(self.fields.iter())
    }

    fn generate_field_bodies(&self) -> TokenStream2 {
        let span = self.args.span();
        let field_bodies = self.fields().map(|field| quote_spanned! { span => #field });

        quote_spanned! { span =>
            #(
                #field_bodies
            )*
        }
    }

    fn generate_offset_const(&self) -> TokenStream2 {
        let span = self.args.span();
        let offset = self.offset;

        quote_spanned! { span =>
            /// The offset of this register within the block.
            pub const OFFSET: u32 = #offset;
        }
    }

    fn maybe_generate_refined_writers(&self) -> Option<TokenStream2> {
        let span = self.args.span();

        let writable_enumerated_fields = self
            .fields()
            .enumerated(AccessMarker::Write)
            .collect::<Vec<_>>();

        if writable_enumerated_fields.is_empty() {
            return None;
        }

        let refined_writer_idents = writable_enumerated_fields
            .iter()
            .map(|field| {
                format_ident!(
                    "{}Writer",
                    inflector::cases::pascalcase::to_pascal_case(&field.ident.to_string())
                )
            })
            .collect::<Vec<_>>();

        let mut body = TokenStream2::new();

        for (field, refined_writer_ident) in
            writable_enumerated_fields.iter().zip(refined_writer_idents)
        {
            let field_ident = &field.ident;

            let schema = match &field.access {
                Access::Write(write) | Access::ReadWrite { read: _, write } => &write.schema,
                _ => unreachable!("fields are writable"),
            };

            let Numericity::Enumerated { variants } = &schema.numericity else {
                unreachable!("field schemas are enumerated in write direction")
            };

            let accessors = variants.iter().map(|variant| {
                Ident::new(
                    &inflector::cases::snakecase::to_snake_case(&variant.ident.to_string()),
                    field.args.span(),
                )
            });

            let variant_idents = variants.iter().map(|variant| &variant.ident);

            body.extend(quote_spanned! { span =>
                pub struct #refined_writer_ident<'a, W> {
                    w: &'a mut W
                }

                impl<'a, W> #refined_writer_ident<'a, W>
                where
                    W: ::proto_hal::macro_utils::Writer,
                {
                    pub fn variant(self, variant: #field_ident::WriteVariant) -> &'a mut W {
                        unsafe { ::proto_hal::macro_utils::Writer::write(self.w, |reg| *reg |= (variant as u32) << #field_ident::OFFSET) }
                    }

                    #(
                        pub fn #accessors(self) -> &'a mut W {
                            self.variant(#field_ident::WriteVariant::#variant_idents)
                        }
                    )*
                }
            });
        }

        Some(body)
    }

    fn maybe_generate_reader(&self) -> Option<TokenStream2> {
        let span = self.args.span();

        let readable_unresolvable_fields =
            self.fields().readable().unresolvable().collect::<Vec<_>>();

        // don't generate a reader if there are no fields
        // to be read
        if readable_unresolvable_fields.is_empty() {
            return None;
        };

        let readable_unresolvable_numeric_fields = self
            .fields()
            .readable()
            .unresolvable()
            .numeric(AccessMarker::Read);
        let readable_unresolvable_numeric_field_idents = self
            .fields()
            .readable()
            .unresolvable()
            .numeric(AccessMarker::Read)
            .idents();
        let readable_unresolvable_enumerated_field_idents = self
            .fields()
            .readable()
            .unresolvable()
            .enumerated(AccessMarker::Read)
            .idents();

        let value_tys = readable_unresolvable_numeric_fields
            .map(|field| {
                let ident = format_ident!(
                    "u{}",
                    Index {
                        index: field.width() as _,
                        span: Span::call_site(),
                    }
                );

                match field.width() {
                    1 => parse_quote! { bool },
                    8 | 16 | 32 => {
                        parse_quote! { #ident }
                    }
                    _ => {
                        parse_quote! { ::proto_hal::macro_utils::arbitrary_int::#ident }
                    }
                }
            })
            .collect::<Vec<Path>>();

        Some(quote_spanned! { span =>
            pub struct Reader {
                value: ::proto_hal::macro_utils::RegisterValue,
            }

            impl From<UnsafeReader> for Reader {
                fn from(unsafe_reader: UnsafeReader) -> Self {
                    Self { value: unsafe_reader.value }
                }
            }

            impl Reader {
                #(
                    pub fn #readable_unresolvable_enumerated_field_idents(&self) -> #readable_unresolvable_enumerated_field_idents::ReadVariant {
                        // SAFETY: assumes
                        // 1. peripheral description is correct (offset/width)
                        // 2. hardware is operating correctly
                        unsafe {
                            #readable_unresolvable_enumerated_field_idents::ReadVariant::from_bits(
                                self.value.region(
                                    #readable_unresolvable_enumerated_field_idents::OFFSET,
                                    #readable_unresolvable_enumerated_field_idents::WIDTH
                                )
                            )
                        }
                    }
                )*

                #(
                    pub fn #readable_unresolvable_numeric_field_idents(&self) -> #value_tys {
                        self.value.#value_tys(#readable_unresolvable_numeric_field_idents::OFFSET)
                    }
                )*
            }
        })
    }

    fn maybe_generate_writer(&self) -> Option<TokenStream2> {
        let span = self.args.span();

        let writable_unresolvable_fields =
            self.fields().writable().unresolvable().collect::<Vec<_>>();

        // don't generate a reader if there are no fields
        // to be written
        if writable_unresolvable_fields.is_empty() {
            return None;
        };

        let writable_unresolvable_numeric_fields = self
            .fields()
            .writable()
            .unresolvable()
            .numeric(AccessMarker::Write);
        let writable_unresolvable_numeric_field_idents = self
            .fields()
            .writable()
            .unresolvable()
            .numeric(AccessMarker::Write)
            .idents();
        let writable_unresolvable_enumerated_fields = self
            .fields()
            .writable()
            .unresolvable()
            .enumerated(AccessMarker::Write)
            .collect::<Vec<_>>();
        let writable_unresolvable_enumerated_field_idents = self
            .fields()
            .writable()
            .unresolvable()
            .enumerated(AccessMarker::Write)
            .idents();

        let value_tys = writable_unresolvable_numeric_fields
            .map(|field| {
                let ident = format_ident!(
                    "u{}",
                    Index {
                        index: field.width() as _,
                        span: Span::call_site(),
                    }
                );

                match field.width() {
                    1 => parse_quote! { bool },
                    8 | 16 | 32 => parse_quote! { #ident },
                    _ => parse_quote! { ::proto_hal::macro_utils::arbitrary_int::#ident },
                }
            })
            .collect::<Vec<Path>>();

        let unresolvable_refined_writer_idents = writable_unresolvable_enumerated_fields
            .iter()
            .map(|field| {
                format_ident!(
                    "{}Writer",
                    inflector::cases::pascalcase::to_pascal_case(&field.ident.to_string())
                )
            })
            .collect::<Vec<_>>();

        Some(quote_spanned! { span =>
            pub struct Writer {
                value: u32,
            }

            impl ::proto_hal::macro_utils::Writer for Writer {
                unsafe fn write(&mut self, f: impl FnOnce(&mut u32)) -> &mut Self {
                    f(&mut self.value);
                    self
                }
            }

            impl Writer {
                const fn new() -> Self {
                    Self {
                        value: 0,
                    }
                }

                #(
                    pub fn #writable_unresolvable_enumerated_field_idents(&mut self) -> #unresolvable_refined_writer_idents<Self> {
                        #unresolvable_refined_writer_idents { w: self }
                    }
                )*

                #(
                    pub fn #writable_unresolvable_numeric_field_idents(&mut self, value: #value_tys) -> &mut Self {
                        unsafe {
                            ::proto_hal::macro_utils::Writer::write(
                                self,
                                |reg| *reg |= (value as u32) << #writable_unresolvable_numeric_field_idents::OFFSET
                            )
                        }
                    }
                )*
            }
        })
    }

    fn maybe_generate_unsafe_reader(&self) -> Option<TokenStream2> {
        let span = self.args.span();

        // don't generate a reader if there are no fields
        // to be read
        if !self.fields().any(|field| field.access.is_read()) {
            return None;
        };

        let readable_numeric_fields = self.fields().readable().numeric(AccessMarker::Read);
        let readable_numeric_field_idents = self
            .fields()
            .readable()
            .numeric(AccessMarker::Read)
            .idents();
        let readable_enumerated_field_idents = self
            .fields()
            .readable()
            .enumerated(AccessMarker::Read)
            .idents();

        let value_tys = readable_numeric_fields
            .map(|field| {
                let ident = format_ident!(
                    "u{}",
                    Index {
                        index: field.width() as _,
                        span: Span::call_site(),
                    }
                );

                match field.width() {
                    1 => parse_quote! { bool },
                    8 | 16 | 32 => {
                        parse_quote! { #ident }
                    }
                    _ => {
                        parse_quote! { ::proto_hal::macro_utils::arbitrary_int::#ident }
                    }
                }
            })
            .collect::<Vec<Path>>();

        let format_derive: Option<Attribute> = if cfg!(feature = "defmt") {
            Some(parse_quote! { #[derive(::defmt::Format)] })
        } else {
            None
        };

        Some(quote_spanned! { span =>
            #format_derive
            pub struct UnsafeReader {
                value: ::proto_hal::macro_utils::RegisterValue,
            }

            impl UnsafeReader {
                const fn new(value: u32) -> Self {
                    Self {
                        value: ::proto_hal::macro_utils::RegisterValue::new(value),
                    }
                }

                #(
                    pub fn #readable_enumerated_field_idents(&self) -> #readable_enumerated_field_idents::ReadVariant {
                        // SAFETY: assumes
                        // 1. peripheral description is correct (offset/width)
                        // 2. hardware is operating correctly
                        unsafe {
                            #readable_enumerated_field_idents::ReadVariant::from_bits(
                                self.value.region(
                                    #readable_enumerated_field_idents::OFFSET,
                                    #readable_enumerated_field_idents::WIDTH
                                )
                            )
                        }
                    }
                )*

                #(
                    pub fn #readable_numeric_field_idents(&self) -> #value_tys {
                        self.value.#value_tys(#readable_numeric_field_idents::OFFSET)
                    }
                )*
            }
        })
    }

    fn maybe_generate_unsafe_writer(&self) -> Option<TokenStream2> {
        let span = self.args.span();

        // don't generate a reader if there are no fields
        // to be written
        if !self.fields().any(|field| field.access.is_write()) {
            return None;
        };

        let writable_numeric_fields = self.fields().writable().numeric(AccessMarker::Write);
        let writable_numeric_field_idents = self
            .fields()
            .writable()
            .numeric(AccessMarker::Write)
            .idents();
        let writable_enumerated_fields = self
            .fields()
            .writable()
            .enumerated(AccessMarker::Write)
            .collect::<Vec<_>>();
        let writable_enumerated_field_idents = self
            .fields()
            .writable()
            .enumerated(AccessMarker::Write)
            .idents();

        let value_tys = writable_numeric_fields
            .map(|field| {
                let ident = format_ident!(
                    "u{}",
                    Index {
                        index: field.width() as _,
                        span: Span::call_site(),
                    }
                );

                match field.width() {
                    1 => parse_quote! { bool },
                    8 | 16 | 32 => parse_quote! { #ident },
                    _ => parse_quote! { ::proto_hal::macro_utils::arbitrary_int::#ident },
                }
            })
            .collect::<Vec<Path>>();

        let refined_writer_idents = writable_enumerated_fields
            .iter()
            .map(|field| {
                format_ident!(
                    "{}Writer",
                    inflector::cases::pascalcase::to_pascal_case(&field.ident.to_string())
                )
            })
            .collect::<Vec<_>>();

        Some(quote_spanned! { span =>
            pub struct UnsafeWriter {
                value: u32,
            }

            impl ::proto_hal::macro_utils::Writer for UnsafeWriter {
                unsafe fn write(&mut self, f: impl FnOnce(&mut u32)) -> &mut Self {
                    f(&mut self.value);
                    self
                }
            }

            impl UnsafeWriter {
                const fn new() -> Self {
                    Self {
                        value: 0,
                    }
                }

                #[allow(unused)]
                const fn with_value(value: u32) -> Self {
                    Self {
                        value,
                    }
                }

                #(
                    pub fn #writable_enumerated_field_idents(&mut self) -> #refined_writer_idents<Self> {
                        #refined_writer_idents { w: self }
                    }
                )*

                #(
                    pub fn #writable_numeric_field_idents(&mut self, value: #value_tys) -> &mut Self {
                        unsafe {
                            ::proto_hal::macro_utils::Writer::write(
                                self,
                                |reg| *reg |= (value as u32) << #writable_numeric_field_idents::OFFSET
                            )
                        }
                    }
                )*
            }
        })
    }

    fn generate_unsafe_interface(&self) -> TokenStream2 {
        let span = self.args.span();

        let mut body = TokenStream2::new();

        if self.fields().any(|field| field.access.is_read()) {
            body.extend(quote_spanned! { span =>
                /// Reads this register **once**, providing
                /// an [`UnsafeReader`] for field value extraction.
                pub unsafe fn read() -> UnsafeReader {
                    UnsafeReader::new(
                        ::core::ptr::read_volatile((super::BASE_ADDR + OFFSET) as *const u32)
                    )
                }
            });
        }

        if self.fields().any(|field| field.access.is_write()) {
            body.extend(quote_spanned! { span =>
                /// Writes to this register with a base value of zero
                /// for unspecified fields.
                pub unsafe fn write_with_zero(f: impl FnOnce(&mut UnsafeWriter) -> &mut UnsafeWriter) {
                    let mut writer = UnsafeWriter::new();

                    f(&mut writer);

                    ::core::ptr::write_volatile((super::BASE_ADDR + OFFSET) as *mut u32, writer.value);
                }
            });

            let writable_resolvable_field_idents = self
                .fields()
                .writable()
                .resolvable()
                .idents()
                .collect::<Vec<_>>();

            if !writable_resolvable_field_idents.is_empty() {
                body.extend(quote_spanned! { span =>
                    /// Writes to this register where unspecified
                    /// fields will return to their reset value.
                    pub unsafe fn write_with_reset(f: impl FnOnce(&mut UnsafeWriter) -> &mut UnsafeWriter) {
                        #[allow(unused_parens)]
                        let mut writer = UnsafeWriter::with_value(
                            #(
                                (#writable_resolvable_field_idents::RESET << #writable_resolvable_field_idents::OFFSET)
                            )|*
                        );

                        f(&mut writer);

                        ::core::ptr::write_volatile((super::BASE_ADDR + OFFSET) as *mut u32, writer.value);
                    }
                });
            }
        }

        if self.fields().any(|field| field.access.is_read())
            && self.fields().any(|field| field.access.is_write())
        {
            body.extend(quote_spanned! { span =>
                /// Reads this register **once** as a source
                /// for the base value of unspecified fields
                /// when writing.
                pub unsafe fn modify(f: impl FnOnce(&mut UnsafeWriter) -> &mut UnsafeWriter) {
                    let mut writer = UnsafeWriter::with_value(read().value.word());

                    f(&mut writer);

                    ::core::ptr::write_volatile((super::BASE_ADDR + OFFSET) as *mut u32, writer.value);
                }
            });
        }

        body
    }

    fn generate_register_struct(&self) -> TokenStream2 {
        let span = self.args.span();

        let resolvable_field_idents = self.fields().resolvable().idents();
        let resolvable_field_tys = self.fields().resolvable().tys().collect::<Vec<_>>();

        let unresolvable_field_idents = self.fields().unresolvable().idents();

        // self.fields().resolvable().map(|field| {
        //     match field.access {
        //         Access::Write(write) | Access::ReadWrite { write, read: _ } => {
        //             write.schema.
        //     }
        // });

        quote_spanned! { span =>
            /// A register. This type gates access to
            /// the fields it encapsulates.
            ///
            /// Field members can be directly moved out of this struct
            /// for lossy modification, or modified in place with
            /// accessor methods.
            pub struct Register<#(#resolvable_field_tys,)*> {
                // resolvable fields.
                #(
                    #resolvable_field_idents: #resolvable_field_tys,
                )*

                // Q: what is this for?
                // unresolvable fields.
                #(
                    #[allow(unused)]
                    #unresolvable_field_idents: (),
                )*
            }
        }
    }

    fn maybe_generate_reset_alias(&self) -> Option<TokenStream2> {
        if !self.is_resolvable() {
            return None;
        };

        let span = self.args.span();

        let resolvable_field_idents = self.fields().resolvable().idents();

        Some(quote_spanned! { span =>
            pub type Reset = Register<
                #(
                    #resolvable_field_idents::Reset,
                )*
            >;
        })
    }

    fn maybe_generate_state_builder(&self) -> Option<TokenStream2> {
        if !self.is_resolvable() {
            return None;
        };

        let span = self.args.span();

        let resolvable_field_idents = self.fields().resolvable().idents().collect::<Vec<_>>();
        let resolvable_field_tys = self.fields().resolvable().tys().collect::<Vec<_>>();
        let writable_resolvable_field_idents = self.fields().writable().resolvable().idents();
        let writable_resolvable_field_tys = self.fields().writable().resolvable().tys();
        let unresolvable_field_idents = self.fields().unresolvable().idents();

        Some(quote_spanned! { span =>
            /// This type facilitates the static construction
            /// of a valid register state.
            pub struct StateBuilder<#(#resolvable_field_tys,)*> {
                #(
                    pub(crate) #resolvable_field_idents: core::marker::PhantomData<#resolvable_field_tys>,
                )*
            }

            impl<#(#resolvable_field_tys,)*> StateBuilder<#(#resolvable_field_tys,)*>
            where
                #(
                    #resolvable_field_tys: #resolvable_field_idents::State,
                )*
            {
                /// For internal use.
                unsafe fn conjure() -> Self {
                    Self {
                        #(
                            #resolvable_field_idents: core::marker::PhantomData,
                        )*
                    }
                }

                /// Complete the state transition and incorporarate
                /// it into the register.
                pub fn finish(self) -> Register<#(#resolvable_field_tys,)*>
                where
                    Self: ::proto_hal::macro_utils::AsRegister,
                {
                    #[allow(unused_parens)]
                    let reg_value = #(
                        ((#writable_resolvable_field_tys::RAW as u32) << #writable_resolvable_field_idents::OFFSET)
                    )|*;

                    // SAFETY: assumes the proc macro implementation is sound
                    // and that the peripheral description is accurate
                    unsafe {
                        core::ptr::write_volatile((super::BASE_ADDR + OFFSET) as *mut u32, reg_value);
                    }

                    // SAFETY:
                    // 1. `self` is destroyed
                    // 2. state has been written
                    Register {
                        #(
                            #resolvable_field_idents: unsafe { #resolvable_field_tys::conjure() },
                        )*

                        #(
                            #unresolvable_field_idents: (), // placeholder
                        )*
                    }
                }
            }
        })
    }

    fn generate_register_impls(&self) -> TokenStream2 {
        let span = self.args.span();

        let resolvable_field_idents = self.fields().resolvable().idents().collect::<Vec<_>>();
        let resolvable_field_tys = self.fields().resolvable().tys().collect::<Vec<_>>();
        let unresolvable_field_idents = self.fields().unresolvable().idents().collect::<Vec<_>>();

        let mut body = TokenStream2::new();

        if self.is_resolvable() {
            // let new_resolvable_field_tys = self
            //     .fields()
            //     .resolvable()
            //     .tys()
            //     .map(|ty| format_ident!("New{ty}"))
            //     .collect::<Vec<_>>();
            // let new_entitlement_bounds = self
            //     .fields()
            //     .resolvable()
            //     .map(|field| {
            //         if field.schema.entitlement_fields.is_empty() {
            //             return None;
            //         }

            //         let entitled_field_tys = field
            //             .schema
            //             .entitlement_fields
            //             .iter()
            //             .map(|ident| {
            //                 format_ident!(
            //                     "New{}",
            //                     &inflector::cases::pascalcase::to_pascal_case(&ident.to_string(),),
            //                 )
            //             })
            //             .collect::<Vec<_>>();

            //         Some(quote_spanned! { span =>
            //             + #(::proto_hal::stasis::Entitled<#entitled_field_tys>)+*
            //         })
            //     })
            //     .collect::<Vec<_>>();

            body.extend(quote_spanned! { span =>
                impl<#(#resolvable_field_tys,)*> Register<#(#resolvable_field_tys,)*>
                where
                    #(
                        #resolvable_field_tys: #resolvable_field_idents::State,
                    )*
                {
                    /// Transition this register into a new state, facilitated by its
                    /// [`StateBuilder`]
                    pub fn transition<B>(self, f: impl FnOnce(<Self as ::proto_hal::macro_utils::AsBuilder>::Builder) -> B) -> B::Register
                    where
                        B: ::proto_hal::macro_utils::AsRegister,
                    {
                        // SAFETY: `self` is destroyed
                        f(unsafe { StateBuilder::conjure() }).into()
                    }
                }
            });

            for (i, field) in self.fields().resolvable().enumerate() {
                let ident = &field.ident;
                let ty = Ident::new(
                    &inflector::cases::pascalcase::to_pascal_case(&ident.to_string()),
                    span,
                );
                let detach_accessor = format_ident!("detach_{ident}");
                let attach_accessor = format_ident!("attach_{ident}");

                let prev_field_idents = resolvable_field_idents.get(..i).unwrap();
                let next_field_idents = resolvable_field_idents.get(i + 1..).unwrap();
                let prev_field_tys = resolvable_field_tys.get(..i).unwrap();
                let next_field_tys = resolvable_field_tys.get(i + 1..).unwrap();

                body.extend(quote_spanned! { span =>
                    impl<#(#resolvable_field_tys,)*> Register<#(#prev_field_tys,)* #ty, #(#next_field_tys,)*> {
                        pub fn #detach_accessor(self) -> (Register<#(#prev_field_tys,)* ::proto_hal::stasis::Unavailable, #(#next_field_tys,)*>, #ty) {
                            (
                                Register {
                                    #(
                                        #prev_field_idents: self.#prev_field_idents,
                                    )*

                                    #ident: ::proto_hal::stasis::Unavailable,

                                    #(
                                        #next_field_idents: self.#next_field_idents,
                                    )*

                                    #(
                                        #unresolvable_field_idents: self.#unresolvable_field_idents,
                                    )*
                                },
                                self.#ident,
                            )
                        }
                    }

                    impl<#(#prev_field_tys,)* #(#next_field_tys,)*> Register<#(#prev_field_tys,)* ::proto_hal::stasis::Unavailable, #(#next_field_tys,)*> {
                        pub fn #attach_accessor<#ty>(self, field: #ty) -> Register<#(#prev_field_tys,)* #ty, #(#next_field_tys,)*>
                        where
                            #ty: #ident::State,
                        {
                            Register {
                                #(
                                    #prev_field_idents: self.#prev_field_idents,
                                )*

                                #ident: field,

                                #(
                                    #next_field_idents: self.#next_field_idents,
                                )*

                                #(
                                    #unresolvable_field_idents: self.#unresolvable_field_idents,
                                )*
                            }
                        }
                    }
                });
            }
        }

        if self
            .fields()
            .unresolvable()
            .any(|field| field.access.is_read())
        {
            body.extend(quote_spanned! { span =>
                impl<#(#resolvable_field_tys,)*> Register<#(#resolvable_field_tys,)*>
                where
                    #(
                        #resolvable_field_tys: #resolvable_field_idents::State,
                    )*
                {
                    pub fn read(&self) -> Reader {
                        // SAFETY: assumes the proc macro implementation is sound
                        // and that the peripheral description is accurate
                        unsafe { read() }.into()
                    }
                }
            });
        }

        if self
            .fields()
            .unresolvable()
            .any(|field| field.access.is_write())
        {
            body.extend(quote_spanned! { span =>
                    impl<#(#resolvable_field_tys,)*> Register<#(#resolvable_field_tys,)*>
                    where
                        #(
                            #resolvable_field_tys: #resolvable_field_idents::State,
                        )*
                    {
                        pub fn write(&self, f: impl FnOnce(&mut Writer) -> &mut Writer) {
                            let mut writer = Writer::new();

                            f(&mut writer);

                            // SAFETY: assumes the proc macro implementation is sound
                            // and that the peripheral description is accurate
                            unsafe {
                                core::ptr::write_volatile((super::BASE_ADDR + OFFSET) as *mut u32, writer.value);
                            }
                        }
                    }
                });
        }

        body
    }

    fn maybe_generate_conversion_trait_impls(&self) -> Option<TokenStream2> {
        if !self.is_resolvable() {
            return None;
        };

        let span = self.args.span();

        let resolvable_field_idents = self.fields().resolvable().idents().collect::<Vec<_>>();
        let resolvable_field_tys = self.fields().resolvable().tys().collect::<Vec<_>>();

        let entitlement_bounds = self
            .fields()
            .resolvable()
            .map(|field| {
                let schema = match &field.access {
                    Access::Read(read) | Access::ReadWrite { read, write: _ } => &read.schema,
                    _ => panic!("a resolvable field should not be write-only"),
                };

                if schema.entitlements.is_empty() {
                    return None;
                }

                let mut entitled_fields = HashSet::new();
                entitled_fields.extend(
                    schema
                        .entitlements
                        .iter()
                        .map(|entitlement| entitlement.field.clone()),
                );

                let entitled_field_tys = entitled_fields
                    .iter()
                    .map(|field| {
                        Ident::new(
                            &inflector::cases::pascalcase::to_pascal_case(&field.to_string()),
                            Span::call_site(),
                        )
                    })
                    .collect::<Vec<_>>();

                Some(quote_spanned! { span =>
                    + #(::proto_hal::stasis::Entitled<#entitled_field_tys>)+*
                })
            })
            .collect::<Vec<_>>();

        Some(quote_spanned! { span =>
            impl<#(#resolvable_field_tys,)*> ::proto_hal::macro_utils::AsBuilder for Register<#(#resolvable_field_tys,)*>
            where
                #(
                    #resolvable_field_tys: #resolvable_field_idents::State,
                )*
            {
                type Builder = StateBuilder<#(#resolvable_field_tys,)*>;
            }

            impl<#(#resolvable_field_tys,)*> ::proto_hal::macro_utils::AsRegister for StateBuilder<#(#resolvable_field_tys,)*>
            where
                #(
                    #resolvable_field_tys: #resolvable_field_idents::State #entitlement_bounds,
                )*
            {
                type Register = Register<#(#resolvable_field_tys,)*>;
            }

            #[allow(clippy::from_over_into)]
            impl<#(#resolvable_field_tys,)*> Into<StateBuilder<#(#resolvable_field_tys,)*>> for Register<#(#resolvable_field_tys,)*>
            where
                #(
                    #resolvable_field_tys: #resolvable_field_idents::State,
                )*
            {
                fn into(self) -> StateBuilder<#(#resolvable_field_tys,)*> {
                    // SAFETY: `self` is destroyed
                    unsafe { StateBuilder::conjure() }
                }
            }

            #[allow(clippy::from_over_into)]
            impl<#(#resolvable_field_tys,)*> Into<Register<#(#resolvable_field_tys,)*>> for StateBuilder<#(#resolvable_field_tys,)*>
            where
                #(
                    #resolvable_field_tys: #resolvable_field_idents::State,
                )*
                Self: ::proto_hal::macro_utils::AsRegister,
            {
                fn into(self) -> Register<#(#resolvable_field_tys,)*> {
                    self.finish()
                }
            }
        })
    }

    fn maybe_generate_builder_methods(&self) -> Option<TokenStream2> {
        if !self.is_resolvable() {
            return None;
        };

        let mut body = TokenStream2::new();

        let span = self.args.span();

        let resolvable_field_idents = self.fields().resolvable().idents().collect::<Vec<_>>();
        let resolvable_field_tys = self.fields().resolvable().tys().collect::<Vec<_>>();

        for (i, field) in self.fields().resolvable().enumerate() {
            let ident = &field.ident;
            let field_state_builder_ty = format_ident!(
                "{}StateBuilder",
                &inflector::cases::pascalcase::to_pascal_case(&ident.to_string()),
            );

            let prev_field_tys = resolvable_field_tys.get(..i).unwrap();
            let next_field_tys = resolvable_field_tys.get(i + 1..).unwrap();

            let schema = match &field.access {
                Access::Read(read) | Access::ReadWrite { read, write: _ } => &read.schema,
                _ => panic!("a resolvable field should not be write-only"),
            };

            if let Numericity::Enumerated { variants } = &schema.numericity {
                let variant_tys = variants
                    .iter()
                    .map(|variant| variant.ident.clone())
                    .collect::<Vec<_>>();
                let variant_accessor_idents = variant_tys
                    .iter()
                    .map(|ident| {
                        Ident::new(
                            &inflector::cases::snakecase::to_snake_case(&ident.to_string()),
                            Span::call_site(),
                        )
                    })
                    .collect::<Vec<_>>();

                for variant in variants {
                    if variant.entitlement_fields.is_empty() {
                        let state_ty = &variant.ident;

                        body.extend(quote_spanned! { span =>
                            unsafe impl<T> ::proto_hal::stasis::Entitled<T> for #ident::#state_ty {}
                        });
                    }
                }

                if field.access.is_write() {
                    body.extend(quote_spanned! { span =>
                            impl<#(#resolvable_field_tys,)*> StateBuilder<#(#resolvable_field_tys,)*>
                            where
                                #(
                                    #resolvable_field_tys: #resolvable_field_idents::State,
                                )*
                            {
                                /// Change the state of this field.
                                pub fn #ident(self) -> #field_state_builder_ty<#(#resolvable_field_tys,)*> {
                                    unsafe { core::mem::transmute(()) }
                                }
                            }

                            pub struct #field_state_builder_ty<#(#resolvable_field_tys,)*> {
                                #(
                                    #resolvable_field_idents: core::marker::PhantomData<#resolvable_field_tys>,
                                )*
                            }

                            impl<#(#resolvable_field_tys,)*> #field_state_builder_ty<#(#resolvable_field_tys,)*>
                            where
                                #(
                                    #resolvable_field_tys: #resolvable_field_idents::State,
                                )*
                            {
                                pub fn generic<S>(self) -> StateBuilder<#(#prev_field_tys,)* S, #(#next_field_tys,)*>
                                where
                                    S: #ident::State,
                                {
                                    // SAFETY: `self` is destroyed
                                    unsafe { StateBuilder::conjure() }
                                }

                                // pub fn dynamic(self, state: #ident::Variant) -> StateBuilder<#(#prev_field_tys,)* #ident::Any, #(#next_field_tys,)*> {
                                //     todo!()
                                // }
                            }
                        });

                    for (ty, accessor) in variant_tys.iter().zip(variant_accessor_idents) {
                        let doc = format!("Set the state of the field to [`{ty}`]({ident}::{ty}).");

                        body.extend(quote_spanned! { span =>
                                impl<#(#resolvable_field_tys,)*> #field_state_builder_ty<#(#resolvable_field_tys,)*>
                                where
                                    #(
                                        #resolvable_field_tys: #resolvable_field_idents::State,
                                    )*
                                {
                                    #[doc = #doc]
                                    pub fn #accessor(self) -> StateBuilder<#(#prev_field_tys,)* #ident::#ty, #(#next_field_tys,)*>
                                    where
                                        #ident::#ty: #ident::State,
                                    {
                                        self.generic()
                                    }
                                }
                            });
                    }
                }
            }
        }

        Some(body)
    }

    fn maybe_generate_states_trait(&self) -> Option<TokenStream2> {
        let span = self.args.span();

        if !self.fields().any(|field| field.is_resolvable()) {
            return None;
        }

        let resolvable_field_tys = self.fields().resolvable().tys();

        Some(quote_spanned! { span =>
            pub trait States {
                #(
                    type #resolvable_field_tys;
                )*
            }
        })
    }
}

impl ToTokens for Register {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let span = self.args.span();
        let ident = &self.ident;
        let mut body = TokenStream2::new();

        body.extend(self.generate_field_bodies());
        body.extend(self.generate_offset_const());
        body.extend(self.maybe_generate_refined_writers());
        body.extend(self.maybe_generate_reader());
        body.extend(self.maybe_generate_writer());
        body.extend(self.maybe_generate_unsafe_reader());
        body.extend(self.maybe_generate_unsafe_writer());
        body.extend(self.generate_unsafe_interface());
        body.extend(self.generate_register_struct());
        body.extend(self.maybe_generate_reset_alias());
        body.extend(self.maybe_generate_state_builder());
        body.extend(self.generate_register_impls());
        body.extend(self.maybe_generate_conversion_trait_impls());
        body.extend(self.maybe_generate_builder_methods());
        body.extend(self.maybe_generate_states_trait());

        tokens.extend(quote_spanned! { span =>
            pub mod #ident {
                #body
            }
        });
    }
}
