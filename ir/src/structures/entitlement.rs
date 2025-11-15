use indexmap::IndexSet;
use proc_macro2::TokenStream;
use quote::quote;

use crate::structures::{
    field::{FieldIndex, FieldNode},
    hal::Hal,
    peripheral::PeripheralIndex,
    variant::VariantIndex,
};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Entitlement(pub(super) VariantIndex);

impl Entitlement {
    pub fn field<'cx>(&self, model: &'cx Hal) -> &'cx FieldNode {
        let variant = model.get_variant(self.0);
        model.get_field(variant.parent)
    }

    pub fn render_up_to_field(&self, model: &Hal) -> TokenStream {
        let field = self.field(model);
        let register = model.get_register(field.parent);
        let peripheral = model.get_peripheral(&register.parent);

        let peripheral_ident = peripheral.module_name();
        let register_ident = register.module_name();
        let field_ident = field.module_name();

        quote! {
            #peripheral_ident::#register_ident::#field_ident
        }
    }

    pub fn render_entirely(&self, model: &Hal) -> TokenStream {
        let prefix = self.render_up_to_field(model);
        let variant = model.get_variant(self.0);

        let variant_ident = variant.type_name();

        quote! {
            #prefix::#variant_ident
        }
    }
}

pub type Entitlements = IndexSet<Entitlement>;

#[derive(Debug, Clone)]
pub enum EntitlementKey {
    Peripheral(PeripheralIndex),
    Field(FieldIndex),
    Affordance(FieldIndex),
    Variant(VariantIndex),
}
