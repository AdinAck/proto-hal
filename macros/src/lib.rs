use darling::{ast::NestedMeta, FromMeta};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use structures::{
    block::{Block, BlockArgs, BlockSpec},
    interrupts::InterruptsSpec,
    Args,
};
use syn::{parse2, ItemEnum, ItemMod};
use tiva::Validate;

fn block_inner(args: TokenStream, item: TokenStream) -> Result<TokenStream2, syn::Error> {
    let block_args = BlockArgs::from_list(&NestedMeta::parse_meta_list(args.into())?)?
        .with_span(Span::call_site());

    let module = parse2::<ItemMod>(item.into())?;

    let block: Block = BlockSpec::parse(
        module.ident.clone(),
        module.vis.clone(),
        block_args,
        utils::extract_items_from(&module)?.iter(),
    )?
    .validate()?;

    Ok(quote! {
        #block
    })
}

#[proc_macro_attribute]
pub fn block(args: TokenStream, item: TokenStream) -> TokenStream {
    match block_inner(args, item) {
        Ok(tokens) => tokens,
        Err(e) => e.to_compile_error(),
    }
    .into()
}

fn interrupts_inner(_args: TokenStream, item: TokenStream) -> Result<TokenStream2, syn::Error> {
    let e = parse2::<ItemEnum>(item.into())?;

    let interrupts = InterruptsSpec::parse(&e)?;

    Ok(quote! {
        #interrupts
    })
}

/// Define the interrupt vector table
/// in accordance to the `cortex-m-rt` spec.
#[proc_macro_attribute]
pub fn interrupts(args: TokenStream, item: TokenStream) -> TokenStream {
    match interrupts_inner(args, item) {
        Ok(tokens) => tokens,
        Err(e) => e.to_compile_error(),
    }
    .into()
}
