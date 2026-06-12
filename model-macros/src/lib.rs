use std::ops::Deref;

use derive_more::{AsRef, Deref};
use heck::ToPascalCase as _;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    Attribute, Expr, ExprLit, ExprRange, FnArg, Ident, Lit, MetaNameValue, braced, parenthesized,
    parse::{Parse, ParseBuffer, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token,
};

type ComponentInput = Input<TokenStream>;
type SchemaInput = Input<Fields>;

/// ```ignore
/// TraitName {
///     name1(a: A, b: B, c: C, ...) { ... }
///     name2(...) { ... }
/// }
/// ```
struct Input<T> {
    attrs: Vec<Attribute>,
    r#trait: Ident,
    modality: Modality,
    funcs: Functions<T>,
}

impl<T: Parse> Parse for Input<T> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            r#trait: input.parse()?,
            modality: input.parse()?,
            funcs: {
                let content;
                braced!(content in input);
                content.parse()?
            },
        })
    }
}

#[derive(Deref, AsRef)]
struct Modality(Option<Ident>);

impl Parse for Modality {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self(if input.peek(token::Lt) {
            input.parse::<token::Lt>()?;
            let ident = input.parse()?;
            input.parse::<token::Gt>()?;
            Some(ident)
        } else {
            None
        }))
    }
}

struct Functions<T> {
    functions: Vec<Function<T>>,
}

impl<T: Parse> Parse for Functions<T> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut functions = Vec::new();

        while !input.is_empty() {
            functions.push(input.parse()?);
        }

        Ok(Self { functions })
    }
}

/// `name(a: A, b: B, c: C, ...) { ... }`
struct Function<T> {
    attrs: Vec<Attribute>,
    name: Ident,
    params: Punctuated<FnArg, token::Comma>,
    body: Option<T>,
}

impl Function<Fields> {
    const CAPTURED_ATTRS: &[&str] = &["inherits"];
}

impl<T: Parse> Parse for Function<T> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            name: input.parse()?,
            params: {
                let content;
                parenthesized!(content in input);
                content.parse_terminated(FnArg::parse, token::Comma)?
            },
            body: {
                if input.peek(token::Brace) {
                    let content;
                    braced!(content in input);
                    content.parse().ok()
                } else {
                    None
                }
            },
        })
    }
}

#[derive(Deref, AsRef)]
struct Fields {
    fields: Punctuated<Field, token::Comma>,
}

impl Parse for Fields {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            fields: input.call(Punctuated::parse_terminated)?,
        })
    }
}

#[derive(Clone)]
struct Field {
    attrs: Vec<Attribute>,
    ident: Ident,
    body: Expr,
}

impl Field {
    const CAPTURED_ATTRS: &[&str] = &["entitlement", "array"];
}

impl Parse for Field {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            ident: input.parse()?,
            body: {
                input.parse::<token::Colon>()?;
                input.parse()?
            },
        })
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { ident, body, .. } = self;

        tokens.extend(quote! { #ident: #body });
    }
}

