pub mod diagnostic;
mod gates;
pub mod parsing;
mod scaffolding;
mod unmask;

use indexmap::IndexMap;
use ir::structures::{field::Field, hal::Hal, peripheral::Peripheral, register::Register};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{
    Expr, ExprLit, Ident, Lit, LitInt, Path, Token, braced,
    parse::Parse,
    parse_quote,
    punctuated::Punctuated,
    token::{Brace, Colon, Comma},
};

// pub use gates::{
//     modify_untracked::modify_untracked,
//     read::read,
//     read_untracked::read_untracked,
//     write::{write, write_in_place},
//     write_untracked::{write_from_reset_untracked, write_from_zero_untracked},
// };
pub use scaffolding::scaffolding;

#[derive(Debug)]
struct Args {
    registers: IndexMap<Path, Vec<RegisterArgs>>,
    overrides: Vec<Override>,
}

impl Parse for Args {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut registers = IndexMap::new();
        let mut overrides = Vec::new();

        while !input.is_empty() {
            if input.peek(Token![@]) {
                input.parse::<Token![@]>()?;
                overrides.push(input.parse()?);
            } else {
                let register_args = input.parse::<RegisterArgs>()?;
                registers
                    .entry(register_args.path.clone())
                    .or_insert(vec![])
                    .push(register_args);
            }
        }

        Ok(Self {
            registers,
            overrides,
        })
    }
}

#[derive(Debug)]
enum Override {
    BaseAddress(Ident, Expr),
    CriticalSection(Expr),
    Unknown(Ident),
}

impl Parse for Override {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<Ident>()?;

        Ok(match ident.to_string().as_str() {
            "base_addr" => {
                let peripheral_ident = input.parse()?;
                let addr = input.parse::<Expr>()?;
                Self::BaseAddress(peripheral_ident, addr)
            }
            "critical_section" => Self::CriticalSection(input.parse()?),
            _unknown => Self::Unknown(ident),
        })
    }
}

#[derive(Debug)]
enum Node {
    Branch {
        path: Path,
        children: Punctuated<Node, Comma>,
    },
    Leaf {
        path: Path,
        entry: FieldArgs,
    },
}

impl Parse for Node {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;

        if input.peek(Colon) {
            // foo::bar::baz: ...

            input.parse::<Colon>()?;

            Ok(Self::Leaf {
                path: path,
                entry: input.parse()?,
            })
        } else if input.peek(Brace) {
            // foo::bar {
            //  baz: ...
            // }

            let block;
            braced!(block in input);

            let children = block.parse_terminated(Parse::parse, Comma)?;

            Ok(Self::Branch { path, children })
        } else {
            // foo::bar::baz {erroneous tokens}

            Ok(Self::Branch {
                path,
                children: Default::default(),
            })
        }
    }
}

#[derive(Debug)]
struct RegisterArgs {
    path: Path,
    fields: Punctuated<FieldArgs, Comma>,
}

impl Parse for RegisterArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;

        if input.peek(Brace) {
            // foo::bar {
            //  baz: ...
            // }

            let block;
            braced!(block in input);

            let fields = block.parse_terminated(Parse::parse, Comma)?;

            Ok(Self { path, fields })
        } else if input.peek(Colon) {
            // foo::bar::baz: ...

            Ok(Self {
                path,
                fields: Punctuated::from_iter(vec![input.parse::<FieldArgs>()?].into_iter()),
            })
        } else {
            Ok(Self {
                path,
                fields: Default::default(),
            })
        }
    }
}

#[derive(Debug)]
struct FieldArgs {
    ident: Ident,
    binding: Option<BindingArgs>,
    transition: Option<TransitionArgs>,
}

impl Parse for FieldArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident = input.parse()?;

        let binding = if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            Some(input.parse()?)
        } else {
            None
        };

        let transition = if input.peek(Token![=>]) {
            Some(input.parse()?)
        } else {
            None
        };

        Ok(Self {
            ident,
            binding,
            transition,
        })
    }
}

pub type BindingArgs = Expr;

#[derive(Debug)]
struct TransitionArgs {
    state: StateArgs,
}

impl Parse for TransitionArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<Token![=>]>()?;
        let state = input.parse()?;

        Ok(Self { state })
    }
}

#[derive(Debug)]
enum StateArgs {
    Expr(Expr),
    Lit(LitInt),
}

impl ToTokens for StateArgs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            StateArgs::Expr(expr) => expr.to_tokens(tokens),
            StateArgs::Lit(lit_int) => lit_int.to_tokens(tokens),
        }
    }
}

impl Parse for StateArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(match input.parse()? {
            Expr::Lit(ExprLit {
                lit: Lit::Int(lit), ..
            }) => Self::Lit(lit),
            other => Self::Expr(other),
        })
    }
}

fn get_register<'hal>(
    path: &Path,
    model: &'hal Hal,
) -> Result<(Option<Path>, &'hal Peripheral, &'hal Register), syn::Error> {
    let mut segments = path.segments.iter().rev();

    let Some(register_ident) = segments.next().map(|segment| &segment.ident) else {
        Err(syn::Error::new_spanned(path, "expected register ident"))?
    };
    let Some(peripheral_ident) = segments.next().map(|segment| &segment.ident) else {
        Err(syn::Error::new_spanned(path, "expected peripheral ident"))?
    };

    let prefix = {
        let segments = segments.rev().collect::<Vec<_>>();
        let leading_colon = &path.leading_colon;

        if segments.is_empty() {
            None
        } else {
            Some(parse_quote! {
                #leading_colon #(#segments)::*
            })
        }
    };

    let peripheral = model
        .peripherals
        .get(peripheral_ident)
        .ok_or(syn::Error::new_spanned(
            peripheral_ident,
            format!("peripheral \"{peripheral_ident}\" does not exist"),
        ))?;

    let register = peripheral
        .registers
        .get(register_ident)
        .ok_or(syn::Error::new_spanned(
            register_ident,
            format!(
                "register \"{register_ident}\" does not exist in peripheral \"{peripheral_ident}\""
            ),
        ))?;

    // TODO: show some peripherals the register *was* found in?

    Ok((prefix, peripheral, register))
}

fn get_field<'a>(ident: &Ident, register: &'a Register) -> syn::Result<&'a Field> {
    register.fields.get(ident).ok_or(syn::Error::new_spanned(
        ident,
        format!(
            "field \"{ident}\" does not exist in register \"{}\"",
            register.module_name()
        ),
    ))
}

pub fn reexports(args: TokenStream) -> TokenStream {
    let idents_raw = vec![
        "modify_untracked",
        "read",
        "read_untracked",
        "write",
        "write_in_place",
        "write_from_reset_untracked",
        "write_from_zero_untracked",
    ];

    let idents = idents_raw
        .iter()
        .map(|name| Ident::new(name, Span::call_site()))
        .collect::<Vec<_>>();

    quote! {
        #(
            #[proc_macro]
            pub fn #idents(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
                ::proto_hal_build::codegen::macros::#idents(&::model::generate(#args), tokens.into()).into()
            }
        )*

        #[proc_macro]
        pub fn scaffolding(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
            ::proto_hal_build::codegen::macros::scaffolding([#(#idents_raw,)*]).into()
        }
    }
}
