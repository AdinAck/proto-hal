use std::collections::HashMap;

use colored::Colorize;
use quote::{format_ident, quote, ToTokens};

use crate::utils::diagnostic::{Context, Diagnostic, Diagnostics};

use super::{entitlement::Entitlement, field::Field, Ident};

#[derive(Debug, Clone)]
pub struct Register {
    pub ident: String,
    pub offset: u32,

    pub fields: HashMap<String, Field>,
}

impl Register {
    pub fn new(
        ident: impl Into<String>,
        offset: u32,
        fields: impl IntoIterator<Item = Field>,
    ) -> Self {
        Self {
            ident: ident.into(),
            offset,
            fields: HashMap::from_iter(
                fields.into_iter().map(|field| (field.ident.clone(), field)),
            ),
        }
    }

    #[expect(unused)]
    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        todo!()
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
        fields.sort_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset));

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