#[proc_macro]
pub fn peripheral(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    inner(tokens, |input| {
        extension_trait(
            input,
            quote! { ::phm::model::AddPeripheral },
            no_modality(|| quote! { ::phm::model::PeripheralEntry<'ncx> }),
        )
    })
}

#[proc_macro]
pub fn register(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    inner(tokens, |input| {
        extension_trait(
            input,
            quote! { ::phm::model::AddRegister },
            no_modality(|| quote! { ::phm::model::RegisterEntry<'ncx> }),
        )
    })
}

#[proc_macro]
pub fn field(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    inner(tokens, |input| {
        extension_trait(
            input,
            quote! { ::phm::model::AddField },
            require_modality(
                input.r#trait.span(),
                |modality| quote! { ::phm::model::FieldEntry<'ncx, ::phm::field::access::#modality> },
            ),
        )
    })
}

// just absolutely disgusting code below

#[proc_macro]
pub fn schema(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    inner(
        tokens,
        |SchemaInput {
             attrs,
             r#trait,
             modality,
             funcs,
         }| {
            let impl_ty = require_modality(r#trait.span(), |modality| {
                quote! { ::phm::model::FieldEntry<'cx, ::phm::field::access::#modality> }
            })(modality)?;

            let get_ty_name = |name: &Ident| {
                format_ident!(
                    "{}Schema",
                    name.to_string().to_pascal_case(),
                    span = name.span()
                )
            };

            let is_entitlement = |field: &Field| {
                field
                    .attrs
                    .iter()
                    .any(|attr| attr.meta.path().is_ident("entitlement"))
            };

            let defs = funcs
                .functions
                .iter()
                .map(
                    |func @ Function {
                         attrs,
                         name,
                         params,
                         body,
                     }| {
                        let body = body.as_ref().map(Deref::deref);
                        let ty_name = get_ty_name(name);
                        let attrs = attrs.iter().filter(|attr| {
                            !Function::<Fields>::CAPTURED_ATTRS
                                .iter()
                                .any(|ident| attr.meta.path().is_ident(ident))
                        });

                        let (fields, factory_defs) = if let Some(fields) = body {
                            let mut fields = Vec::from_iter(fields.iter());

                            inherit(funcs, func, &mut fields)?;
                            let (fields, factory_defs, ..) = expand_fields(fields)?;
                            (Some(fields), Some(factory_defs))
                        } else {
                            (None, None)
                        };

                        let return_ = if fields
                            .is_some_and(|mut fields| fields.find(is_entitlement).is_none())
                        {
                            None
                        } else {
                            Some(quote! { -> #ty_name })
                        };

                        Ok(quote! {
                            #(#attrs)*
                            fn #name<'ncx>(&'ncx mut self, #params) #return_;
                            #factory_defs
                        })
                    },
                )
                .collect::<Result<Vec<_>, syn::Error>>()?;

            let impls = funcs
                .functions
                .iter()
                .map(
                    |func @ Function {
                         name, params, body, ..
                     }| {
                        let body = body.as_ref().map(Deref::deref);
                        let ty_name = get_ty_name(name);

                        let Some(fields) = body else {
                            return Ok::<_, syn::Error>(quote! {});
                        };
                        let mut fields = Vec::from_iter(fields.iter());

                        inherit(funcs, func, &mut fields)?;
                        let (fields, .., factory_impls) = expand_fields(fields)?;

                        let (entitlement_fields, non_entitlement_fields) =
                            fields.partition::<Vec<_>, _>(is_entitlement);


                        let (return_, schema) = if entitlement_fields.is_empty() {
                            (None, None)
                        } else {
                            (Some(quote! { -> #ty_name }), Some(
                                quote! { #ty_name { #(#entitlement_fields.make_entitlement(),)* } },
                            ))
                        };

                        let non_entitlement_fields =
                            non_entitlement_fields.iter().map(|field| &field.body);

                        Ok(quote! {
                            fn #name<'ncx>(&'ncx mut self, #params) #return_ {
                                #(#non_entitlement_fields;)*
                                #schema
                            }

                            #factory_impls
                        })
                    },
                )
                .collect::<Result<Vec<TokenStream>, syn::Error>>()?;

            let schemas = funcs
                .functions
                .iter()
                .map(
                    |func @ Function {
                         attrs, name, body, ..
                     }| {
                        let ty_name = get_ty_name(name);
                        let attrs = attrs.iter().filter(|attr| {
                            !Function::<Fields>::CAPTURED_ATTRS
                                .iter()
                                .any(|ident| attr.meta.path().is_ident(ident))
                        });

                        let Some(fields) = body else {
                            return Ok(quote! {});
                        };

                        let mut fields = Vec::from_iter(fields.iter());

                        inherit(funcs, func, &mut fields)?;
                        let (fields, ..) = expand_fields(fields)?;

                        let mut entitlement_fields = fields
                            .clone()
                            .filter(|field| is_entitlement(field))
                            .map(|field| field.ident)
                            .peekable();

                        if entitlement_fields.peek().is_none() {
                            return Ok(quote! {});
                        }

                        let entitlement_field_attrs =
                            fields.filter(|field| is_entitlement(field)).map(|field| {
                                field
                                    .attrs
                                    .into_iter()
                                    .filter(|attr| {
                                        !Field::CAPTURED_ATTRS
                                            .iter()
                                            .any(|ident| attr.meta.path().is_ident(ident))
                                    })
                                    .collect::<Vec<_>>()
                            });

                        Ok(quote! {
                            #[derive(Clone, Copy)]
                            #(#attrs)*
                            pub struct #ty_name {#(
                                #(#entitlement_field_attrs)*
                                pub #entitlement_fields: ::phm::Entitlement,
                            )*}
                        })
                    },
                )
                .collect::<Result<Vec<TokenStream>, syn::Error>>()?;

            Ok(quote! {
                #(#attrs)*
                pub trait #r#trait {
                    #(#defs)*
                }

                impl<'cx> #r#trait for #impl_ty {
                    #(#impls)*
                }

                #(#schemas)*
            })
        },
    )
}

fn inherits(func: &Function<Fields>) -> Option<Result<Ident, syn::Error>> {
    func.attrs.iter().find_map(|attr| {
        if attr.meta.path().is_ident("inherits") {
            Some(attr.parse_args::<Ident>())
        } else {
            None
        }
    })
}

fn inherit<'a>(
    funcs: &'a Functions<Fields>,
    func: &Function<Fields>,
    fields: &mut Vec<&'a Field>,
) -> Result<(), syn::Error> {
    if let Some(inheriting) = inherits(func) {
        let inheriting = inheriting?;

        let inheriting = funcs
            .functions
            .iter()
            .find(|other| other.name == inheriting)
            .ok_or(syn::Error::new_spanned(
                &inheriting,
                "schema does not exist",
            ))?;

        let mut inherited_fields = Vec::from_iter(inheriting.body.iter().flat_map(|f| f.iter()));
        inherited_fields.extend(fields.iter());
        *fields = inherited_fields;
    }

    Ok(())
}

fn get_array(field: &Field) -> Option<&Attribute> {
    field
        .attrs
        .iter()
        .find(|attr| attr.meta.path().is_ident("array"))
}

fn expand_fields<'f>(
    fields: impl IntoIterator<Item = &'f Field>,
) -> Result<
    (
        impl Iterator<Item = Field> + Clone,
        TokenStream,
        TokenStream,
    ),
    syn::Error,
> {
    let mut factory_defs = quote! {};
    let mut factory_impls = quote! {};
    let fields = fields
        .into_iter()
        .cloned()
        .map(|field| {
            if let Some(array) = get_array(&field) {
                let list = array.meta.require_list()?;

                let (indicies, index_pattern, index_pattern_span) = list.parse_args_with(|input: &ParseBuffer| {
                    let indicies = input.parse::<ExprRange>().map_err(|mut e| {
                        e.combine(syn::Error::new(e.span(), "expected array range"));
                        e
                    })?;
                    input.parse::<token::Comma>()?;
                    let index_pattern = input.parse::<MetaNameValue>().map_err(|mut e| {
                        e.combine(syn::Error::new(
                            e.span(),
                            "expected argument \"index_pattern\"",
                        ));
                        e
                    })?;

                    if !index_pattern.path.is_ident("index_pattern") {
                        Err(syn::Error::new_spanned(
                            &index_pattern,
                            "expected argument \"index_pattern\"",
                        ))?
                    }

                    let Expr::Lit(ExprLit {
                        lit: Lit::Str(index_pattern),
                        ..
                    }) = index_pattern.value
                    else {
                        Err(syn::Error::new_spanned(
                            &index_pattern.value,
                            "expected string literal",
                        ))?
                    };

                    Ok((indicies, index_pattern.value(), index_pattern.span()))
                })?;

                let range = match (
                    indicies.limits,
                    indicies.start.clone().map(|x| *x),
                    indicies.end.clone().map(|x| *x),
                ) {
                    (.., None) => Err(syn::Error::new_spanned(
                        &indicies,
                        "range end must be specified",
                    ))?,
                    (
                        syn::RangeLimits::HalfOpen(..),
                        None,
                        Some(Expr::Lit(ExprLit {
                            lit: Lit::Int(end), ..
                        })),
                    ) => 0..=(end.base10_parse::<u8>()? - 1),
                    (
                        syn::RangeLimits::HalfOpen(..),
                        Some(Expr::Lit(ExprLit {
                            lit: Lit::Int(start),
                            ..
                        })),
                        Some(Expr::Lit(ExprLit {
                            lit: Lit::Int(end), ..
                        })),
                    ) => start.base10_parse()?..=(end.base10_parse::<u8>()? - 1),
                    (
                        syn::RangeLimits::Closed(..),
                        None,
                        Some(Expr::Lit(ExprLit {
                            lit: Lit::Int(end), ..
                        })),
                    ) => 0..=end.base10_parse()?,
                    (
                        syn::RangeLimits::Closed(..),
                        Some(Expr::Lit(ExprLit {
                            lit: Lit::Int(start),
                            ..
                        })),
                        Some(Expr::Lit(ExprLit {
                            lit: Lit::Int(end), ..
                        })),
                    ) => start.base10_parse()?..=end.base10_parse()?,
                    (..) => Err(syn::Error::new_spanned(
                        &indicies,
                        "range must be bounded by integer literals",
                    ))?,
                };

                {
                    let Field { ident, body, attrs } = &field;
                    let index = Ident::new(&index_pattern, index_pattern_span);
                    let attrs = attrs.iter().filter(|attr| !Field::CAPTURED_ATTRS.iter().any(|ident| attr.meta.path().is_ident(ident)));

                    factory_defs.extend({
                        quote! {
                            #(#attrs)*
                            fn #ident<'ncx>(&'ncx mut self, #index: u8) -> ::phm::model::VariantEntry<'ncx>;
                        }
                    });

                    factory_impls.extend({
                        quote! {
                            fn #ident<'ncx>(&'ncx mut self, #index: u8) -> ::phm::model::VariantEntry<'ncx> { #body }
                        }
                    });
                }


                Ok(range
                    .map(|i| Field {
                        attrs: field.attrs.clone(),
                        ident: {
                            Ident::new(
                                &field
                                    .ident
                                    .to_string()
                                    .replace(&index_pattern, &i.to_string()),
                                index_pattern.span(),
                            )
                        },
                        body: {
                            let factory_ident = &field.ident;
                            parse_quote! { self.#factory_ident(#i) }
                        },
                    })
                    .collect())
            } else {
                Ok::<_, syn::Error>(vec![field])
            }
        })
        .collect::<Result<Vec<_>, syn::Error>>()?
        .into_iter()
        .flatten();

    Ok((fields, factory_defs, factory_impls))
}

fn inner<T: Parse>(
    tokens: proc_macro::TokenStream,
    f: impl FnOnce(&T) -> syn::Result<TokenStream>,
) -> proc_macro::TokenStream {
    match f(&parse_macro_input!(tokens as T)) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn no_modality(
    f: impl FnOnce() -> TokenStream,
) -> impl FnOnce(&Option<Ident>) -> syn::Result<TokenStream> {
    |modality| {
        if let Some(modality) = modality {
            Err(syn::Error::new(modality.span(), "unexpected modality"))
        } else {
            Ok(f())
        }
    }
}

fn require_modality(
    span: Span,
    f: impl FnOnce(&Ident) -> TokenStream,
) -> impl FnOnce(&Option<Ident>) -> syn::Result<TokenStream> {
    move |modality| {
        let Some(modality) = modality else {
            Err(syn::Error::new(span, "field modality must be specified"))?
        };
        Ok(f(modality))
    }
}

fn extension_trait(
    ComponentInput {
        attrs,
        r#trait,
        modality,
        funcs,
    }: &ComponentInput,
    parent: TokenStream,
    child: impl FnOnce(&Option<Ident>) -> syn::Result<TokenStream>,
) -> syn::Result<TokenStream> {
    let child = child(modality)?;
    let defs = funcs.functions.iter().map(|Function { name, params, .. }| {
        quote! {
            fn #name<'ncx>(&'ncx mut self, #params) -> #child;
        }
    });

    let impls = funcs.functions.iter().map(
        |Function {
             attrs,
             name,
             params,
             body,
         }| {
            quote! {
                #(#attrs)*
                fn #name<'ncx>(&'ncx mut self, #params) -> #child {
                    use ::phm::prelude::*;

                    #body
                }
            }
        },
    );

    Ok(quote! {
        #(#attrs)*
        pub trait #r#trait {
            #(#defs)*
        }

        impl<T> #r#trait for T where T: #parent {
            #(#impls)*
        }
    })
}
