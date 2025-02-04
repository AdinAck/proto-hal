use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};

use darling::FromMeta;
use proc_macro2::Span;
use quote::{format_ident, quote_spanned, ToTokens};
use syn::{parse_quote, Ident, Item, Path, Visibility};
use tiva::Validator;

use crate::utils::{extract_items_from, require_module, PathArray, Spanned, SynErrorCombinator};

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
        let mut errors = SynErrorCombinator::new();

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
                errors.push(syn::Error::new_spanned(
                    entitlement,
                    "entitlement exists already",
                ));
            }
        }

        let mut register_offset = 0u32;

        for item in items {
            let module = require_module(item)?;

            // TODO: this isn't the most flexible solution
            // but it does work for now.
            // args should be dispatched procedurally.
            match (
                SchemaArgs::get(module.attrs.iter())?,
                RegisterArgs::get(module.attrs.iter())?,
            ) {
                (Some(schema_args), None) => {
                    errors.try_maybe_then(
                        SchemaSpec::parse(
                            module.ident.clone(),
                            schema_args,
                            extract_items_from(module)?.iter(),
                        ),
                        |spec| {
                            let schema = Schema::validate(spec)?;

                            block.schemas.insert(schema.ident.clone(), schema);

                            Ok(())
                        },
                    );
                }
                (None, Some(register_args)) => {
                    errors.try_maybe_then(
                        RegisterSpec::parse(
                            module.ident.clone(),
                            &mut block.schemas,
                            register_args.offset.unwrap_or(register_offset),
                            register_args,
                            extract_items_from(module)?.iter(),
                        ),
                        |spec| {
                            let register = Register::validate(spec)?;

                            register_offset = register.args.offset.unwrap_or(register_offset) + 0x4;
                            block.registers.push(register);

                            Ok(())
                        },
                    );
                }
                (None, None) => {
                    errors.push(syn::Error::new_spanned(module, "extraneous item"));
                }
                (schema_args, register_args) => {
                    let msg = "only one module annotation is permitted";

                    for span in [
                        schema_args.map(|args| args.span()),
                        register_args.map(|args| args.span()),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        errors.push(syn::Error::new(span, msg));
                    }
                }
            }
        }

        errors.coalesce()?;

        Ok(block)
    }
}

impl Validator<BlockSpec> for Block {
    type Error = syn::Error;

    fn validate(spec: BlockSpec) -> Result<Self, Self::Error> {
        let mut errors = SynErrorCombinator::new();

        for register in &spec.registers {
            if register.args.offset.is_none() && !spec.args.auto_increment {
                errors.push(syn::Error::new(
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

                errors.push(syn::Error::new(spec.args.span(), msg));
            }
        }

        errors.coalesce()?;

        Ok(Self { spec })
    }
}

impl ToTokens for Block {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;
        let base_addr = self.base_addr;

        let span = self.args.span();

        let (resolvable_registers, unresolvable_registers) = self
            .registers
            .iter()
            .partition::<Vec<_>, _>(|register| register.is_resolvable());

        let resolvable_register_idents = resolvable_registers
            .iter()
            .map(|register| &register.ident)
            .collect::<Vec<_>>();

        let unresolvable_register_idents = unresolvable_registers
            .iter()
            .map(|register| &register.ident)
            .collect::<Vec<_>>();

        let resolvable_register_tys = resolvable_registers
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

            /// The address of this block.
            const BASE_ADDR: u32 = #base_addr;

            /// A register block. This type gates
            /// access to the registers it encapsulates.
            ///
            /// Register members can be directly moved out of this struct
            /// or modified in place with accessor methods.
            pub struct Block<
                #(
                    #resolvable_register_tys,
                )*

                #(
                    #entitlement_tys,
                )*
            > {
                // Stateful registers.
                #(
                    #resolvable_register_idents: #resolvable_register_tys,
                )*

