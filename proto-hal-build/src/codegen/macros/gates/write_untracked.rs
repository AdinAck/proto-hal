// use std::collections::HashMap;

// use indexmap::IndexMap;
// use ir::structures::{
//     field::{Field, Numericity},
//     hal::Hal,
//     peripheral::Peripheral,
//     register::Register,
// };
// use proc_macro2::TokenStream;
// use quote::{ToTokens, format_ident, quote, quote_spanned};
// use syn::{Expr, Ident, Path, spanned::Spanned};

// use crate::codegen::macros::{Args, Override, RegisterArgs, StateArgs, get_field, get_register};

// enum Scheme {
//     FromZero,
//     FromReset,
// }

// /// A parsed unit of the provided tokens and corresponding model nodes which
// /// represents a single register.
// struct Parsed<'input, 'model> {
//     peripheral: &'model Peripheral,
//     register: &'model Register,
//     items: IndexMap<Ident, (&'model Field, &'input StateArgs)>,
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
//     IndexMap<Path, (&'input RegisterArgs, &'model Peripheral, &'model Register)>,
//     Vec<syn::Error>,
// ) {
//     let mut registers = IndexMap::new();
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

// /// Lookup fields from a register given provided field idents and transitions.
// fn parse_fields<'input, 'model>(
//     register_args: &'input RegisterArgs,
//     register: &'model Register,
// ) -> (
//     IndexMap<Ident, (&'model Field, &'input StateArgs)>,
//     Vec<syn::Error>,
// ) {
//     let mut items = IndexMap::new();
//     let mut errors = Vec::new();

//     for field_args in &register_args.fields {
//         let mut parse_field = || {
//             let field = get_field(&field_args.ident, register)?;

//             let transition = field_args
//                 .transition
//                 .as_ref()
//                 .map(|transition| &transition.state)
//                 .ok_or(syn::Error::new_spanned(
//                     &field_args.ident,
//                     "expected transition",
//                 ))?;

//             if let Some(..) = items.insert(field_args.ident.clone(), (field, transition)) {
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

//         if let Some(binding) = &field_args.binding {
//             errors.push(syn::Error::new_spanned(binding, "no binding is accepted"));
//         }
//     }

//     (items, errors)
// }

// fn validate<'input, 'model>(parsed: &IndexMap<Path, Parsed<'input, 'model>>) -> Vec<syn::Error> {
//     parsed
//         .values()
//         .flat_map(|Parsed { items, .. }| items.iter())
//         .filter_map(|(ident, (field, ..))| {
//             if field.access.get_write().is_none() {
//                 Some(syn::Error::new_spanned(
//                     ident,
//                     format!("field \"{ident}\" is not writable"),
//                 ))
//             } else {
//                 None
//             }
//         })
//         .collect::<Vec<_>>()
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

// fn initials<'input, 'model>(scheme: &Scheme, parsed: &Parsed<'input, 'model>) -> u32 {
//     match scheme {
//         Scheme::FromZero => 0,
//         Scheme::FromReset => {
//             let mask = parsed.items.values().fold(0, |acc, (field, ..)| {
//                 acc | ((u32::MAX >> (32 - field.width)) << field.offset)
//             });

//             parsed.register.reset.unwrap_or(0) & !mask
//         }
//     }
// }

// fn write_values(
//     path: &Path,
//     transition: &StateArgs,
//     ident: &Ident,
//     field: &Field,
// ) -> Option<TokenStream> {
//     Some(match (transition, &field.access.get_write()?.numericity) {
//         (StateArgs::Expr(expr), Numericity::Enumerated { .. }) => {
//             quote! {{
//                 #[allow(unused_imports)]
//                 use #path::#ident::write::Variant::*;
//                 #expr as u32
//             }}
//         }
//         (StateArgs::Expr(expr), ..) => {
//             quote! {{
//                 #expr as u32
//             }}
//         }
//         (StateArgs::Lit(lit_int), ..) => quote! { #lit_int },
//     })
// }

// fn write_untracked(scheme: Scheme, model: &Hal, tokens: TokenStream) -> TokenStream {
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

