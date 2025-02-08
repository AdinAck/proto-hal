use std::{
    collections::HashSet,
    ops::{Deref, Range},
};

use darling::FromMeta;
use syn::{ExprRange, Ident, Path};

use crate::utils::{parse_expr_range, Spanned, SynErrorCombinator};

use super::{
    variant::{Variant, VariantArgs},
    Args,
};

#[derive(Debug, Clone, FromMeta)]
pub struct Step(u32);

impl Default for Step {
    fn default() -> Self {
        Self(1)
    }
}

impl Deref for Step {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, FromMeta)]
pub struct VariantArrayArgs {
    pub range: ExprRange,
    #[darling(default)]
    pub step: Step,

    #[darling(flatten)]
    pub state: VariantArgs,
}

impl Args for VariantArrayArgs {
    const NAME: &str = "variant_array";
}

#[derive(Debug)]
pub struct VariantArray {
    pub args: Spanned<VariantArrayArgs>,
    pub ident: Ident,
    pub range: Range<u32>,
    pub step: Step,
    pub bits: u32,
    pub entitlements: HashSet<Path>,
    pub entitlement_fields: HashSet<Ident>,
}

impl VariantArray {
    pub fn parse(ident: Ident, bits: u32, args: Spanned<VariantArrayArgs>) -> syn::Result<Self> {
        let mut errors = SynErrorCombinator::new();

        let bits = args.state.bits.unwrap_or(bits);
        let mut entitlements = HashSet::new();
        let mut entitlement_fields = HashSet::new();

        for entitlement in args.state.entitlements.elems.iter().cloned() {
            entitlement_fields.insert(
                entitlement
                    .segments
                    .iter()
                    .nth_back(1)
                    .unwrap()
                    .ident
                    .clone(),
            );

            if !entitlements.insert(entitlement.clone()) {
                errors.push(syn::Error::new_spanned(
                    entitlement,
                    "entitlement already exists",
                ));
            }
        }

        errors.coalesce()?;

        // TODO: outside of error combinator but whatever
        let range = parse_expr_range(&args.range)?;
        let step = args.step.clone();

        Ok(Self {
            args,
            ident,
            range,
            step,
            bits,
            entitlements,
            entitlement_fields,
        })
    }
}

impl VariantArray {
    pub fn count(&self) -> u32 {
        self.range.clone().count() as _
    }

    pub fn to_states(&self) -> syn::Result<Vec<Variant>> {
        let mut states = Vec::new();
        let mut bits = self.bits;

        let replace_pos = self.ident.to_string().rfind("X").ok_or(syn::Error::new(
            self.ident.span(),
            "state array struct ident must contain an 'X' to indicate replacement location",
        ))?;

        // generate states
        for i in self.range.clone().step_by(*self.step as _) {
            let mut s = self.ident.to_string();
            s.replace_range(replace_pos..replace_pos + 1, &i.to_string());
            let ident = Ident::new(&s, self.ident.span());

            let args = self.args.state.clone().with_span(self.args.span());

            let state = Variant {
                args,
                ident,
                bits,
                entitlements: self.entitlements.clone(),
                entitlement_fields: self.entitlement_fields.clone(),
            };

            bits += 1;
            states.push(state);
        }

        Ok(states)
    }
}
