// use std::collections::HashMap;

// use indexmap::IndexMap;
// use model::structures::{
//     field::{Field, Numericity},
//     hal::Hal,
//     peripheral::Peripheral,
//     register::Register,
// };
// use proc_macro2::TokenStream;
// use quote::{format_ident, quote, quote_spanned};
// use syn::{Expr, Ident, Path, spanned::Spanned};

// use crate::codegen::macros::{
//     Args, BindingArgs, Override, RegisterArgs, StateArgs, get_field, get_register,
// };

// /// A parsed unit of the provided tokens and corresponding model nodes which
// /// represents a single register.
// struct Parsed<'input, 'model> {
//     peripheral: &'model Peripheral,
//     register: &'model Register,
//     items: IndexMap<Ident, (&'model Field, &'input BindingArgs)>,
// }

// fn parse<'input, 'model>(
//     args: &'input Args,
//     model: &'model Hal,
// ) -> (IndexMap<Path, Parsed<'input, 'model>>, Vec<syn::Error>) {
//     let mut out = IndexMap::new();
//     let mut errors = Vec::new();

//     let (registers, e) = parse_registers(args, model);
//     errors.extend(e);

//     for (register_ident, (register_args, peripheral, register)) in registers {
//         let (items, e) = parse_fields(register_args, register);
//         errors.extend(e);

//         out.insert(
//             register_ident.clone(),
//             Parsed {
//                 peripheral,
//                 register,
//                 items,
//             },
//         );
//     }

//     (out, errors)
// }

// /// Lookup peripherals and registers from the model given provided register paths.
// fn parse_registers<'input, 'model>(
//     args: &'input Args,
//     model: &'model Hal,
// ) -> (
//     HashMap<Path, (&'input RegisterArgs, &'model Peripheral, &'model Register)>,
//     Vec<syn::Error>,
// ) {
//     let mut registers = HashMap::new();
//     let mut errors = Vec::new();

//     for register_args in &args.registers {
//         let mut parse_register = || {
//             let (.., peripheral, register) = get_register(&register_args.path, model)?;

//             if let Some(..) = registers.insert(
//                 register_args.path.clone(),
//                 (register_args, peripheral, register),
//             ) {
//                 Err(syn::Error::new_spanned(
//                     &register_args.path,
//                     "register already specified",
//                 ))?
//             }

//             Ok(())
//         };

//         if let Err(e) = parse_register() {
//             errors.push(e);
//         }
//     }

//     (registers, errors)
// }

// /// Lookup fields from a register given provided field idents.
// fn parse_fields<'input, 'model>(
//     register_args: &'input RegisterArgs,
//     register: &'model Register,
// ) -> (
//     IndexMap<Ident, (&'model Field, &'input BindingArgs)>,
//     Vec<syn::Error>,
// ) {
//     let mut items = IndexMap::new();
//     let mut errors = Vec::new();

//     for field_args in &register_args.fields {
//         let mut parse_field = || {
//             let field = get_field(&field_args.ident, register)?;

//             let binding = field_args.binding.as_ref().ok_or(syn::Error::new_spanned(
//                 &field_args.ident,
//                 "expected binding",
//             ))?;

//             if let Some(..) = items.insert(field_args.ident.clone(), (field, binding)) {
//                 Err(syn::Error::new_spanned(
//                     &field_args.ident,
//                     "field already specified",
//                 ))?
//             }

//             Ok(())
//         };

//         if let Err(e) = parse_field() {
//             errors.push(e);
//         }

//         if let Some(transition) = &field_args.transition {
//             errors.push(syn::Error::new(
//                 match &transition.state {
//                     StateArgs::Expr(expr) => expr.span(),
//                     StateArgs::Lit(lit_int) => lit_int.span(),
//                 },
//                 "no transition is accepted",
//             ));
//         }
//     }

//     (items, errors)
// }

// fn validate<'input, 'model>(parsed: &IndexMap<Path, Parsed<'input, 'model>>) -> Vec<syn::Error> {
//     parsed
//         .values()
//         .flat_map(|Parsed { items: fields, .. }| fields.iter())
//         .filter_map(|(ident, (field, ..))| {
//             if field.access.get_read().is_none() {
//                 Some(syn::Error::new_spanned(
//                     ident,
//                     format!("field \"{ident}\" is not readable"),
//                 ))
//             } else {
//                 None
//             }
//         })
//         .collect::<Vec<_>>()
// }

// fn unique_register_ident(peripheral: &Peripheral, register: &Register) -> Ident {
//     format_ident!("{}_{}", peripheral.module_name(), register.module_name(),)
// }

