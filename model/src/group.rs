use derive_more::{AsRef, Deref, From};
use heck::ToSnakeCase as _;
use indexmap::IndexMap;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;
use ters::ters;

use crate::{
    Node, field::FieldIndex, model::View, peripheral::PeripheralIndex, register::RegisterIndex,
};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Deref, From)]
pub struct PeripheralGroupIndex(pub(super) Ident);
#[derive(Debug, Clone, Hash, PartialEq, Eq, Deref, From)]
pub struct RegisterGroupIndex(pub(super) Ident);
#[derive(Debug, Clone, Hash, PartialEq, Eq, Deref, From)]
pub struct FieldGroupIndex(pub(super) Ident);

#[ters]
#[derive(Debug, Clone, Deref, AsRef)]
pub struct GroupNode<P, C> {
    pub(super) parent: P,
    #[deref]
    #[as_ref]
    pub(super) group: Group,
    #[get]
    pub(super) members: IndexMap<Ident, C>,
}

pub type PeripheralGroupNode = GroupNode<(), PeripheralIndex>;
pub type RegisterGroupNode = GroupNode<PeripheralIndex, RegisterIndex>;
pub type FieldGroupNode = GroupNode<RegisterIndex, FieldIndex>;

impl Node for PeripheralGroupNode {
    type Index = PeripheralGroupIndex;
}

impl Node for RegisterGroupNode {
    type Index = RegisterGroupIndex;
}

impl Node for FieldGroupNode {
    type Index = FieldGroupIndex;
}

#[derive(Debug, Clone)]
pub struct Group {
    pub ident: Ident,
}

impl Group {
    pub fn module_name(&self) -> Ident {
        Ident::new(&self.ident.to_string().to_snake_case(), Span::call_site())
    }
}

impl<'cx> View<'cx, PeripheralGroupNode> {
    pub fn generate(&self) -> TokenStream {
        let module = self.module_name();
        let members = self
            .members
            .values()
            .map(|member| self.model.get_peripheral(member.clone()).generate());

        quote! {
            pub mod #module {
                #(#members)*
            }
        }
    }
}

impl<'cx> View<'cx, RegisterGroupNode> {
    pub fn generate(&self) -> TokenStream {
        let module = self.module_name();
        let members = self
            .members
            .values()
            .map(|member| self.model.get_register(*member).generate());

        quote! {
            pub mod #module {
                #(#members)*
            }
        }
    }
}

impl<'cx> View<'cx, FieldGroupNode> {
    pub fn generate(&self) -> TokenStream {
        let module = self.module_name();
        let members = self
            .members
            .values()
            .map(|member| self.model.get_field(*member).generate());

        quote! {
            pub mod #module {
                #(#members)*
            }
        }
    }
}
