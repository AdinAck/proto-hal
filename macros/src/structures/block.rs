use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};

use darling::FromMeta;
use proc_macro2::Span;
use quote::{format_ident, quote_spanned, ToTokens};
use syn::{parse_quote, Ident, Item, Path, Visibility};
use tiva::{Validate, Validator};

use crate::utils::{extract_items_from, require_module, PathArray, Spanned};

use super::{
    register::{Register, RegisterArgs, RegisterSpec},
    schema::{Schema, SchemaArgs, SchemaSpec},
    Args,
};

#[derive(Debug, Clone, Default, FromMeta)]
#[darling(default)]
pub struct BlockArgs {
    pub base_addr: u32,
    pub entitlements: PathArray,

    #[darling(default)]
    pub auto_increment: bool,
    #[darling(default)]
    pub erase_mod: bool,
}

impl Args for BlockArgs {
    const NAME: &str = "block";
}

#[derive(Debug)]
pub struct BlockSpec {
    pub args: Spanned<BlockArgs>,
    pub ident: Ident,
    pub base_addr: u32,
    pub entitlements: HashSet<Path>,
    pub registers: Vec<Register>,
    pub schemas: HashMap<Ident, Schema>,

    pub vis: Visibility,
}

#[derive(Debug)]
pub struct Block {
    spec: BlockSpec,
}

impl Deref for Block {
    type Target = BlockSpec;

    fn deref(&self) -> &Self::Target {
        &self.spec
    }
}

impl BlockSpec {
    pub fn parse<'a>(
        ident: Ident,
        vis: Visibility,
        args: Spanned<BlockArgs>,
        items: impl Iterator<Item = &'a Item>,
    ) -> syn::Result<Self> {
        let mut block = Self {
            args: args.clone(),
            ident,
            base_addr: args.base_addr,
            entitlements: HashSet::new(),
            registers: Vec::new(),
            schemas: HashMap::new(),
            vis,
        };

        for entitlement in &args.entitlements.elems {
            if !block.entitlements.insert(entitlement.clone()) {
                Err(syn::Error::new_spanned(
                    entitlement,
                    "entitlement exists already",
                ))?
            }
        }

        let mut register_offset = 0u8;

        for item in items {
            let module = require_module(item)?;

            if let Some(schema_args) = SchemaArgs::get(module.attrs.iter())? {
                let schema: Schema = SchemaSpec::parse(
                    module.ident.clone(),
                    schema_args,
                    extract_items_from(module)?.iter(),
                )?
                .validate()?;

                block.schemas.insert(schema.ident().clone(), schema);
            } else if let Some(register_args) = RegisterArgs::get(module.attrs.iter())? {
                if !args.auto_increment && register_args.offset.is_none() {
                    // TODO: improve the span of this error
                    Err(syn::Error::new_spanned(block.ident.clone(), "register offset must be specified. to infer offsets, add the `auto_increment` argument to the block attribute macro"))?
                }

                let offset = register_args.offset;

                let register = RegisterSpec::parse(
                    module.ident.clone(),
                    &mut block.schemas,
                    register_args.offset.unwrap_or(register_offset),
                    register_args,
                    extract_items_from(module)?.iter(),
                )?
                .validate()?;

                register_offset = offset.unwrap_or(register_offset) + 0x4;

                block.registers.push(register);
            } else {
                Err(syn::Error::new_spanned(module, "erroneous module"))?
            }
        }

        Ok(block)
    }
}

impl Validator<BlockSpec> for Block {
    type Error = syn::Error;

    fn validate(spec: BlockSpec) -> Result<Self, Self::Error> {
        for register in &spec.registers {
            if register.args.offset.is_none() && !spec.args.auto_increment {
                return Err(syn::Error::new(
                    register.args.span(),
                    "register offset must be specified. to infer offsets, use `auto_increment`",
                ));
            }
        }

        for slice in spec.registers.windows(2) {
            let lhs = slice.first().unwrap();
            let rhs = slice.last().unwrap();
            if lhs.offset + 4 > rhs.offset {
                let msg = format!(
                    "register domains overlapping. {} {{ domain: {}..{} }}, {} {{ domain: {}..{} }}",
                    lhs.ident, lhs.offset, lhs.offset + 4,
                    rhs.ident, rhs.offset, rhs.offset + 4,
                );

                Err(syn::Error::new(spec.args.span(), msg))?
            }
        }

        Ok(Self { spec })
    }
}

impl ToTokens for Block {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;
        let base_addr = self.base_addr;

        let span = self.args.span();

        let (stateful_registers, stateless_registers) = self
            .registers
            .iter()
            .partition::<Vec<_>, _>(|register| register.is_stateful());