// fn unique_field_ident(peripheral: &Peripheral, register: &Register, field: &Ident) -> Ident {
//     format_ident!(
//         "{}_{}_{}",
//         peripheral.module_name(),
//         register.module_name(),
//         field
//     )
// }

// fn addrs<'input, 'model>(
//     path: &Path,
//     parsed: &Parsed<'input, 'model>,
//     overridden_base_addrs: &HashMap<Ident, Expr>,
// ) -> TokenStream {
//     let register_offset = parsed.register.offset as usize;

//     if let Some(base_addr) = overridden_base_addrs.get(&parsed.peripheral.module_name()) {
//         quote! { (#base_addr + #register_offset) }
//     } else {
//         quote! { #path::ADDR }
//     }
// }

// fn returns(path: &Path, ident: &Ident, field: &Field) -> Option<TokenStream> {
//     Some(match field.access.get_read()?.numericity {
//         Numericity::Numeric => quote! { u32 },
//         Numericity::Enumerated { .. } => quote! {
//             #path::#ident::ReadVariant
//         },
//     })
// }

// fn read_values<'input, 'model>(
//     path: &Path,
//     parsed: &Parsed<'input, 'model>,
//     ident: &Ident,
//     field: &Field,
// ) -> Option<TokenStream> {
//     let reg = unique_register_ident(parsed.peripheral, parsed.register);
//     let mask = u32::MAX >> (32 - field.width);
//     let shift = if field.offset == 0 {
//         None
//     } else {
//         let offset = &field.offset;
//         Some(quote! { >> #offset })
//     };

//     let value = quote! {
//         (#reg #shift) & #mask
//     };

//     Some(match field.access.get_read()?.numericity {
//         Numericity::Numeric => value,
//         Numericity::Enumerated { .. } => quote! {
//             unsafe { #path::#ident::ReadVariant::from_bits(#value) }
//         },
//     })
// }

// fn parameters<'input, 'model>(
//     path: &Path,
//     parsed: &Parsed<'input, 'model>,
//     ident: &Ident,
//     field: &Field,
// ) -> TokenStream {
//     let unique_ident = unique_field_ident(parsed.peripheral, parsed.register, ident);
//     let ty = field.type_name();

//     quote! {
//         #[expect(unused)] #unique_ident: &#path::#ident::#ty<::proto_hal::stasis::Dynamic>
//     }
// }

// pub fn read(model: &Hal, tokens: TokenStream) -> TokenStream {
//     let args = match syn::parse2::<Args>(tokens) {
//         Ok(args) => args,
//         Err(e) => return e.to_compile_error(),
//     };

//     let mut errors = Vec::new();

//     let (parsed, e) = parse(&args, &model);
//     errors.extend(e);
//     errors.extend(validate(&parsed));

//     let mut overridden_base_addrs: HashMap<Ident, Expr> = HashMap::new();

//     for override_ in &args.overrides {
//         match override_ {
//             Override::BaseAddress(ident, expr) => {
//                 overridden_base_addrs.insert(ident.clone(), expr.clone());
//             }
//             Override::CriticalSection(expr) => errors.push(syn::Error::new_spanned(
//                 &expr,
//                 "stand-alone read access is atomic and doesn't require a critical section",
//             )),
//             Override::Unknown(ident) => errors.push(syn::Error::new_spanned(
//                 &ident,
//                 format!("unexpected override \"{}\"", ident),
//             )),
//         };
//     }

//     let suggestions = if errors.is_empty() {
//         None
//     } else {
//         let imports = args
//             .registers
//             .iter()
//             .map(|register| {
//                 let path = &register.path;
//                 let fields = register.fields.iter().map(|field| &field.ident);

//                 let span = path.span();

//                 quote_spanned! { span =>
//                     #[allow(unused_imports)]
//                     use #path::{#(
//                         #fields as _,
//                     )*};
//                 }
//             })
//             .collect::<TokenStream>();
//         Some(imports)
//     };

//     let errors = {
//         let errors = errors.into_iter().map(|e| e.to_compile_error());

//         quote! {
//             #(
//                 #errors
//             )*
//         }
//     };

//     let (reg_idents, addrs, returns, read_values, parameters, bindings) = parsed
//         .iter()
//         .map(|(path, parsed)| {
//             let (returns, read_values, parameters, bindings) = parsed
//                 .items
//                 .iter()
//                 .filter_map(|(ident, (field, binding))| {
//                     Some((
//                         returns(path, ident, field)?,
//                         read_values(path, parsed, ident, field)?,
//                         parameters(path, parsed, ident, field),
//                         binding,
//                     ))
//                 })
//                 .collect::<(Vec<_>, Vec<_>, Vec<_>, Vec<&Expr>)>();

