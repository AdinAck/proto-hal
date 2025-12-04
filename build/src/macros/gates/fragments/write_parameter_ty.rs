use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::parsing::syntax::Binding;

pub fn write_parameter_ty(
    binding: &Binding,
    input_ty: &TokenStream,
    value_ty: Option<&TokenStream>,
) -> TokenStream {
    if let Some(ty) = value_ty
        && binding.is_dynamic()
    {
        quote! { (&mut #input_ty, #ty) }
    } else if binding.is_viewed() {
        quote! { &#input_ty }
    } else {
        input_ty.clone()
    }
}
