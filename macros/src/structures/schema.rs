use std::{collections::HashSet, ops::Deref};

use darling::FromMeta;
use syn::{Ident, Item};

use super::{
    entitlement::Entitlement, variant::{Variant, VariantArgs}, variant_array::{VariantArray, VariantArrayArgs}, Args
};
use crate::utils::{require_struct, Spanned, SynErrorCombinator, Width};
use tiva::Validator;

#[derive(Debug, Clone, Default, FromMeta)]
pub struct SchemaArgs {
    #[darling(default)]
    pub auto_increment: bool,
    pub width: Width,
}

impl Args for SchemaArgs {
    const NAME: &str = "schema";
}

#[derive(Debug, Clone, PartialEq)]
pub enum Numericity {
    Numeric,
    Enumerated { variants: Vec<Variant> },
}

impl Numericity {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Numeric)
    }

    pub fn is_enumerated(&self) -> bool {
        matches!(self, Self::Enumerated { variants: _ })
    }
}

#[derive(Debug, Clone)]
pub struct SchemaSpec {
    pub args: Spanned<SchemaArgs>,
    pub ident: Ident,
    pub width: Width,
    pub entitlements: HashSet<Entitlement>,

    // computed properties
    pub numericity: Numericity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    spec: SchemaSpec,
}

impl Deref for Schema {
    type Target = SchemaSpec;

    fn deref(&self) -> &Self::Target {
        &self.spec
    }
}

impl SchemaSpec {
    pub fn parse<'a>(
        ident: Ident,
        args: Spanned<SchemaArgs>,
        items: impl Iterator<Item = &'a Item>,
    ) -> syn::Result<Self> {
        let mut errors = SynErrorCombinator::new();

        let width = args.width;
        let mut variants = Vec::new();
        let mut entitlements = HashSet::new();

        let mut state_bits = 0u32;

        for item in items {
            let s = require_struct(item)?;

            let get_args = || {
                Ok::<_, syn::Error>((
                    VariantArgs::get(s.attrs.iter())?,
                    VariantArrayArgs::get(s.attrs.iter())?,
                ))
            };

            errors.try_maybe_then(get_args(), |arg_collection| {
                let arg_entitlements = match arg_collection {
                    (Some(state_args), None) => {
                        let state =
                            Variant::parse(s.ident.clone(), state_bits, state_args.clone())?;

                        state_bits = state.bits + 1;
                        variants.push(state);

                        Ok(state_args.entitlements.elems.clone())
                    }
                    (None, Some(state_array_args)) => {
                        let state_array = VariantArray::parse(
                            s.ident.clone(),
                            state_bits,
                            state_array_args.clone(),
                        )?;

                        state_bits = state_array.bits + state_array.count();
                        variants.extend(state_array.to_states()?);

                        Ok(state_array_args.state.entitlements.elems.clone())
                    }
                    (None, None) => Err(syn::Error::new_spanned(s, "extraneous item")),
                    (Some(state_args), Some(state_array_args)) => {
                        let msg = "only one struct annotation is permitted";

                        let mut e = syn::Error::new(state_args.span(), msg);
                        e.combine(syn::Error::new(state_array_args.span(), msg));

                        Err(e)
                    }
                }?;

                let mut errors = SynErrorCombinator::new();

                // collect fields of state entitlements (specified in state args)
                for entitlement in &arg_entitlements {
                    errors.maybe_then(Entitlement::from_path(&entitlement), |entitlement| {
                        entitlements.insert(entitlement);
                    });
                }

                errors.coalesce()
            });
        }

        errors.coalesce()?;

        Ok(Self {
            args,
            ident,
            width,
            entitlements,
            numericity: if variants.is_empty() {
                Numericity::Numeric
            } else {
                Numericity::Enumerated { variants }
            },
        })
    }
}

impl PartialEq for SchemaSpec {
    fn eq(&self, other: &Self) -> bool {
        self.numericity == other.numericity
    }
}

impl Validator<SchemaSpec> for Schema {
    type Error = syn::Error;

    fn validate(spec: SchemaSpec) -> Result<Self, Self::Error> {
        let mut errors = SynErrorCombinator::new();

        if let Numericity::Enumerated { variants } = &spec.numericity {
            for variant in variants {
                if variant.args.bits.is_none() && !spec.args.auto_increment {
                    errors.push(syn::Error::new(variant.args.span(), "state bit value `bits` must be specified. to infer the bit value, use `auto_increment`"));
                }

                if variant.bits >> spec.width != 0 {
                    errors.push(syn::Error::new(
                        variant.args.span(),
                        "state bit value does not fit within field width",
                    ));
                }
            }
        }

        errors.coalesce()?;

        Ok(Self { spec })
    }
}
