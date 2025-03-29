use std::collections::HashMap;

use colored::Colorize;
use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Serialize};

use crate::utils::diagnostic::{Context, Diagnostic, Diagnostics};

use super::{field::Field, Ident};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Register {
    pub ident: String,
    pub offset: u32,

    pub fields: HashMap<String, Field>,
}

impl Register {
    pub fn empty(ident: impl Into<String>, offset: u32) -> Self {
        Self {
            ident: ident.into(),
            offset,
            fields: HashMap::new(),
        }
    }

    pub fn fields(mut self, fields: impl IntoIterator<Item = Field>) -> Self {
        for field in fields {
            self.fields.insert(field.ident.clone(), field);
        }

        self
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = context.clone().and(self.ident.clone());

        if self.offset % 4 != 0 {
            diagnostics.push(
                Diagnostic::error(format!(
                    "register offset must be word aligned. (offset {} does not satisfy: offset % 4 == 0)",
                    self.offset
                ))
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
                    Diagnostic::error(format!(
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
                    Diagnostic::error(format!(
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

impl Ident for Register {
    fn ident(&self) -> &str {
        &self.ident
    }
}

impl ToTokens for Register {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = format_ident!("{}", self.ident);
        let offset = self.offset;

        let field_bodies = self.fields.values().map(|field| field.to_token_stream());

        tokens.extend(quote! {
            pub mod #ident {
                #(
                    #field_bodies
                )*

                pub const OFFSET: u32 = #offset;
            }

        });
    }
}
