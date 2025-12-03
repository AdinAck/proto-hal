use std::collections::HashMap;

use derive_more::{Deref, DerefMut};
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{Ident, Index};

use crate::diagnostic::{Context, Diagnostic, Diagnostics};

#[derive(Debug, Clone)]
pub enum InterruptKind {
    Reserved,
    Handler(Ident),
}

#[derive(Debug, Clone)]
pub struct Interrupt {
    pub kind: InterruptKind,
    pub docs: Vec<String>,
}

impl Interrupt {
    pub fn new(kind: InterruptKind) -> Self {
        Self {
            kind,
            docs: Vec::new(),
        }
    }

    pub fn reserved() -> Self {
        Self::new(InterruptKind::Reserved)
    }

    pub fn handler(ident: impl AsRef<str>) -> Self {
        Self::new(InterruptKind::Handler(Ident::new(
            ident.as_ref(),
            Span::call_site(),
        )))
    }

    pub fn docs<I>(mut self, docs: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.docs
            .extend(docs.into_iter().map(|doc| doc.as_ref().to_string()));

        self
    }
}

#[derive(Debug, Clone, Default, Deref, DerefMut)]
pub struct Interrupts {
    interrupts: Vec<Interrupt>,
}

impl Interrupts {
    pub fn empty() -> Self {
        Self {
            interrupts: Vec::new(),
        }
    }

    pub fn validate(&self) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let context = Context::with_path(vec!["interrupts".to_string()]);

        let mut seen = HashMap::new();

        for (i, interrupt) in self.interrupts.iter().enumerate() {
            if let InterruptKind::Handler(ident) = &interrupt.kind
                && let Some(existing) = seen.insert(ident, i)
            {
                diagnostics.insert(Diagnostic::interrupt_exists(
                    ident,
                    &i,
                    &existing,
                    context.clone(),
                ));
            }
        }

        diagnostics
    }

    pub fn device_x(&self) -> String {
        let mut body = String::new();

        for vector in self.interrupts.iter().filter_map(|interrupt| {
            let InterruptKind::Handler(ident) = &interrupt.kind else {
                None?
            };
            Some(ident)
        }) {
            body.push_str(format!("PROVIDE({vector} = DefaultHandler);\n").as_str());
        }

        body
    }
}

impl ToTokens for Interrupts {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        if self.interrupts.is_empty() {
            return;
        }

        let handler_idents = self.interrupts.iter().filter_map(|interrupt| {
            let InterruptKind::Handler(ident) = &interrupt.kind else {
                None?
            };
            Some(ident)
        });
        let docs = self.interrupts.iter().map(|interrupt| &interrupt.docs);

        let symbols = quote! {
            unsafe extern "C" {
                #(
                    #(#[doc = #docs])*
                    fn #handler_idents();
                )*
            }
        };

        let table_length = Index::from(self.interrupts.len());

        let vectors = self
            .interrupts
            .iter()
            .map(|interrupt| match &interrupt.kind {
                InterruptKind::Reserved => quote! { ::proto_hal::interrupt::Vector::reserved() },
                InterruptKind::Handler(ident) => {
                    quote! { ::proto_hal::interrupt::Vector::handler(#ident) }
                }
            });

        let table = quote! {
            #[doc(hidden)]
            #[unsafe(link_section = ".vector_table.interrupts")]
            #[unsafe(no_mangle)]
            pub static __INTERRUPTS: [::proto_hal::interrupt::Vector; #table_length] = [
                #(
                    #vectors,
                )*
            ];
        };

        let vectors = self
            .interrupts
            .iter()
            .enumerate()
            .filter_map(|(position, interrupt)| match &interrupt.kind {
                InterruptKind::Reserved => None,
                InterruptKind::Handler(ident) => {
                    let docs = &interrupt.docs;
                    let position = Index::from(position);
                    Some(quote! {
                        #(#[doc = #docs])*
                        #ident = #position,
                    })
                }
            });

        let enum_ = quote! {
            #[allow(non_camel_case_types)]
            #[repr(u16)]
            pub enum interrupt {
                #(
                    #vectors
                )*
            }
        };

        tokens.extend(quote! {
            #[cfg(feature = "interrupts")]
            pub use ::cortex_m_rt::interrupt;

            #[cfg(feature = "interrupts")]
            #symbols
            #[cfg(feature = "interrupts")]
            #table
            #[cfg(feature = "interrupts")]
            #enum_
        });
    }
}
