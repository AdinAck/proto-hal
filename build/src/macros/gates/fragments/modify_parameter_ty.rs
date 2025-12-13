use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::parsing::syntax::Binding;

pub fn modify_parameter_ty(
    binding: &Binding,
    input_ty: &TokenStream,
    write_value_ty: Option<&TokenStream>,
    return_ty: Option<&TokenStream>,
) -> TokenStream {
    if let Some(ty) = write_value_ty
        && binding.is_dynamic()
    {
        quote! { (&mut #input_ty, impl FnOnce(#return_ty) -> #ty) }
    } else if binding.is_dynamic() {
        quote! { &mut #input_ty }
    } else if binding.is_viewed() {
        quote! { &#input_ty }
    } else {
        input_ty.clone()
    }
}
