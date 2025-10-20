use std::collections::HashMap;

use indexmap::IndexMap;
use ir::structures::{
    field::{Field, Numericity},
    hal::Hal,
    peripheral::Peripheral,
    register::Register,
};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{Expr, Ident, Path, spanned::Spanned};

use crate::codegen::macros::{Args, Override, RegisterArgs, StateArgs, get_field, get_register};

/// A parsed unit of the provided tokens and corresponding model nodes which
/// represents a single register.
struct Parsed<'args, 'hal> {
    peripheral: &'hal Peripheral,
    register: &'hal Register,
    items: IndexMap<Ident, (&'hal Field, Option<&'args StateArgs>)>,
}

fn parse<'args, 'hal>(
    args: &'args Args,
    model: &'hal Hal,
) -> (IndexMap<Path, Parsed<'args, 'hal>>, Vec<syn::Error>) {
    let mut out = IndexMap::new();
    let mut errors = Vec::new();

    let (registers, e) = parse_registers(args, model);
    errors.extend(e);

    for (register_ident, (register_args, peripheral, register)) in registers {
        let (items, e) = parse_fields(register_args, register);
        errors.extend(e);

        out.insert(
            register_ident.clone(),
            Parsed {
                peripheral,
                register,
                items,
            },
        );
    }

    (out, errors)
}

