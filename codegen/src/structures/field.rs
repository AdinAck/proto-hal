use std::{collections::HashMap, ops::Deref};

use darling::{util::SpannedValue, FromMeta};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{Expr, Ident, Item};
use tiva::Validator;

use crate::{
    access::{self, Access, AccessArgs, AccessSpec, Read, Write},
    utils::{FieldOffset, Spanned, SynErrorCombinator, Width},
};

use super::{
    schema::{Numericity, Schema, SchemaArgs, SchemaSpec},
    variant::Variant,
    Args,
};

#[derive(Debug, Clone, Default, FromMeta)]
pub struct FieldArgs {
    pub offset: Option<FieldOffset>,
    pub width: Option<SpannedValue<Width>>,
    pub schema: Option<Ident>,
    pub read: Option<SpannedValue<AccessArgs>>,
    pub write: Option<SpannedValue<AccessArgs>>,
    pub reset: Option<Expr>,

    #[darling(default)]
    pub auto_increment: bool,
}

impl Args for FieldArgs {
    const NAME: &str = "field";
}

impl FieldArgs {
    pub fn check_conflict_and_inherit(&mut self) -> syn::Result<()> {
        let mut errors = SynErrorCombinator::new();

        let msg = "property is inherited from register";

        for args in [self.read.as_mut(), self.write.as_mut()]
            .into_iter()
            .flatten()
        {
            if let Some(inherited_schema) = &self.schema {
                if let Some(schema) = &args.schema {
                    errors.push(syn::Error::new(schema.span(), msg));
                } else {
                    args.schema.replace(inherited_schema.clone());
                }
            }
        }

        errors.coalesce()
    }
}

#[derive(Debug)]
pub enum Resolvability {
    Resolvable { reset: Expr },
    Unresolvable,
}

pub struct Resolvable {
    reset: Expr,
}

pub struct Unresolvable;

#[derive(Debug)]
pub struct FieldSpec<A, R> {
    pub args: Spanned<FieldArgs>,
    pub ident: Ident,
    pub offset: FieldOffset,
    pub access: A,

    // private because it is a computed
    // property
    width: Width,
    resolvability: R,
}

#[derive(Debug)]
pub struct Field<A, R> {
    spec: FieldSpec<A, R>,
}

impl<A, R> Deref for Field<A, R> {
    type Target = FieldSpec<A, R>;

    fn deref(&self) -> &Self::Target {
        &self.spec
    }
}

pub type Unrefined = Field<access::Unrefined, Resolvability>;

impl<A, R> Field<A, R> {
    pub fn refine_access<NewA>(
        self,
        f: impl FnOnce(A) -> Result<NewA, A>,
    ) -> Result<Field<NewA, R>, Self> {
        let spec = self.spec;

        match f(spec.access) {
            Ok(access) => Ok(Field {
                spec: FieldSpec {
                    args: spec.args,
                    ident: spec.ident,
                    offset: spec.offset,
                    access,
                    width: spec.width,
                    resolvability: spec.resolvability,
                },
            }),
            Err(access) => Err(Field {
                spec: FieldSpec {
                    args: spec.args,
                    ident: spec.ident,
                    offset: spec.offset,
                    access,
                    width: spec.width,
                    resolvability: spec.resolvability,
                },
            }),
        }
    }
}

impl FieldSpec<access::Unrefined, Resolvability> {
    pub fn parse<'a>(
        ident: Ident,
        offset: FieldOffset,
        schemas: &HashMap<Ident, Schema>,
        mut args: Spanned<FieldArgs>,
        items: impl Iterator<Item = &'a Item>,
    ) -> syn::Result<Self> {
        args.check_conflict_and_inherit()?; // WARN: very important and easy to miss

        let implicit_schema = if args
            .read
            .as_ref()
            .and_then(|read| read.schema.as_ref())
            .is_some()
            || args
                .write
                .as_ref()
                .and_then(|write| write.schema.as_ref())
                .is_some()
        {
            None
        } else {
            // derive schema from field body
            Some(Schema::validate(SchemaSpec::parse(
                ident.clone(),
                SchemaArgs {
                    auto_increment: args.auto_increment,
                    width: *args
                        .width
                        .ok_or(syn::Error::new(args.span(), "width must be specified"))?,
                }
                .with_span(args.span()),
                items,
            )?)?)
        };

        let offset = args.offset.unwrap_or(offset);

        let access = AccessSpec::parse(
            args.read.as_ref(),
            args.write.as_ref(),
            implicit_schema,
            schemas,
        )?;

        Self::new(args, ident, offset, access)
    }
}

