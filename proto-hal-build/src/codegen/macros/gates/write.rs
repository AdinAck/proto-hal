use std::collections::HashMap;

use indexmap::{IndexMap, IndexSet};
use inflector::Inflector;
use ir::structures::{
    field::{Field, Numericity},
    hal::Hal,
    peripheral::Peripheral,
    register::Register,
    variant::Variant,
};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{Expr, Ident, LitInt, Path, spanned::Spanned};

use crate::codegen::macros::{
    Args, BindingArgs, Override, RegisterArgs, StateArgs, get_field, get_register,
};

type FieldItem<'args, 'hal> = (
    &'hal Field,
    &'args BindingArgs,
    Option<(&'args StateArgs, WriteState<'args, 'hal>)>,
);

/// A parsed unit of the provided tokens and corresponding model nodes which
/// represents a single register.
struct Parsed<'args, 'hal> {
    prefix: Option<Path>,
    peripheral: &'hal Peripheral,
    register: &'hal Register,
    items: IndexMap<Ident, FieldItem<'args, 'hal>>,
}

enum WriteState<'args, 'hal> {
    Variant(&'hal Variant),
    Expr(&'args Expr),
    Lit(&'args LitInt),
}

fn parse<'args, 'hal>(
    args: &'args Args,
    model: &'hal Hal,
) -> (IndexMap<Path, Parsed<'args, 'hal>>, Vec<syn::Error>) {
    let mut out = IndexMap::new();
    let mut errors = Vec::new();

    let (registers, e) = parse_registers(args, model);
    errors.extend(e);

    for (register_ident, (prefix, register_args, peripheral, register)) in registers {
        let (items, e) = parse_fields(register_args, register);
        errors.extend(e);

        out.insert(
            register_ident.clone(),
            Parsed {
                prefix,
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
    IndexMap<
        Path,
        (
            Option<Path>,
            &'args RegisterArgs,
            &'hal Peripheral,
            &'hal Register,
        ),
    >,
    Vec<syn::Error>,
) {
    let mut registers = IndexMap::new();
    let mut errors = Vec::new();

    for register_args in &args.registers {
        let mut parse_register = || {
            let (prefix, peripheral, register) = get_register(&register_args.path, model)?;

            if let Some(..) = registers.insert(
                register_args.path.clone(),
                (prefix, register_args, peripheral, register),
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
) -> (IndexMap<Ident, FieldItem<'args, 'hal>>, Vec<syn::Error>) {
    let mut items = IndexMap::new();
    let mut errors = Vec::new();

    for field_args in &register_args.fields {
        let mut parse_field = || {
            let field = get_field(&field_args.ident, register)?;

            let binding = field_args.binding.as_ref().ok_or(syn::Error::new_spanned(
                &field_args.ident,
                "expected binding",
            ))?;

            let transition = field_args
                .transition
                .as_ref()
                .map(|transition| &transition.state);

            let transition_and_write_state = match transition {
                Some(transition) => Some((
                    transition,
                    match (
                        transition,
                        &field
                            .access
                            .get_write()
                            .ok_or(syn::Error::new_spanned(
                                &field_args.ident,
                                format!("field {} must be writable", field_args.ident),
                            ))?
                            .numericity,
                    ) {
                        (
                            StateArgs::Expr(expr @ Expr::Path(path)),
                            Numericity::Enumerated { variants },
                        ) => {
                            let ident = path.path.require_ident()?;
                            if let Some(variant) = variants
                                .values()
                                .find(|variant| &variant.type_name() == ident)
                            {
                                WriteState::Variant(variant)
                            } else {
                                WriteState::Expr(expr)
                            }
                        }
                        (StateArgs::Expr(expr), Numericity::Enumerated { .. }) => {
                            WriteState::Expr(expr)
                        }
                        (StateArgs::Expr(expr), Numericity::Numeric) => WriteState::Expr(expr),
                        (StateArgs::Lit(lit_int), Numericity::Enumerated { variants }) => {
                            let bits = lit_int.base10_parse::<u32>()?;
                            let variant = variants
                                .values()
                                .find(|variant| variant.bits == bits)
                                .ok_or(syn::Error::new_spanned(
                                    &lit_int,
                                    format!(
                                        "value {} has no corresponding variant in field {}",
                                        lit_int, field_args.ident
                                    ),
                                ))?;
                            WriteState::Variant(variant)
                        }
                        (StateArgs::Lit(lit_int), Numericity::Numeric) => WriteState::Lit(lit_int),
                    },
                )),
                None => None,
            };

            if let Some(..) = items.insert(
                field_args.ident.clone(),
                (field, binding, transition_and_write_state),
            ) {
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
    }

    (items, errors)
}

fn validate<'args, 'hal>(parsed: &IndexMap<Path, Parsed<'args, 'hal>>) -> Vec<syn::Error> {
    parsed
        .values()
        .flat_map(|Parsed { items, .. }| items.iter())
        .flat_map(|(field_ident, (field, ..))| {
            let Some(write) = field.access.get_write() else {
                return vec![syn::Error::new_spanned(
                    field_ident,
                    format!("field \"{field_ident}\" is not writable"),
                )];
            };

            let access_entitlements = write.entitlements.iter().map(|entitlement| (entitlement.peripheral(), entitlement.register(), entitlement.field()));
            let statewise_entitlements = match &write.numericity {
                Numericity::Numeric => None,
                Numericity::Enumerated { variants } => Some(
                    variants
                        .values()
                        .flat_map(|variant| variant.entitlements.iter().map(|entitlement| (entitlement.peripheral(), entitlement.register(), entitlement.field())))
                        .collect::<IndexSet<_>>(),
                ),
            }
            .into_iter()
            .flatten();

            let mut errors = Vec::new();

            for (peripheral_entitlement, register_entitlement, field_entitlement) in access_entitlements.chain(statewise_entitlements) {
                if query_field(parsed, peripheral_entitlement, register_entitlement, field_entitlement).is_none() {
                    errors.push(syn::Error::new_spanned(
                        &field_ident,
                        format!(
                            "field \"{field_ident}\" is entitled to at least one state in \"{peripheral_entitlement}::{register_entitlement}::{field_entitlement}\" which must be provided",
                        )
                    ));
                }
            }

            errors
        })
        .collect::<Vec<_>>()
}

fn query_field<'args, 'hal, 'parsed>(
    parsed: &'parsed IndexMap<Path, Parsed<'args, 'hal>>,
    peripheral_ident: &Ident,
    register_ident: &Ident,
    field_ident: &Ident,
) -> Option<(
    &'parsed Path,
    &'parsed Ident,
    &'parsed FieldItem<'args, 'hal>,
)> {
    // peripheral/register is provided
    let Some((path, parsed_reg)) = parsed.iter().find(
        |(
            ..,
            Parsed {
                peripheral,
                register,
                ..
            },
        )| { &peripheral.ident == peripheral_ident && &register.ident == register_ident },
    ) else {
        None?
    };

    // field is provided
    parsed_reg
        .items
        .iter()
        .find(|(ident, ..)| &field_ident == ident)
        .map(|(ident, item)| (path, ident, item))
}

fn make_unique_field_ident(peripheral: &Peripheral, register: &Register, field: &Ident) -> Ident {
    format_ident!(
        "{}_{}_{}",
        peripheral.module_name(),
        register.module_name(),
        field
    )
}

fn make_addr<'args, 'hal>(
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

fn make_initial<'args, 'hal>(parsed: &Parsed<'args, 'hal>) -> u32 {
    // start with inert field values (or zero)
    let inert = parsed
        .register
        .fields
        .values()
        .filter_map(|field| Some((field, field.access.get_write()?.numericity.some_inert()?)))
        .fold(0, |acc, (field, variant)| {
            acc | (variant.bits << field.offset)
        });

    // mask out values to be filled in by user
    let mask = parsed.items.values().fold(0, |acc, (field, ..)| {
        acc | ((u32::MAX >> (32 - field.width)) << field.offset)
    });

    // fill in statically known values from fields being statically transitioned
    let statics = parsed
        .items
        .values()
        .flat_map(|(field, binding, transition)| Some((field, binding, transition.as_ref()?)))
        .flat_map(|(field, binding, (.., write_state))| {
            if let Expr::Reference(..) = binding {
                None?
            }

            let bits = match write_state {
                WriteState::Variant(variant) => variant.bits,
                WriteState::Lit(lit_int) => lit_int
                    .base10_parse::<u32>()
                    .expect("lit int should be valid"),
                WriteState::Expr(..) => None?,
            };

            Some(bits << field.offset)
        })
        .reduce(|acc, value| acc | value)
        .unwrap_or(0);

    (inert & !mask) | statics
}

fn make_return_ty<'args, 'hal>(
    path: &'args Path,
    binding: &'args BindingArgs,
    state_args: &'args StateArgs,
    write_state: &'args WriteState<'args, 'hal>,
    field: &'hal Field,
    field_ident: &'args Ident,
) -> Option<TokenStream> {
    if let Expr::Reference(..) = binding {
        None?
    }

    let ty_name = field.type_name();

    Some(match write_state {
        WriteState::Variant(variant) => {
            let ty = match state_args {
                StateArgs::Expr(expr) => quote! { #expr },
                StateArgs::Lit(..) => {
                    let ty = variant.type_name();
                    quote! { #ty }
                }
            };
            quote! {
                #path::#field_ident::#ty_name<#path::#field_ident::#ty>
            }
        }
        WriteState::Expr(expr) => {
            quote! {
                #path::#field_ident::#ty_name<#path::#field_ident::#expr>
            }
        }
        WriteState::Lit(lit_int) => quote! { #path::#field_ident::#ty_name<#lit_int> },
    })
}

fn make_input_ty<'args, 'hal>(
    path: &'args Path,
    binding: &'args BindingArgs,
    peripheral: &'hal Peripheral,
    register: &'hal Register,
    field: &'hal Field,
    field_ident: &'args Ident,
    transition: Option<&(&'args StateArgs, WriteState<'args, 'hal>)>,
) -> TokenStream {
    let ty_name = field.type_name();
    let generic = format_ident!(
        "{}{}{}",
        peripheral.type_name(),
        register.type_name(),
        field.type_name()
    );

    match binding {
        Expr::Reference(r) => {
            if r.mutability.is_some() && transition.is_some() {
                quote! {
                    #path::#field_ident::#ty_name<::proto_hal::stasis::Dynamic>
                }
            } else {
                quote! {
                    #path::#field_ident::#ty_name<#generic>
                }
            }
        }
        _expr => quote! {
            #path::#field_ident::#ty_name<#generic>
        },
    }
}

fn make_parameter_ty<'args, 'hal>(
    binding: &'args BindingArgs,
    transition: Option<&(&'args StateArgs, WriteState<'args, 'hal>)>,
    input_ty: &TokenStream,
) -> TokenStream {
    if let Expr::Reference(r) = binding {
        if r.mutability.is_some() && transition.is_some() {
            quote! { (&mut #input_ty, u32) }
        } else {
            quote! { &#input_ty }
        }
    } else {
        input_ty.clone()
    }
}

fn make_generic<'args, 'hal>(
    binding: &'args BindingArgs,
    peripheral: &'hal Peripheral,
    register: &'hal Register,
    field: &'hal Field,
    transition: Option<&(&'args StateArgs, WriteState<'args, 'hal>)>,
) -> Option<TokenStream> {
    if let Expr::Reference(r) = binding
        && r.mutability.is_some()
        && transition.is_some()
    {
        None?
    }

    let generic = format_ident!(
        "{}{}{}",
        peripheral.type_name(),
        register.type_name(),
        field.type_name()
    );

    Some(quote! { #generic })
}

fn make_parameter_constraints<'args, 'hal>(
    parsed: &IndexMap<Path, Parsed<'args, 'hal>>,
    path: &'args Path,
    prefix: Option<&Path>,
    binding: &'args BindingArgs,
    field: &'hal Field,
    field_ident: &'args Ident,
    generic: Option<&TokenStream>,
    input_ty: &TokenStream,
    return_ty: Option<&TokenStream>,
) -> Option<TokenStream> {
    // if the subject field's write access has entitlements, the entitlements
    // must be satisfied in the input to the gate, and the fields used to
    // satisfy the entitlements cannot be written

    let mut constraints = Vec::new();
    let span = field_ident.span();

    if let Some(generic) = generic {
        constraints
            .push(quote! { #generic: ::proto_hal::stasis::State<#path::#field_ident::Field> });
    }

    if let Expr::Reference(r) = binding
        && r.mutability.is_none()
    {
    } else {
        let write_access_entitlements = field
            .access
            .get_write()
            .map(|write| {
                write
                    .entitlements
                    .iter()
                    .map(|entitlement| {
                        (
                            entitlement.peripheral(),
                            entitlement.register(),
                            entitlement.field(),
                        )
                    })
                    .collect::<IndexSet<_>>()
            })
            .into_iter()
            .flatten()
            .filter_map(|(peripheral, register, field)| {
                query_field(parsed, peripheral, register, field)?;

                let generic = format_ident!("{}{}{}", peripheral.to_string().to_pascal_case(), register.to_string().to_pascal_case(), field.to_string().to_pascal_case(), span = span);

                let prefix = prefix.map(|prefix| quote! { #prefix:: });
                let field_ty = Ident::new(field.to_string().to_pascal_case().as_str(), Span::call_site());

                Some(quote! {
                    #input_ty: ::proto_hal::stasis::Entitled<#prefix #peripheral::#register::#field::#field_ty<#generic>>
                })
            });

        constraints.extend(write_access_entitlements);
    }

    if let Expr::Reference(..) = binding {
        return Some(quote! { #(#constraints,)* });
    }

    let Some(return_ty) = return_ty else {
        return Some(quote! { #(#constraints,)* });
    };

    let statewise_entitlements = field
        .access
        .get_write()
        .and_then(|write| {
            Some(match &write.numericity {
                Numericity::Numeric => None?,
                Numericity::Enumerated { variants } => variants
                    .values()
                    .flat_map(|variant| {
                        variant.entitlements.iter().map(|entitlement| {
                            (
                                entitlement.peripheral(),
                                entitlement.register(),
                                entitlement.field(),
                            )
                        })
                    })
                    .collect::<IndexSet<_>>(),
            })
        })
        .into_iter()
        .flatten()
        .filter_map(|(peripheral, register, field)| {
            let entitlement_return_ty = {
                let (path, field_ident, (field, binding, transition)) = query_field(parsed, peripheral, register, field)?;
                if let Some((state_args, write_state)) = transition {
                    make_return_ty(path, binding, state_args, write_state, field, field_ident)
                } else {
                    None
                }
            };

            Some(if let Some(entitlement_return_ty) = entitlement_return_ty {
                // the entitled to field is being transitioned
                quote! {
                    #return_ty: ::proto_hal::stasis::Entitled<#entitlement_return_ty>
                }
            } else {
                let generic = format_ident!("{peripheral}{register}{field}", span = span);

                quote! {
                    #return_ty: ::proto_hal::stasis::Entitled<#prefix::#peripheral::#register::#field<#generic>>
                }
            })
        });

    constraints.extend(statewise_entitlements);

    Some(quote! { #(#constraints,)* })
}

fn make_argument<'args, 'hal>(
    path: &'args Path,
    binding: &'args BindingArgs,
    transition: Option<&&'args StateArgs>,
    field: &'hal Field,
    field_ident: &'args Ident,
) -> TokenStream {
    if let Expr::Reference(r) = binding
        && r.mutability.is_some()
    {
        // continue forth!
    } else {
        return quote! { #binding };
    };

    let Some(transition) = transition else {
        return quote! { #binding };
    };

    let Some(write) = &field.access.get_write() else {
        return quote! { #binding };
    };

    let body = match (transition, &write.numericity) {
        (StateArgs::Expr(expr), Numericity::Enumerated { .. }) => {
            quote! {{
                #[allow(unused_imports)]
                use #path::#field_ident::write::Variant::*;
                #expr as u32
            }}
        }
        (StateArgs::Expr(expr), ..) => {
            quote! {{
                #expr as u32
            }}
        }
        (StateArgs::Lit(lit_int), ..) => quote! { #lit_int },
    };

    quote! { (#binding, #body) }
}

fn make_dynamic_value<'args, 'hal>(parsed: &Parsed<'args, 'hal>) -> Option<TokenStream> {
    let values = parsed
        .items
        .iter()
        .filter_map(|(field_ident, (field, binding, transition))| {
            transition.as_ref()?;

            if let Expr::Reference(r) = binding
                && r.mutability.is_some()
            {
                // and onwards!
            } else {
                None?
            };

            let unique_field_ident =
                make_unique_field_ident(parsed.peripheral, parsed.register, field_ident);

            let offset = field.offset;
            let shift = (offset != 0).then_some(quote! { << #offset });

            Some(quote! { (#unique_field_ident.1 #shift) })
        })
        .collect::<Vec<_>>();

    (!values.is_empty()).then_some(quote! {
        #(#values)|*
    })
}

pub fn write(model: &Hal, tokens: TokenStream) -> TokenStream {
    let args = match syn::parse2::<Args>(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let mut errors = Vec::new();

    let (parsed, e) = parse(&args, &model);
    errors.extend(e);
    errors.extend(validate(&parsed));

    let mut overridden_base_addrs: HashMap<Ident, Expr> = HashMap::new();

    for override_ in &args.overrides {
        match override_ {
            Override::BaseAddress(ident, expr) => {
                overridden_base_addrs.insert(ident.clone(), expr.clone());
            }
            Override::CriticalSection(expr) => errors.push(syn::Error::new_spanned(
                &expr,
                "stand-alone read access is atomic and doesn't require a critical section",
            )),
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
        Some(imports)
    };

    let errors = {
        let errors = errors.into_iter().map(|e| e.to_compile_error());

        quote! {
            #(
                #errors
            )*
        }
    };

    let mut generics = Vec::new();
    let mut parameter_idents = Vec::new();
    let mut parameter_tys = Vec::new();
    let mut return_tys = Vec::new();
    let mut constraints = Vec::new();
    let mut addrs = Vec::new();
    let mut initials = Vec::new();
    let mut dynamic_values = Vec::new();
    let mut arguments = Vec::new();

    for (path, parsed_reg) in &parsed {
        for (field_ident, (field, binding, transition)) in &parsed_reg.items {
            parameter_idents.push(make_unique_field_ident(
                parsed_reg.peripheral,
                parsed_reg.register,
                field_ident,
            ));

            let input_ty = make_input_ty(
                path,
                binding,
                parsed_reg.peripheral,
                parsed_reg.register,
                field,
                field_ident,
                transition.as_ref(),
            );

            // TODO: if field isn't transitioned, just return it
            let return_ty = if let Some((state_args, write_state)) = transition {
                make_return_ty(path, binding, state_args, write_state, field, field_ident)
            } else {
                None
            };

            let generic = make_generic(
                binding,
                parsed_reg.peripheral,
                parsed_reg.register,
                field,
                transition.as_ref(),
            );

            if let Some(parameter_constraints) = make_parameter_constraints(
                &parsed,
                path,
                parsed_reg.prefix.as_ref(),
                binding,
                field,
                field_ident,
                generic.as_ref(),
                &input_ty,
                return_ty.as_ref(),
            ) {
                constraints.push(parameter_constraints);
            }

            if let Some(generic) = generic {
                generics.push(generic);
            }

            parameter_tys.push(make_parameter_ty(binding, transition.as_ref(), &input_ty));

            if let Some(return_ty) = return_ty {
                return_tys.push(return_ty);
            }

            arguments.push(make_argument(
                path,
                binding,
                transition.as_ref().map(|(state_args, ..)| state_args),
                field,
                field_ident,
            ));
        }

        if parsed_reg.items.iter().any(|(.., (_, binding, ..))| {
            if let Expr::Reference(r) = binding {
                r.mutability.is_none()
            } else {
                false
            }
        }) {
            continue;
        }

        if parsed_reg
            .items
            .values()
            .any(|(.., transition)| transition.is_some())
        {
            addrs.push(make_addr(path, parsed_reg, &overridden_base_addrs));
            initials.push(make_initial(parsed_reg));
            dynamic_values.push(make_dynamic_value(parsed_reg));
        }
    }

    let generics = (!generics.is_empty()).then_some(quote! {
        <#(#generics,)*>
    });

    let transmute = (!return_tys.is_empty()).then_some(quote! {
        unsafe { ::core::mem::transmute(()) }
    });

    let return_tys = (!return_tys.is_empty()).then_some(quote! {
        -> (#(#return_tys),*)
    });

    let constraints = (!constraints.is_empty()).then_some(quote! {
        where #(#constraints)*
    });

    let write_reg_values =
        initials
            .iter()
            .zip(dynamic_values.iter())
            .map(
                |(initial, dynamic_values)| match (initial, dynamic_values) {
                    (0, Some(dynamic_values)) => dynamic_values.clone(),
                    (1.., Some(dynamic_values)) => quote! { #initial | #dynamic_values },
                    (initial, None) => quote! { #initial },
                },
            );

    quote! {
        {
            #suggestions
            #errors

            fn gate #generics (#(#parameter_idents: #parameter_tys,)*) #return_tys #constraints {
                #(
                    unsafe {
                        ::core::ptr::write_volatile(
                            #addrs as *mut u32,
                            #write_reg_values
                        )
                    };
                )*

                #transmute
            }

            gate(#(#arguments,)*)
        }
    }
}