                // Stateless registers.
                #(
                    #unresolvable_register_idents: #unresolvable_register_idents::Register,
                )*

                #(
                    /// This entitlement is required to
                    /// use this block in any way.
                    #entitlement_idents: #entitlement_tys,
                )*
            }

            pub type Reset = Block<
                #(
                    #resolvable_register_idents::Reset,
                )*

                #(
                    #reset_entitlement_tys,
                )*
            >;

            impl Reset {
                /// Conjure an instance of this block in reset state.
                ///
                /// # Safety
                ///
                /// If the underlying hardware is *not* in the
                /// reset state, the aassumed invariances of
                /// this block are broken and may lead to UB.
                ///
                /// Do not create multiple instances of this block.
                pub unsafe fn conjure() -> Self {
                    ::core::mem::transmute(())
                }
            }
        };

        let entitlements = self
            .entitlements
            .iter()
            .map(|path| path.clone())
            .collect::<Vec<_>>();

        for (i, (ident, ty)) in resolvable_register_idents
            .iter()
            .zip(resolvable_register_tys.iter())
            .enumerate()
        {
            let prev_register_idents = resolvable_register_idents.get(..i).unwrap();
            let next_register_idents = resolvable_register_idents.get(i + 1..).unwrap();

            let prev_register_tys = resolvable_register_tys.get(..i).unwrap();
            let next_register_tys = resolvable_register_tys.get(i + 1..).unwrap();

            let transition_accessor = format_ident!("transition_{ident}");
            let use_accessor = format_ident!("use_{ident}");

            body.extend(quote_spanned! { span =>
                impl<#(#resolvable_register_tys,)* #(#entitlement_tys,)*> Block<#(#resolvable_register_tys,)* #(#entitlement_tys,)*>
                where
                    #ty: ::proto_hal::macro_utils::AsBuilder,
                {
                    /// Access this register for in place transitioning.
                    pub fn #transition_accessor<R, B>(self, f: impl FnOnce(#ty::Builder) -> B) -> Block<#(#prev_register_tys,)* R, #(#next_register_tys,)* #(#entitlement_tys,)*>
                    where
                        B: ::proto_hal::macro_utils::AsRegister<Register = R>,
                        #(
                            #entitlement_tys: ::proto_hal::stasis::EntitlementLock<Resource = #entitlements>,
                        )*
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
                                #unresolvable_register_idents: self.#unresolvable_register_idents,
                            )*

                            #(
                                #entitlement_idents: self.#entitlement_idents,
                            )*
                        }
                    }

                    /// Access this register for use.
                    pub fn #use_accessor<T, R>(self, f: impl FnOnce(#ty) -> (R, T)) -> (Block<#(#prev_register_tys,)* R, #(#next_register_tys,)* #(#entitlement_tys,)*>, T)
                    where
                        #(
                            #entitlement_tys: ::proto_hal::stasis::EntitlementLock<Resource = #entitlements>,
                        )*
                    {
                        let (reg, t) = f(self.#ident);

                        (
                            Block {
                                #(
                                    #prev_register_idents: self.#prev_register_idents,
                                )*

                                #ident: reg,

                                #(
                                    #next_register_idents: self.#next_register_idents,
                                )*

                                #(
                                    #unresolvable_register_idents: self.#unresolvable_register_idents,
                                )*

                                #(
                                    #entitlement_idents: self.#entitlement_idents,
                                )*
                            },
                            t,
                        )
                    }
                }
            });
        }

        if !unresolvable_registers.is_empty() {
            body.extend(quote_spanned! { span =>
                impl<#(#resolvable_register_tys,)* #(#entitlement_tys,)*> Block<#(#resolvable_register_tys,)* #(#entitlement_tys,)*>
                {
                    #(
                        pub fn #resolvable_register_idents(&self) -> &#resolvable_register_tys {
                            &self.#resolvable_register_idents
                        }
                    )*

                    #(
                        pub fn #unresolvable_register_idents(&self) -> &#unresolvable_register_idents::Register {
                            &self.#unresolvable_register_idents
                        }
                    )*
                }
            });
        }

        if !self.entitlements.is_empty() {
            body.extend(quote_spanned! { span =>
                impl<#(#resolvable_register_tys,)*> Block<#(#resolvable_register_tys,)* #(#reset_entitlement_tys,)*> {
                    /// Attach to required entitlements, enabling usage of this block.
                    pub fn attach(self, #(#entitlement_idents: #entitlements,)*) -> Block<#(#resolvable_register_tys,)* #(#entitlements,)*> {
                        Block {
                            #(
                                #resolvable_register_idents: self.#resolvable_register_idents,
                            )*

                            #(
                                #unresolvable_register_idents: self.#unresolvable_register_idents,
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
