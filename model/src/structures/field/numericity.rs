use indexmap::IndexMap;
use syn::{Ident, parse_quote};

use crate::structures::{
    model::{Model, View},
    variant::{Variant, VariantIndex, VariantNode},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Numericity {
    Numeric(Numeric),
    Enumerated(Enumerated),
}

impl Default for Numericity {
    fn default() -> Self {
        Self::Numeric(Numeric)
    }
}

impl Numericity {
    pub(in crate::structures) fn add_child(&mut self, variant: &Variant, index: VariantIndex) {
        match self {
            Numericity::Numeric(..) => {
                *self = Numericity::Enumerated(Enumerated {
                    variants: IndexMap::from([(variant.module_name(), index)]),
                })
            }
            Numericity::Enumerated(enumerated) => {
                enumerated.variants.insert(variant.module_name(), index);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Numeric;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enumerated {
    pub variants: IndexMap<Ident, VariantIndex>,
}

impl Numeric {
    pub fn ty(&self, width: u8) -> (Ident, Ident) {
        match width {
            1 => (parse_quote! { bool }, parse_quote! { Bool }),
            2..9 => (parse_quote! { u8 }, parse_quote! { UInt8 }),
            9..17 => (parse_quote! { u16 }, parse_quote! { UInt16 }),
            17..33 => (parse_quote! { u32 }, parse_quote! { UInt32 }),
            _unreachable => unreachable!("fields cannot be greater than 32 bits wide"),
        }
    }
}

impl Enumerated {
    pub fn variants<'cx>(&self, model: &'cx Model) -> impl Iterator<Item = View<'cx, VariantNode>> {
        self.variants
            .values()
            .map(|index| model.get_variant(*index))
    }

    /// View an inert variant if one exists. If there is more than one, the variant returned
    /// is not guaranteed to be any particular one, nor consistent. If the numericity is
    /// [`Numeric`](Numericity::Numeric), [`None`] is returned.
    pub fn some_inert<'cx>(&self, model: &'cx Model) -> Option<&'cx Variant> {
        self.variants(model)
            .map(|view| &view.variant)
            .find(|variant| variant.inert)
    }
}
