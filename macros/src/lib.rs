#[proc_macro]
pub fn generate_macros(args: proc_macro::TokenStream) -> proc_macro::TokenStream {
    build::macros::reexports(args.into()).into()
}
