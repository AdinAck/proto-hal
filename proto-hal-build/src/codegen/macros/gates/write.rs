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
use quote::{ToTokens as _, format_ident, quote, quote_spanned};
use syn::{Expr, Ident, LitInt, Path, spanned::Spanned};

use crate::codegen::macros::{
    Args, BindingArgs, Override, RegisterArgs, StateArgs, get_field, get_register,
};

type FieldItem<'input, 'model> = (
    &'model Field,
    &'input BindingArgs,
    Option<(&'input StateArgs, WriteState<'input, 'model>)>,
);

/// A parsed unit of the provided tokens and corresponding model nodes which
/// represents a single register.
struct Parsed<'input, 'model> {
    prefix: Option<Path>,
    peripheral: &'model Peripheral,
    register: &'model Register,
    items: IndexMap<Ident, FieldItem<'input, 'model>>,
}

enum WriteState<'input, 'model> {
    Variant(&'model Variant),
    Expr(&'input Expr),
    Lit(&'input LitInt),
}

fn parse<'input, 'model>(
    args: &'input Args,
    model: &'model Hal,
) -> (IndexMap<Path, Parsed<'input, 'model>>, Vec<syn::Error>) {
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
fn parse_registers<'input, 'model>(
    args: &'input Args,
    model: &'model Hal,
) -> (
    IndexMap<
        Path,
        (
            Option<Path>,
            &'input RegisterArgs,
            &'model Peripheral,
            &'model Register,
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
fn parse_fields<'input, 'model>(
    register_args: &'input RegisterArgs,
    register: &'model Register,
) -> (IndexMap<Ident, FieldItem<'input, 'model>>, Vec<syn::Error>) {
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

fn validate<'input, 'model>(
    hal: &'model Hal,
    parsed: &IndexMap<Path, Parsed<'input, 'model>>,
) -> Vec<syn::Error> {
    let mut errors = Vec::new();
    let field_errors = parsed
        .values()
        .flat_map(|parsed_reg| parsed_reg.items.iter().flat_map(|(field_ident, (field, .., transition))| {
            if transition.is_none() {
                return vec![];
            }

            // the following validation steps only apply if the field is
            // being transitioned

            // write access
            let Some(write) = field.access.get_write() else {
                return vec![syn::Error::new_spanned(
                    field_ident,
                    format!("field \"{field_ident}\" is not writable"),
                )];
            };

            // entitlements *from* the field
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

            // entitlements *to* the field
            let mut statewise_entitlements = IndexSet::new();
            for peripheral in hal.peripherals.values() {
                for register in peripheral.registers.values() {
                    for field in register.fields.values() {
                        if let Some(Numericity::Enumerated { variants }) = field.access.get_write().map(|write| &write.numericity) {
                            for variant in variants.values() {
                                if variant.entitlements
                                    .iter()
                                    .any(|entitlement|
                                        entitlement.peripheral() == &parsed_reg.peripheral.module_name()
                                        && entitlement.register() == &parsed_reg.register.module_name()
                                        && parsed_reg.items.keys().any(|field_ident| field_ident == entitlement.field())
                                    ) {
                                    statewise_entitlements.insert((peripheral.module_name(), register.module_name(), field.module_name()));
                                }
                            }
                        }
                    }
                }
            }

            for entitlement in statewise_entitlements {
                if !parsed.values().any(|parsed_reg| {
                    parsed_reg.peripheral.module_name() == entitlement.0 && parsed_reg.register.module_name() == entitlement.1 && parsed_reg.items.keys().any(|field_ident| field_ident == &entitlement.2)
                }) {
                    errors.push(syn::Error::new_spanned(&field_ident, format!(
                        "field \"{field_ident}\" is an entitlement of at least one state in \"{}::{}::{}\" which must be provided",
                        entitlement.0,
                        entitlement.1,
                        entitlement.2
                    )));
                }
            }

            errors
        }));

    let register_errors = parsed.iter().flat_map(|(path, parsed)| {
        let provided_fields = parsed
            .items
            .values()
            .map(|(field, ..)| field)
            .collect::<Vec<_>>();
        let required_fields = parsed
            .register
            .fields
            .values()
            .filter(|field| {
                let Some(write) = field.access.get_write() else {
                    return false;
                };
                write.numericity.some_inert().is_none()
            })
            .collect::<Vec<_>>();

        let mut errors = Vec::new();
        let mut previous_missing_fields = IndexSet::new();

        for i in 0..32 {
            if provided_fields
                .iter()
                .any(|field| field.domain().contains(&i))
            {
                continue;
            }

            let missing_fields = required_fields
                .iter()
                .filter(|field| field.domain().contains(&i))
                .map(|field| field.module_name())
                .collect();

            if previous_missing_fields != missing_fields && !missing_fields.is_empty() {
                let formatted_fields = missing_fields
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                errors.push(syn::Error::new(
                    path.span(),
                    if missing_fields.len() == 1 {
                        format!("{formatted_fields} must be specified")
                    } else {
                        format!("one of [{formatted_fields}] must be specified")
                    },
                ));
                previous_missing_fields = missing_fields;
            }
        }

        errors
    });

    errors.extend(field_errors);
    errors.extend(register_errors);

    errors
}

fn query_field<'input, 'model, 'parsed>(
    parsed: &'parsed IndexMap<Path, Parsed<'input, 'model>>,
    peripheral_ident: &Ident,
    register_ident: &Ident,
    field_ident: &Ident,
) -> Option<(
    &'parsed Path,
    &'parsed Ident,
    &'parsed FieldItem<'input, 'model>,
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

fn make_addr<'input, 'model>(
    path: &Path,
    parsed: &Parsed<'input, 'model>,
    overridden_base_addrs: &HashMap<Ident, Expr>,
) -> TokenStream {
    let register_offset = parsed.register.offset as usize;

    if let Some(base_addr) = overridden_base_addrs.get(&parsed.peripheral.module_name()) {
        quote! { (#base_addr + #register_offset) }
    } else {
        quote! { #path::ADDR }
    }
}

fn make_initial<'input, 'model>(parsed: &Parsed<'input, 'model>) -> u32 {
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

fn make_return_ty<'input, 'model>(
    path: &'input Path,
    binding: &'input BindingArgs,
    state_args: &'input StateArgs,
    write_state: &'input WriteState<'input, 'model>,
    field: &'model Field,
    field_ident: &'input Ident,
    output_generic: Option<&TokenStream>,
) -> Option<TokenStream> {
    if let Expr::Reference(..) = binding {
        None?
    }

    let ty_name = field.type_name();

    if let Some(output_generic) = output_generic {
        return Some(quote! {
            #path::#field_ident::#ty_name<#output_generic>
        });
    }

    let numeric_ty = field
        .access
        .get_write()?
        .numericity
        .numeric_ty(field.width)
        .map(|(.., ty)| ty);

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
            let state = if let Some(numeric_ty) = numeric_ty {
                quote! { ::proto_hal::stasis::#numeric_ty<#expr> }
            } else {
                quote! { #path::#field_ident::#expr }
            };

            quote! {
                #path::#field_ident::#ty_name<#state>
            }
        }
        WriteState::Lit(lit_int) => {
            let state = if let Some(numeric_ty) = numeric_ty {
                quote! { ::proto_hal::stasis::#numeric_ty<#lit_int> }
            } else {
                quote! { #lit_int }
            };

            quote! { #path::#field_ident::#ty_name<#state> }
        }
    })
}

fn make_input_ty<'input, 'model>(
    path: &'input Path,
    binding: &'input BindingArgs,
    peripheral: &'model Peripheral,
    register: &'model Register,
    field: &'model Field,
    field_ident: &'input Ident,
    transition: Option<&(&'input StateArgs, WriteState<'input, 'model>)>,
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

fn make_parameter_ty<'input, 'model>(
    binding: &'input BindingArgs,
    transition: Option<&(&'input StateArgs, WriteState<'input, 'model>)>,
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

fn make_generics<'input, 'model>(
    binding: &'input BindingArgs,
    peripheral: &Ident,
    register: &Ident,
    field: &Ident,
    transition: Option<&(&'input StateArgs, WriteState<'input, 'model>)>,
) -> (Option<TokenStream>, Option<TokenStream>) {
    if let Expr::Reference(r) = binding
        && r.mutability.is_some()
        && transition.is_some()
    {
        return (None, None);
    }

    let input_generic = format_ident!("{}{}{}", peripheral, register, field,);

    let output_generic = if let Some((state_args, ..)) = transition
        && let StateArgs::Expr(expr) = state_args
        && expr.to_token_stream().to_string().trim() == "_"
    {
        let ident = format_ident!("New{input_generic}");

        Some(quote! { #ident })
    } else {
        None
    };

    (Some(quote! { #input_generic }), output_generic)
}

fn make_constraints<'input, 'model>(
    parsed: &IndexMap<Path, Parsed<'input, 'model>>,
    path: &'input Path,
    prefix: Option<&Path>,
    binding: &'input BindingArgs,
    field: &'model Field,
    field_ident: &'input Ident,
    input_generic: Option<&TokenStream>,
    output_generic: Option<&TokenStream>,
    input_ty: &TokenStream,
    return_ty: Option<&TokenStream>,
) -> Option<TokenStream> {
    // if the subject field's write access has entitlements, the entitlements
    // must be satisfied in the input to the gate, and the fields used to
    // satisfy the entitlements cannot be written

    let mut constraints = Vec::new();
    let span = field_ident.span();

    if let Some(generic) = input_generic {
        constraints
            .push(quote! { #generic: ::proto_hal::stasis::State<#path::#field_ident::Field> });
    }

    if let Some(generic) = output_generic {
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
        .filter_map(|(peripheral, register, f)| {
            let entitlement_return_ty = {
                let (path, field_ident, (field, binding, transition)) = query_field(parsed, peripheral, register, f)?;
                let (.., output_generic) = make_generics(binding, &Ident::new(peripheral.to_string().to_pascal_case().as_str(), peripheral.span()), &Ident::new(register.to_string().to_pascal_case().as_str(), register.span()), &field.type_name(), transition.as_ref());

                if let Some((state_args, write_state)) = transition {
                    make_return_ty(path, binding, state_args, write_state, field, field_ident, output_generic.as_ref())
                } else {
                    None
                }
            };

            Some(if let Some(entitlement_return_ty) = entitlement_return_ty {
                // the entitled to field is being transitioned

                if let Some(output_generic) = output_generic {
                    // transitioned generically
                    let field_ty = field.type_name();

                    quote! {
                        #path::#field_ident::#field_ty<#output_generic>: ::proto_hal::stasis::Entitled<#entitlement_return_ty>
                    }
                } else {
                    // transitioned concretely
                    quote! {
                        #return_ty: ::proto_hal::stasis::Entitled<#entitlement_return_ty>
                    }
                }
            } else {
                let generic = format_ident!(
                    "{}{}{}",
                    peripheral.to_string().to_pascal_case(),
                    register.to_string().to_pascal_case(),
                    f.to_string().to_pascal_case(),
                    span = span);

                let field_ty = Ident::new(f.to_string().to_pascal_case().as_str(), f.span());

                let prefix = prefix.map(|prefix| quote! { #prefix:: });

                quote! {
                    #return_ty: ::proto_hal::stasis::Entitled<#prefix #peripheral::#register::#f::#field_ty<#generic>>
                }
            })
        });

    constraints.extend(statewise_entitlements);

    Some(quote! { #(#constraints,)* })
}

fn make_argument<'input, 'model>(
    path: &'input Path,
    binding: &'input BindingArgs,
    transition: Option<&&'input StateArgs>,
    field: &'model Field,
    field_ident: &'input Ident,
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

fn make_reg_write_value<'input, 'model>(parsed: &Parsed<'input, 'model>) -> Option<TokenStream> {
    let values = parsed
        .items
        .iter()
        .filter_map(|(field_ident, (field, binding, transition))| {
            let offset = field.offset;
            let shift = (offset != 0).then_some(quote! { << #offset });

            let (input_generic, output_generic) = make_generics(
                binding,
                &parsed.peripheral.type_name(),
                &parsed.register.type_name(),
                &field.type_name(),
                transition.as_ref(),
            );

            match (binding, transition, input_generic, output_generic) {
                // 1. &mut binding => expr
                (Expr::Reference(r), Some(..), ..) if r.mutability.is_some() => {
                    let unique_field_ident =
                        make_unique_field_ident(parsed.peripheral, parsed.register, field_ident);

                    Some(quote! { (#unique_field_ident.1 #shift) })
                }
                // 2. &binding
                (Expr::Reference(..), None, Some(input_generic), ..) => {
                    Some(quote! { #input_generic::VALUE #shift })
                }
                // 3. binding => _
                (.., Some(output_generic)) => Some(quote! { #output_generic::VALUE #shift }),
                (..) => None,
            }
        })
        .collect::<Vec<_>>();

    (!values.is_empty()).then_some(quote! {
        #(#values)|*
    })
}

fn make_conjure() -> TokenStream {
    quote! { ::proto_hal::stasis::Conjure::conjure() }
}

fn write_inner(model: &Hal, tokens: TokenStream, in_place: bool) -> TokenStream {
    let args = match syn::parse2::<Args>(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let mut errors = Vec::new();

    let (parsed, e) = parse(&args, &model);
    errors.extend(e);
    errors.extend(validate(model, &parsed));

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
    let mut reg_write_values = Vec::new();
    let mut arguments = Vec::new();
    let mut conjures = Vec::new();
    let mut rebinds = Vec::new();

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

            let (input_generic, output_generic) = make_generics(
                binding,
                &parsed_reg.peripheral.type_name(),
                &parsed_reg.register.type_name(),
                &field.type_name(),
                transition.as_ref(),
            );

            // TODO: if field isn't transitioned, just return it
            let return_ty = if let Some((state_args, write_state)) = transition {
                make_return_ty(
                    path,
                    binding,
                    state_args,
                    write_state,
                    field,
                    field_ident,
                    output_generic.as_ref(),
                )
            } else {
                None
            };

            if let Some(parameter_constraints) = make_constraints(
                &parsed,
                path,
                parsed_reg.prefix.as_ref(),
                binding,
                field,
                field_ident,
                input_generic.as_ref(),
                output_generic.as_ref(),
                &input_ty,
                return_ty.as_ref(),
            ) {
                constraints.push(parameter_constraints);
            }

            if let Some(generic) = input_generic {
                generics.push(generic);
            }

            if let Some(generic) = output_generic {
                generics.push(generic);
            }

            parameter_tys.push(make_parameter_ty(binding, transition.as_ref(), &input_ty));

            if let Some(return_ty) = return_ty {
                return_tys.push(return_ty);
                conjures.push(make_conjure());
                rebinds.push(binding);
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
            reg_write_values.push(make_reg_write_value(parsed_reg));
        }
    }

    let generics = (!generics.is_empty()).then_some(quote! {
        <#(#generics,)*>
    });

    let conjures = (!return_tys.is_empty()).then_some(quote! {
        unsafe {(
            #(
                #conjures
            ),*
        )}
    });

    let return_tys = (!return_tys.is_empty()).then_some(quote! {
        -> (#(#return_tys),*)
    });

    let constraints = (!constraints.is_empty()).then_some(quote! {
        where #(#constraints)*
    });

    let reg_write_values =
        initials
            .iter()
            .zip(reg_write_values.iter())
            .map(|(initial, write_values)| match (initial, write_values) {
                (0, Some(write_values)) => write_values.clone(),
                (1.., Some(write_values)) => quote! { #initial | #write_values },
                (initial, None) => quote! { #initial },
            });

    let rebinds = in_place.then_some(quote! { let (#(#rebinds),*) = });
    let semicolon = in_place.then_some(quote! { ; });

    quote! {
        #rebinds {
            #suggestions
            #errors

            fn gate #generics (#(#parameter_idents: #parameter_tys,)*) #return_tys #constraints {
                #(
                    unsafe {
                        ::core::ptr::write_volatile(
                            #addrs as *mut u32,
                            #reg_write_values
                        )
                    };
                )*

                #conjures
            }

            gate(#(#arguments,)*)
        } #semicolon
    }
}

pub fn write(model: &Hal, tokens: TokenStream) -> TokenStream {
    write_inner(model, tokens, false)
}
pub fn write_in_place(model: &Hal, tokens: TokenStream) -> TokenStream {
    write_inner(model, tokens, true)
}