/// Lookup peripherals and registers from the model given provided register paths.
fn parse_registers<'args, 'hal>(
    args: &'args Args,
    model: &'hal Hal,
) -> (
    IndexMap<Path, (&'args RegisterArgs, &'hal Peripheral, &'hal Register)>,
    Vec<syn::Error>,
) {
    let mut registers = IndexMap::new();
    let mut errors = Vec::new();

    if args.registers.is_empty() {
        errors.push(syn::Error::new(
            Span::call_site(),
            "at least one register must be specified",
        ));
    }

    for register_args in &args.registers {
        let mut parse_register = || {
            let (.., peripheral, register) = get_register(&register_args.path, model)?;

            if let Some(..) = registers.insert(
                register_args.path.clone(),
                (register_args, peripheral, register),
            ) {
                Err(syn::Error::new_spanned(
                    &register_args.path,
                    "register already specified",
                ))?
            }

            Ok(())
        };

        if let Err(e) = parse_register() {
            errors.push(e);
        }
    }

    (registers, errors)
}

/// Lookup fields from a register given provided field idents and transitions.
fn parse_fields<'args, 'hal>(
    register_args: &'args RegisterArgs,
    register: &'hal Register,
) -> (
    IndexMap<Ident, (&'hal Field, Option<&'args StateArgs>)>,
    Vec<syn::Error>,
) {
    let mut items = IndexMap::new();
    let mut errors = Vec::new();

    if register_args.fields.is_empty() {
        errors.push(syn::Error::new(
            Span::call_site(),
            "at least one field must be specified",
        ));
    }

    for field_args in &register_args.fields {
        let mut parse_field = || {
            let field = get_field(&field_args.ident, register)?;

            let transition = field_args
                .transition
                .as_ref()
                .map(|transition| &transition.state);

            if let Some(..) = items.insert(field_args.ident.clone(), (field, transition)) {
                Err(syn::Error::new_spanned(
                    &field_args.ident,
                    "field already specified",
                ))?
            }

            Ok(())
        };

        if let Err(e) = parse_field() {
            errors.push(e);
        }

        if let Some(binding) = &field_args.binding {
            errors.push(syn::Error::new_spanned(binding, "no binding is accepted"));
        }
    }

    (items, errors)
}

fn validate<'args, 'hal>(parsed: &IndexMap<Path, Parsed<'args, 'hal>>) -> Vec<syn::Error> {
    parsed
        .values()
        .flat_map(|Parsed { items, .. }| items.iter())
        .filter_map(|(ident, (field, transition))| {
            if transition.is_some() && !field.access.is_write() {
                Some(syn::Error::new_spanned(
                    ident,
                    format!("field \"{ident}\" is not writable"),
                ))
            } else if !field.access.is_read() {
                Some(syn::Error::new_spanned(
                    ident,
                    format!("field \"{ident}\" is not readable"),
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

fn unique_register_ident(peripheral: &Peripheral, register: &Register) -> Ident {
    format_ident!("{}_{}", peripheral.module_name(), register.module_name(),)
}

fn unique_field_ident(peripheral: &Peripheral, register: &Register, field: &Ident) -> Ident {
    format_ident!(
        "{}_{}_{}",
        peripheral.module_name(),
        register.module_name(),
        field
    )
}

fn addrs<'args, 'hal>(
    path: &Path,
    parsed: &Parsed<'args, 'hal>,
    overridden_base_addrs: &HashMap<Ident, Expr>,
) -> TokenStream {
    let register_offset = parsed.register.offset as usize;

    if let Some(base_addr) = overridden_base_addrs.get(&parsed.peripheral.module_name()) {
        quote! { (#base_addr + #register_offset) }
    } else {
        quote! { #path::ADDR }
    }
}

fn masks(register: &Register) -> u32 {
    register.fields.values().fold(0, |acc, field| {
        acc | ((u32::MAX >> (32 - field.width)) << field.offset)
    })
}

fn returns(path: &Path, ident: &Ident, field: &Field) -> Option<TokenStream> {
    Some(match field.access.get_read()?.numericity {
        Numericity::Numeric => quote! { u32 },
        Numericity::Enumerated { .. } => quote! {
            #path::#ident::read::Variant
        },
    })
}

fn read_values<'args, 'hal>(
    path: &Path,
    parsed: &Parsed<'args, 'hal>,
    ident: &Ident,
    field: &Field,
) -> Option<TokenStream> {
    let reg = unique_register_ident(parsed.peripheral, parsed.register);
    let mask = u32::MAX >> (32 - field.width);

    let value = quote! {
        (#reg >> #path::#ident::OFFSET) & #mask
    };

    Some(match field.access.get_read()?.numericity {
        Numericity::Numeric => value,
        Numericity::Enumerated { .. } => quote! {
            unsafe { #path::#ident::read::Variant::from_bits(#value) }
        },
    })
}

fn write_values(
    path: &Path,
    transition: &StateArgs,
    ident: &Ident,
    field: &Field,
) -> Option<TokenStream> {
    Some(match (transition, &field.access.get_write()?.numericity) {
        (StateArgs::Expr(expr), Numericity::Enumerated { .. }) => {
            quote! {{
                #[allow(unused_imports)]
                use #path::#ident::write::Variant::*;
                (#expr) as u32
            }}
        }
        (StateArgs::Expr(expr), ..) => {
            quote! {{
                (#expr) as u32
            }}
        }
        (StateArgs::Lit(lit_int), ..) => quote! { #lit_int },
    })
}

pub fn modify_untracked(model: &Hal, tokens: TokenStream) -> TokenStream {
    let args = match syn::parse2::<Args>(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let mut errors = Vec::new();

    let (parsed, e) = parse(&args, &model);
    errors.extend(e);
    errors.extend(validate(&parsed));

    let mut overridden_base_addrs: HashMap<Ident, Expr> = HashMap::new();
    let mut cs = None;

    for override_ in &args.overrides {
        match override_ {
            Override::BaseAddress(ident, expr) => {
                overridden_base_addrs.insert(ident.clone(), expr.clone());
            }
            Override::CriticalSection(expr) => {
                cs.replace(quote! {
                    #expr;
                });
            }
            Override::Unknown(ident) => errors.push(syn::Error::new_spanned(
                &ident,
                format!("unexpected override \"{}\"", ident),
            )),
        };
    }

    let suggestions = if errors.is_empty() {
        None
    } else {
        let imports = args
            .registers
            .iter()
            .map(|register| {
                let path = &register.path;
                let fields = register.fields.iter().map(|field| &field.ident);

                let span = path.span();

                quote_spanned! { span =>
                    #[allow(unused_imports)]
                    use #path::{#(
                        #fields as _,
                    )*};
                }
            })
            .collect::<TokenStream>();

        let consts = args
            .overrides
            .iter()
            .filter_map(|override_| {
                let Override::Unknown(ident) = override_ else {
                    None?
                };

                let span = ident.span();

                Some(quote_spanned! { span =>
                    #[allow(unused)]
                    mod critical_section {}

                    #[allow(unused_imports)]
                    use #ident as _;
                })
            })
            .collect::<TokenStream>();

        Some(quote! {
            #imports
            #consts
        })
    };

    let errors = {
        let errors = errors.into_iter().map(|e| e.to_compile_error());

        quote! {
            #(
                #errors
            )*
        }
    };

    // read items
    let (read_reg_idents, read_addrs, returns, read_values, read_field_idents) = parsed
        .iter()
        .map(|(path, parsed)| {
            let (returns, read_values, read_field_idents) = parsed
                .items
                .iter()
                .filter_map(|(ident, (field, ..))| {
                    Some((
                        returns(path, ident, field)?,
                        read_values(path, parsed, ident, field)?,
                        unique_field_ident(parsed.peripheral, parsed.register, ident),
                    ))
                })
                .collect::<(Vec<_>, Vec<_>, Vec<_>)>();

            (
                unique_register_ident(parsed.peripheral, parsed.register),
                addrs(path, parsed, &overridden_base_addrs),
                returns,
                read_values,
                read_field_idents,
            )
        })
        .collect::<(Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>)>();

    // write items
    let (
        write_reg_idents,
        write_addrs,
        masks,
        parameter_idents,
        parameter_tys,
        write_values,
        write_offsets,
    ) = parsed
        .iter()
        .map(|(path, parsed)| {
            let (parameter_idents, parameter_tys, write_values, offsets) = parsed
                .items
                .iter()
                .filter_map(|(ident, (field, transition))| {
                    Some((
                        unique_field_ident(parsed.peripheral, parsed.register, ident),
                        quote! { u32 },
                        write_values(path, (*transition)?, ident, field)?,
                        quote! { #path::#ident::OFFSET },
                    ))
                })
                .collect::<(Vec<_>, Vec<_>, Vec<_>, Vec<_>)>();

            (
                unique_register_ident(parsed.peripheral, parsed.register),
                addrs(path, parsed, &overridden_base_addrs),
                masks(&parsed.register),
                parameter_idents,
                parameter_tys,
                write_values,
                offsets,
            )
        })
        .collect::<(Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>)>();

    let body = quote! {
        #cs

        unsafe fn read() -> (#(u32, #(#returns),*)*) {
            #(
                let #read_reg_idents = unsafe {
                    ::core::ptr::read_volatile(#read_addrs as *const u32)
                };
            )*

            (
                #(
                    #read_reg_idents,
                    #(
                        #read_values,
                    )*
                )*
            )
        }

        unsafe fn write(#(#write_reg_idents: u32,)* #(#(#parameter_idents: #parameter_tys,)*)*) {
            #(
                unsafe {
                    ::core::ptr::write_volatile(
                        #write_addrs as *mut u32,
                        #write_reg_idents & !#masks #(
                            | (#parameter_idents << #write_offsets)
                        )*
                    )
                };
            )*
        }

        #[allow(unused)]
        let (#(
            #read_reg_idents,
            #(
                #read_field_idents,
            )*
        )*) = read();

        write(#(
            #write_reg_idents,
            #(
                #write_values,
            )*
        )*);
    };

    let body = if cs.is_none() {
        quote! {
            ::proto_hal::critical_section::with(|_| {
                #body
            })
        }
    } else {
        quote! {{ #body }}
    };

    quote! {
        #suggestions
        #errors
        #body
    }
}