//             (
//                 unique_register_ident(parsed.peripheral, parsed.register),
//                 addrs(path, parsed, &overridden_base_addrs),
//                 returns,
//                 read_values,
//                 parameters,
//                 bindings,
//             )
//         })
//         .collect::<(Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>)>();

//     quote! {
//         #suggestions
//         #errors

//         {
//             fn gate(#(#(#parameters,)*),*) -> (#(#(#returns),*),*) {
//                 #(
//                     let #reg_idents = unsafe {
//                         ::core::ptr::read_volatile(#addrs as *const u32)
//                     };
//                 )*

//                 (
//                     #(#(
//                         #read_values
//                     ),*),*
//                 )
//             }

//             gate(#(#(#bindings),*),*)
//         }
//     }
// }

use std::collections::HashMap;

use model::structures::model::Model;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments,
        utils::{
            render_diagnostics, return_rank::ReturnRank, suggestions, unique_field_ident,
            unique_register_ident,
        },
    },
    parsing::{
        semantic::{
            self, FieldItem, RegisterItem,
            policies::{BindingOnly, ForbidPeripherals},
        },
        syntax::Override,
    },
};

type EntryPolicy<'cx> = BindingOnly<'cx>;
type Input<'cx> = semantic::Gate<'cx, ForbidPeripherals, EntryPolicy<'cx>>;

pub fn read(model: &Model, tokens: TokenStream) -> TokenStream {
    let args = match syn::parse2(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let (input, mut diagnostics) = Input::parse(&args, model);
    diagnostics.extend(validate(&input));

    let mut overridden_base_addrs: HashMap<Ident, Expr> = HashMap::new();

    for override_ in &args.overrides {
        match override_ {
            Override::BaseAddress(ident, expr) => {
                overridden_base_addrs.insert(ident.clone(), expr.clone());
            }
            Override::CriticalSection(expr) => diagnostics.push(
                syn::Error::new_spanned(
                    expr,
                    "stand-alone read access is atomic and doesn't require a critical section",
                )
                .into(),
            ),
            Override::Unknown(ident) => diagnostics.push(
                syn::Error::new_spanned(ident, format!("unexpected override \"{}\"", ident)).into(),
            ),
        };
    }

    let suggestions = suggestions(&args, &diagnostics);
    let errors = render_diagnostics(diagnostics);

    let return_rank = ReturnRank::from_input(&input, |_| true);
    let return_def = fragments::read_return_def(&return_rank);
    let return_ty = fragments::read_return_ty(&return_rank);
    let return_init = fragments::read_return_init(&return_rank);

    let mut reg_idents = Vec::new();
    let mut addrs = Vec::new();
    let mut parameters = Vec::new();
    let mut bindings = Vec::new();

    for register_item in input.visit_registers() {
        reg_idents.push(unique_register_ident(
            register_item.peripheral(),
            register_item.register(),
        ));
        addrs.push(fragments::register_address(
            register_item.peripheral(),
            register_item.register(),
            &overridden_base_addrs,
        ));

        for field_item in register_item.fields().values() {
            parameters.push(make_parameter(register_item, field_item));
            bindings.push(field_item.entry().as_ref());
        }
    }

    let return_ty = return_ty.map(|return_ty| quote! { -> #return_ty });

    quote! {
        #suggestions
        #errors

        {
            #return_def

            fn gate(#(#parameters,)*) #return_ty {
                #(
                    let #reg_idents = unsafe {
                        ::core::ptr::read_volatile(#addrs as *const u32)
                    };
                )*

                #return_init
            }

            gate(#(#bindings,)*)
        }
    }
}

fn validate<'cx>(input: &Input<'cx>) -> Diagnostics {
    input
        .visit_fields()
        .filter_map(|field_item| {
            if !field_item.field().access.is_read() {
                Some(Diagnostic::field_must_be_readable(field_item.ident()))
            } else {
                None
            }
        })
        .collect()
}

fn make_parameter<'cx>(
    register_item: &RegisterItem<'cx, EntryPolicy<'cx>>,
    field_item: &FieldItem<'cx, EntryPolicy<'cx>>,
) -> TokenStream {
    let unique_ident = unique_field_ident(
        register_item.peripheral(),
        register_item.register(),
        field_item.field(),
    );
    let path = register_item.path();
    let ident = field_item.ident();
    let ty = field_item.field().type_name();

    quote! { #unique_ident: &#path::#ident::#ty<::proto_hal::stasis::Dynamic> }
}