impl FieldSpec<access::Unrefined, Resolvability> {
    pub fn new(
        args: Spanned<FieldArgs>,
        ident: Ident,
        offset: FieldOffset,
        access: AccessSpec,
    ) -> Result<Self, syn::Error> {
        let width = Self::compute_width(&access);
        let resolvability = Self::compute_resolvability(&args, &access)?;

        Ok(Self {
            args,
            ident,
            offset,
            access,

            width,
            resolvability,
        })
    }

    pub fn resolvability(&self) -> &Resolvability {
        &self.resolvability
    }

    pub fn is_resolvable(&self) -> bool {
        matches!(self.resolvability(), Resolvability::Resolvable { reset: _ })
    }

    pub fn width(&self) -> Width {
        self.width
    }

    fn compute_width(access: &AccessSpec) -> Width {
        match access {
            AccessSpec::Read(read) => read.schema.width,
            AccessSpec::Write(write) => write.schema.width,
            AccessSpec::ReadWrite { read, write } => {
                // unnecessary, these by definition must be equal
                // if this fails something is broken in `access`
                assert_eq!(read.schema.width, write.schema.width);

                read.schema.width
            }
        }
    }

    fn compute_resolvability(
        args: &Spanned<FieldArgs>,
        access: &AccessSpec,
    ) -> Result<Resolvability, syn::Error> {
        /*
        Determining Resolvability:

        For a field to be resolvable, it is to
        be statically analyzable. That means it can
        be configured such that its state is trackable
        with the type system.

        For a field to be resolvable, it must be:
        1. explicitly resolved by an effect somewhere or
        2. read and write

        If a field is purely read, with no resolving effects,
        this field's value must be sourced from an external
        influence (for example the IDR register of GPIO) and
        as such cannot be resolved statically.

        An example of a field that is read only but *has*
        resolving effects, is the RRDY bit in CORDIC.
        We know this bit to be 1 when the interrupt fires,
        so at that point it is resolved.

        An example of an unresolvable write only field
        is the ARG field of the CORDIC. The register
        maintains no state and can only be written to,
        so it has no state to resolve, thus unresolvable.

        ---

        This is NOT to be confused with statefulness. Fields
        can be stateful and unresolvable. Some peripheral states
        simply may be too dynamic to be tracked statically.
        */

        Ok(if let AccessSpec::ReadWrite { read, write } = access {
            if read.schema == write.schema {
                Resolvability::Resolvable {
                    reset: args.reset.clone().ok_or(syn::Error::new(
                        args.span(),
                        "resolvable fields must have a reset specified",
                    ))?,
                }
            } else {
                Resolvability::Unresolvable
            }
        } else {
            Resolvability::Unresolvable
        })
    }
}

impl Validator<FieldSpec<access::Unrefined, Resolvability>>
    for Field<access::Unrefined, Resolvability>
{
    type Error = syn::Error;

    fn validate(spec: FieldSpec<access::Unrefined, Resolvability>) -> Result<Self, Self::Error> {
        let mut errors = SynErrorCombinator::new();

        if spec.args.width.is_some() && spec.args.schema.is_some() {
            errors.push(syn::Error::new(
                spec.args.span(),
                "field width is inherited from imported schema",
            ));
        }

        if spec.offset + spec.width > 32 {
            let msg = format!(
                "field domain exceeds register domain. {{ domain: {}..{} }}",
                spec.offset,
                spec.offset + spec.width
            );

            errors.push(Self::Error::new(spec.args.span(), msg));
        }

        errors.coalesce()?;

        Ok(Self { spec })
    }
}