//     let (addrs, initials, parameter_idents, parameter_tys, write_values, offsets) = parsed
//         .iter()
//         .map(|(path, parsed)| {
//             let (parameter_idents, parameter_tys, write_values, offsets) = parsed
//                 .items
//                 .iter()
//                 .filter_map(|(ident, (field, transition))| {
//                     Some((
//                         unique_field_ident(parsed.peripheral, parsed.register, ident),
//                         quote! { u32 },
//                         write_values(path, transition, ident, field)?,
//                         field.offset.to_token_stream(),
//                     ))
//                 })
//                 .collect::<(Vec<_>, Vec<_>, Vec<_>, Vec<_>)>();

//             (
//                 addrs(path, parsed, &overridden_base_addrs),
//                 initials(&scheme, parsed),
//                 parameter_idents,
//                 parameter_tys,
//                 write_values,
//                 offsets,
//             )
//         })
//         .collect::<(Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>)>();

//     quote! {
//         #suggestions
//         #errors

//         {
//             unsafe fn gate(#(#(#parameter_idents: #parameter_tys),*),*) {
//                 #(
//                     unsafe {
//                         ::core::ptr::write_volatile(
//                             #addrs as *mut u32,
//                             #initials #(
//                                 | (#parameter_idents << #offsets)
//                             )*
//                         )
//                     };
//                 )*
//             }

//             gate(#(#(#write_values),*),*)
//         }
//     }
// }

// pub fn write_from_zero_untracked(model: &Hal, tokens: TokenStream) -> TokenStream {
//     write_untracked(Scheme::FromZero, model, tokens)
// }

// pub fn write_from_reset_untracked(model: &Hal, tokens: TokenStream) -> TokenStream {
//     write_untracked(Scheme::FromReset, model, tokens)
// }

use std::collections::HashMap;

use ir::structures::hal::Hal;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments::{register_address, write_value},
        utils::{render_diagnostics, suggestions, unique_field_ident},
    },
    parsing::{
        semantic::{
            self,
            policies::{ForbidPeripherals, TransitionOnly},
        },
        syntax::Override,
    },
};

enum Scheme {
    FromZero,
    FromReset,
}

type Input<'cx> = semantic::Gate<'cx, ForbidPeripherals, TransitionOnly<'cx>>;

pub fn write_from_zero_untracked(model: &Hal, tokens: TokenStream) -> TokenStream {
    write_untracked(Scheme::FromZero, model, tokens)
}

pub fn write_from_reset_untracked(model: &Hal, tokens: TokenStream) -> TokenStream {
    write_untracked(Scheme::FromReset, model, tokens)
}

fn write_untracked(scheme: Scheme, model: &Hal, tokens: TokenStream) -> TokenStream {
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
                    &expr,
                    "stand-alone read access is atomic and doesn't require a critical section",
                )
                .into(),
            ),
            Override::Unknown(ident) => diagnostics.push(
                syn::Error::new_spanned(&ident, format!("unexpected override \"{}\"", ident))
                    .into(),
            ),
        };
    }

    let suggestions = suggestions(&args, &diagnostics);
    let errors = render_diagnostics(diagnostics);

    let parameter_idents = Vec::new();
    let mut addrs = Vec::new();
    let mut write_values = Vec::new();

    for register_item in input.visit_registers() {
        let register_path = register_item.path();

        addrs.push(register_address(
            register_item.peripheral(),
            register_item.register(),
            &overridden_base_addrs,
        ));

        for field_item in register_item.fields().values() {
            parameter_idents.push(unique_field_ident(
                register_item.peripheral(),
                register_item.register(),
                field_item.field(),
            ));

            write_values.push(write_value(
                &register_path,
                field_item.ident(),
                field_item.entry(),
            ));
        }
    }

    quote! {
        #suggestions
        #errors

        {
            unsafe fn gate(#(#parameter_idents: u32),*) {
                #(
                    unsafe {
                        ::core::ptr::write_volatile(
                            #addrs as *mut u32,
                            #reg_write_values
                        )
                    };
                )*
            }

            gate(#(#write_values),*)
        }
    }
}

fn validate<'cx>(input: &Input<'cx>) -> Diagnostics {
    input
        .visit_fields()
        .filter_map(|field_item| {
            if !field_item.field().access.is_write() {
                Some(Diagnostic::field_must_be_writable(field_item.ident()))
            } else {
                None
            }
        })
        .collect()
}
