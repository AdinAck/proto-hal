use std::collections::HashMap;

use colored::Colorize;
use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Serialize};

use crate::utils::diagnostic::{self, Context, Diagnostic};

use super::field::Field;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Register {
    pub ident: String,
    pub offset: u32,

    pub fields: HashMap<String, Field>,
}

impl Register {
    pub fn empty(ident: String, offset: u32) -> Self {
        Self {
            ident,
            offset,
            fields: HashMap::new(),
        }
    }

    pub fn validate(&self, context: &Context) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let new_context = context.clone().and(self.ident.clone());

        if self.offset % 4 != 0 {
            diagnostics.push(
                diagnostic::Error(format!("register offset must be word aligned."))
                    .with_context(new_context.clone()),
            );
        }

        let mut fields = self.fields.values().collect::<Vec<_>>();
        fields.sort();

        for window in fields.windows(2) {
            let lhs = window[0];
            let rhs = window[1];

            if lhs.offset + lhs.width > rhs.offset {
                diagnostics.push(
                    diagnostic::Error(format!(
                        "fields [{}] and [{}] overlap.",
                        lhs.ident.bold(),
                        rhs.ident.bold()
                    ))
                    .with_context(new_context.clone()),
                );
            }
        }

        if let Some(field) = fields.last() {
            if field.offset + field.width > 32 {
                diagnostics.push(
                    diagnostic::Error(format!(
                        "field [{}] exceeds register width.",
                        field.ident.bold()
                    ))
                    .with_context(new_context.clone()),
                );
            }
        }

        for field in fields {
            diagnostics.extend(field.validate(&new_context));
        }

        diagnostics
    }
}

impl PartialOrd for Register {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Register {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.offset.cmp(&other.offset)
    }
}

impl ToTokens for Register {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = format_ident!("{}", self.ident);

        let field_idents = self
            .fields
            .values()
            .map(|field| format_ident!("{}", field.ident));

        let field_bodies = self.fields.values().map(|field| field.to_token_stream());

        tokens.extend(quote! {
            pub mod #ident {
                #(
                    #field_bodies
                )*

                pub struct Reset {
                    #(
                        #field_idents: #field_idents::Reset,
                    )*
                }
            }
        });
    }
}
