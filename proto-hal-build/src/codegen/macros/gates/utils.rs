use ir::structures::{peripheral::Peripheral, register::Register};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{Ident, spanned::Spanned as _};

use crate::codegen::macros::{
    diagnostic::Diagnostics,
    parsing::semantic::{
        self, FieldEntryRefinementInput,
        policies::{Filter, Refine},
    },
};

pub fn unique_register_ident(peripheral: &Peripheral, register: &Register) -> Ident {
    format_ident!("{}_{}", peripheral.module_name(), register.module_name(),)
}

pub fn render_diagnostics(diagnostics: Diagnostics) -> TokenStream {
    let errors = diagnostics
        .into_iter()
        .map(|e| syn::Error::from(e).to_compile_error());

    quote! {
        #(
            #errors
        )*
    }
}

pub fn suggestions<'cx, PeripheralPolicy, EntryPolicy>(
    input: &semantic::Gate<'cx, PeripheralPolicy, EntryPolicy>,
    diagnostics: &Diagnostics,
) -> Option<TokenStream>
where
    PeripheralPolicy: Filter,
    EntryPolicy: Refine<'cx, Input = FieldEntryRefinementInput<'cx>> + 'cx,
{
    if diagnostics.is_empty() {
        None
    } else {
        let imports = input
            .visit_registers()
            .map(|register| {
                let path = &register.peripheral_path();
                let fields = register.fields().values().map(|field| field.ident());

                let span = path.span();

                quote_spanned! { span =>
                    #[allow(unused_imports)]
                    use #path::{#(
                        #fields as _,
                    )*};
                }
            })
            .collect::<TokenStream>();
        Some(imports)
    }
}