        let stateful_register_idents = stateful_registers
            .iter()
            .map(|register| &register.ident)
            .collect::<Vec<_>>();

        let stateless_register_idents = stateless_registers
            .iter()
            .map(|register| &register.ident)
            .collect::<Vec<_>>();

        let stateful_register_tys = stateful_registers
            .iter()
            .map(|register| {
                Ident::new(
                    &inflector::cases::pascalcase::to_pascal_case(&register.ident.to_string()),
                    Span::call_site(),
                )
            })
            .collect::<Vec<_>>();

        let entitlement_idents = (0..self.entitlements.len())
            .map(|i| format_ident!("entitlement{}", i))
            .collect::<Vec<_>>();

        let entitlement_tys = (0..self.entitlements.len())
            .map(|i| format_ident!("Entitlement{}", i))
            .collect::<Vec<_>>();

        let reset_entitlement_tys = entitlement_tys
            .iter()
            .map(|_| {
                parse_quote! {
                    ::proto_hal::stasis::Unsatisfied
                }
            })
            .collect::<Vec<Path>>();

        let register_bodies = self
            .registers
            .iter()
            .map(|register| quote_spanned! { span => #register });

        let mut body = quote_spanned! { span =>
            #(
                #register_bodies
            )*

            const BASE_ADDR: u32 = #base_addr;

            pub struct Block<
                #(
                    #stateful_register_tys,
                )*

                #(
                    #entitlement_tys,
                )*
            > {
                #(
                    pub #stateful_register_idents: #stateful_register_tys,
                )*

                #(
                    pub #stateless_register_idents: #stateless_register_idents::Register,
                )*

                #(
                    pub #entitlement_idents: #entitlement_tys,
                )*
            }

            pub type Reset = Block<
                #(
                    #stateful_register_idents::Reset,
                )*

                #(
                    #reset_entitlement_tys,
                )*
            >;

            impl Reset {
                pub unsafe fn conjure() -> Self {
                    ::core::mem::transmute(())
                }
            }
        };

        let entitlements = self
            .entitlements
            .iter()
            .map(|path| {
                parse_quote! {
                    ::proto_hal::stasis::Entitlement<#path>
                }
            })
            .collect::<Vec<Path>>();

        for (i, (ident, ty)) in stateful_register_idents
            .iter()
            .zip(stateful_register_tys.iter())
            .enumerate()
        {
            let prev_register_idents = stateful_register_idents.get(..i).unwrap();
            let next_register_idents = stateful_register_idents.get(i + 1..).unwrap();

            let prev_register_tys = stateful_register_tys.get(..i).unwrap();
            let next_register_tys = stateful_register_tys.get(i + 1..).unwrap();

            body.extend(quote_spanned! { span =>
                impl<#(#stateful_register_tys,)*> Block<#(#stateful_register_tys,)* #(#entitlements,)*>
                where
                    #ty: ::proto_hal::macro_utils::AsBuilder,
                {
                    pub fn #ident<R, B>(self, f: impl FnOnce(#ty::Builder) -> B) -> Block<#(#prev_register_tys,)* R, #(#next_register_tys,)* #(#entitlements,)*>
                    where
                        B: ::proto_hal::macro_utils::AsRegister<Register = R>,
                    {
                        Block {
                            #(
                                #prev_register_idents: self.#prev_register_idents,
                            )*

                            #ident: f(self.#ident.into()).into(),

                            #(
                                #next_register_idents: self.#next_register_idents,
                            )*

                            #(
                                #stateless_register_idents: self.#stateless_register_idents,
                            )*

                            #(
                                #entitlement_idents: self.#entitlement_idents,
                            )*
                        }
                    }
                }
            });
        }

        if !self.entitlements.is_empty() {
            body.extend(quote_spanned! { span =>
                impl<#(#stateful_register_tys,)*> Block<#(#stateful_register_tys,)* #(#reset_entitlement_tys,)*> {
                    pub fn attach(self, #(#entitlement_idents: #entitlements,)*) -> Block<#(#stateful_register_tys,)* #(#entitlements,)*> {
                        Block {
                            #(
                                #stateful_register_idents: self.#stateful_register_idents,
                            )*

                            #(
                                #stateless_register_idents: self.#stateless_register_idents,
                            )*

                            #(
                                #entitlement_idents,
                            )*
                        }
                    }
                }
            });
        }

        let vis = &self.vis;

        tokens.extend(if self.args.erase_mod {
            body
        } else {
            quote_spanned! { span =>
                #vis mod #ident {
                    #body
                }
            }
        })
    }
}