impl Field {
    fn maybe_generate_state_bodies(&self) -> Option<TokenStream2> {
        if !self.is_resolvable() {
            return None;
        };

        // NOTE: if a field is resolvable and has split schemas,
        // the schema that represents the resolvable aspect of the
        // field must be from read access, as the value the field
        // holds must represent the state to be resolved
        let schema = match &self.access {
            AccessSpec::Read(read) | AccessSpec::ReadWrite { read, write: _ } => &read.schema,
            _ => return None,
        };

        let Numericity::Enumerated { variants } = &schema.numericity else {
            return None;
        };

        let span = self.args.span();

        // NOTE: the variant renders to a state implementation
        let state_bodies = variants.iter().map(|variant| quote! { #variant });

        Some(quote_spanned! { span =>
            #(
                #state_bodies
            )*
        })
    }

    fn generate_offset_const(&self) -> TokenStream2 {
        let span = self.args.span();

        let offset = self.offset;

        quote_spanned! { span =>
            pub const OFFSET: u8 = #offset;
        }
    }

    fn generate_width_const(&self) -> TokenStream2 {
        let span = self.args.span();

        let width = self.width;

        quote_spanned! { span =>
            pub const WIDTH: u8 = #width;
        }
    }

    fn maybe_generate_resets(&self) -> Option<TokenStream2> {
        let span = self.args.span();

        let Resolvability::Resolvable { reset } = &self.resolvability else {
            return None;
        };

        let schema = match &self.access {
            AccessSpec::Read(read) | AccessSpec::ReadWrite { read, write: _ } => &read.schema,
            _ => return None,
        };

        match &schema.numericity {
            Numericity::Enumerated { variants: _ } => Some(quote_spanned! { span =>
                pub type Reset = #reset;
                pub const RESET: u32 = Reset::RAW as u32;
            }),
            Numericity::Numeric => todo!(),
        }
    }

    fn maybe_generate_variant_enum(&self) -> Option<TokenStream2> {
        let span = self.args.span();

        let variant_enum = |ident, variants: &Vec<Variant>| {
            let variant_idents = variants
                .iter()
                .map(|variant| variant.ident.clone())
                .collect::<Vec<_>>();
            let variant_bits = variants
                .iter()
                .map(|variant| variant.bits)
                .collect::<Vec<_>>();

            let is_variant_idents = variants.iter().map(|variant| {
                format_ident!(
                    "is_{}",
                    inflector::cases::snakecase::to_snake_case(&variant.ident.to_string())
                )
            });

            quote_spanned! { span =>
                #[repr(u32)]
                pub enum #ident {
                    #(
                        #variant_idents = #variant_bits,
                    )*
                }

                impl #ident {
                    pub unsafe fn from_bits(bits: u32) -> Self {
                        match bits {
                            #(
                                #variant_bits => Self::#variant_idents,
                            )*
                            _ => ::core::hint::unreachable_unchecked(),
                        }
                    }

                    #(
                        pub fn #is_variant_idents(&self) -> bool {
                            matches!(self, Self::#variant_idents)
                        }
                    )*
                }
            }
        };

        // TODO: there must be a better way to do this
        match &self.access {
            AccessSpec::Read(read) => {
                let Numericity::Enumerated { variants } = &read.schema.numericity else {
                    return None;
                };

                let variant_enum = variant_enum(Ident::new("Variant", span), variants);

                Some(quote_spanned! { span =>
                    pub type ReadVariant = Variant;
                    pub type WriteVariant = Variant;
                    #variant_enum
                })
            }
            AccessSpec::Write(write) => {
                let Numericity::Enumerated { variants } = &write.schema.numericity else {
                    return None;
                };

                let variant_enum = variant_enum(Ident::new("Variant", span), variants);

                Some(quote_spanned! { span =>
                    pub type ReadVariant = Variant;
                    pub type WriteVariant = Variant;
                    #variant_enum
                })
            }
            AccessSpec::ReadWrite { read, write } => {
                if read.schema == write.schema {
                    let Numericity::Enumerated { variants } = &read.schema.numericity else {
                        return None;
                    };

                    let variant_enum = variant_enum(Ident::new("Variant", span), variants);

                    Some(quote_spanned! { span =>
                        pub type ReadVariant = Variant;
                        pub type WriteVariant = Variant;
                        #variant_enum
                    })
                } else {
                    let read_variant_enum = if let Numericity::Enumerated {
                        variants: read_variants,
                    } = &read.schema.numericity
                    {
                        Some(variant_enum(Ident::new("ReadVariant", span), read_variants))
                    } else {
                        return None;
                    };
                    let write_variant_enum = if let Numericity::Enumerated {
                        variants: write_variants,
                    } = &write.schema.numericity
                    {
                        Some(variant_enum(
                            Ident::new("WriteVariant", span),
                            write_variants,
                        ))
                    } else {
                        return None;
                    };

                    if let (None, None) = (&read_variant_enum, &write_variant_enum) {
                        return None;
                    }

                    Some(quote_spanned! { span =>
                        #read_variant_enum
                        #write_variant_enum
                    })
                }
            }
        }
    }

    fn maybe_generate_state_trait(&self) -> Option<TokenStream2> {
        let span = self.args.span();

        if !self.is_resolvable() {
            return None;
        };

        let schema = match &self.access {
            AccessSpec::Read(read) | AccessSpec::ReadWrite { read, write: _ } => &read.schema,
            _ => return None,
        };

        match &schema.numericity {
            Numericity::Enumerated { variants } => {
                let variant_idents = variants
                    .iter()
                    .map(|variant| variant.ident.clone())
                    .collect::<Vec<_>>();

                let conversion_methods = if self.access.is_write() {
                    let into_func_idents = variant_idents.iter().map(|ident| {
                        format_ident!(
                            "into_{}",
                            inflector::cases::snakecase::to_snake_case(&ident.to_string())
                        )
                    });

                    let warning_msg = "# Warning
This method incurs a runtime cost and is lossy,
as an entire register read is needed to mutate a single field.
Consider using register accessors when performing state transitions.";

                    let into_func_docs = variant_idents.iter().map(|ident| {
                        format!("Convert this state into [`{}`].\n{}", ident, warning_msg)
                    });

                    Some(quote! {
                        /// Convert this state into a new state.
                        #[doc = #warning_msg]
                        fn into_state<S>(self) -> S
                        where
                            S: State,
                        {
                            // SAFETY: assumes the proc macro implementation is sound
                            // and that the peripheral description is accurate
                            let mut reg_value = unsafe { core::ptr::read_volatile((super::super::BASE_ADDR + super::OFFSET) as *const u32) };

                            // i.e.
                            // 0000 0000 0000 0000 0111 1111 1100 0000
                            const MASK: u32 = (0xffff_ffff >> (32 - (WIDTH as u32))) << (OFFSET as u32);

                            reg_value &= !MASK;
                            reg_value |= (S::RAW as u32) << (OFFSET as u32);

                            // SAFETY: assumes the proc macro implementation is sound
                            // and that the peripheral description is accurate
                            unsafe {
                                core::ptr::write_volatile((super::super::BASE_ADDR + super::OFFSET) as *mut u32, reg_value);
                            }

                            // SAFETY:
                            // 1. previous state is moved and destroyed
                            // 2. state has been written to field
                            unsafe { S::conjure() }
                        }

                        #(
                            #[doc = #into_func_docs]
                            fn #into_func_idents(self) -> #variant_idents
                            {
                                self.into_state()
                            }
                        )*
                    })
                } else {
                    None
                };

                Some(quote_spanned! { span =>
                    pub trait State: ::proto_hal::stasis::Freeze {
                        const RAW: ReadVariant;

                        unsafe fn conjure() -> Self;

                        #conversion_methods
                    }
                })
            }
            Numericity::Numeric => todo!(),
        }
    }

    fn generate_module_docs(&self) -> TokenStream2 {
        let span = self.args.span();

        let access_doc = match &self.access {
            AccessSpec::Read(_) => "- Access: read",
            AccessSpec::Write(_) => "- Access: write",
            AccessSpec::ReadWrite { read: _, write: _ } => "- Access: read/write",
        };

        let domain_doc = format!("- Domain: {}..{}", self.offset, self.offset + self.width);

        let resolvability_doc = if self.is_resolvable() {
            "- Type: resolvable"
        } else {
            "- Type: unresolvable"
        };

        // TODO: figure this out
        // let variants_doc = if let Numericity::Enumerated { variants } = &self.schema.numericity {
        //     let msg = format!("\t- Variants: {}", variants.len());

        //     Some(quote! { #[doc = #msg] })
        // } else {
        //     None
        // };

        quote_spanned! { span =>
            #[doc = "A register field with the following properties:"]
            #[doc = #access_doc]
            #[doc = #domain_doc]
            #[doc = #resolvability_doc]
            // #variants_doc
        }
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let span = self.args.span();
        let ident = &self.ident;
        let mut body = TokenStream2::new();

        body.extend(self.maybe_generate_state_bodies());
        body.extend(self.generate_offset_const());
        body.extend(self.generate_width_const());
        body.extend(self.maybe_generate_resets());
        body.extend(self.maybe_generate_variant_enum());
        body.extend(self.maybe_generate_state_trait());

        let docs = self.generate_module_docs();

        tokens.extend(quote_spanned! { span =>
            #docs
            pub mod #ident {
                #body
            }
        });
    }
}
