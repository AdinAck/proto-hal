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
                fields
                    .into_iter()
                    .map(|field| (field.ident.to_string().clone(), field)),
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
                        lhs.ident.to_string().bold(),
                        rhs.ident.to_string().bold()
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
                        field.ident.to_string().bold()
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
        let mut body = quote! {};

        let ident = format_ident!("{}", self.ident);
        let offset = self.offset;

        // field bodies
        for field in self.fields.values() {
            body.extend(field.to_token_stream());
        }

        // unsafe interface
        if self.fields.values().any(|field| field.access.is_read()) {
            // reader

            let readable_numeric_fields = self.fields.values().filter_map(|field| {
                if field.access.is_read() {
                    Some(&field.ident)
                } else {
                    None
                }
            });
            // let readable_numeric_field_idents = self
            //     .fields()
            //     .readable()
            //     .numeric(AccessMarker::Read)
            //     .idents();
            // let readable_enumerated_field_idents = self
            //     .fields()
            //     .readable()
            //     .enumerated(AccessMarker::Read)
            //     .idents();

            // let value_tys = readable_numeric_fields
            //     .map(|field| {
            //         let ident = format_ident!(
            //             "u{}",
            //             Index {
            //                 index: field.width() as _,
            //                 span: Span::call_site(),
            //             }
            //         );

            //         match field.width() {
            //             1 => parse_quote! { bool },
            //             8 | 16 | 32 => {
            //                 parse_quote! { #ident }
            //             }
            //             _ => {
            //                 parse_quote! { ::proto_hal::macro_utils::arbitrary_int::#ident }
            //             }
            //         }
            //     })
            //     .collect::<Vec<Path>>();

            // body.extend(quote! {
            //     pub struct UnsafeReader {
            //         value: ::proto_hal::ir_utils::RegisterValue,
            //     }

            //     impl UnsafeReader {
            //         const fn new(value: u32) -> Self {
            //             Self {
            //                 value: ::proto_hal::ir_utils::RegisterValue::new(value),
            //             }
            //         }

            //         #(
            //             pub fn #readable_enumerated_field_idents(&self) -> #readable_enumerated_field_idents::ReadVariant {
            //                 // SAFETY: assumes
            //                 // 1. peripheral description is correct (offset/width)
            //                 // 2. hardware is operating correctly
            //                 unsafe {
            //                     #readable_enumerated_field_idents::ReadVariant::from_bits(
            //                         self.value.region(
            //                             #readable_enumerated_field_idents::OFFSET,
            //                             #readable_enumerated_field_idents::WIDTH
            //                         )
            //                     )
            //                 }
            //             }
            //         )*

            //         #(
            //             pub fn #readable_numeric_field_idents(&self) -> #value_tys {
            //                 self.value.#value_tys(#readable_numeric_field_idents::OFFSET)
            //             }
            //         )*
            //     }
            // });

            body.extend(quote! {
                pub unsafe fn read() -> UnsafeReader {
                    UnsafeReader::new(
                        ::core::ptr::read_volatile((super::BASE_ADDR + OFFSET) as *const u32)
                    )
                }
            });
        }

        if self.fields.values().any(|field| field.access.is_write()) {
            body.extend(quote! {
                pub unsafe fn write(f: impl FnOnce(&mut UnsafeWriter) -> &mut UnsafeWriter) {
                    let mut writer = UnsafeWriter::new();

                    f(&mut writer);

                    ::core::ptr::write_volatile((super::BASE_ADDR + OFFSET) as *mut u32, writer.value);
                }
            });
        }

        // offset constant
        body.extend(quote! {
            pub const OFFSET: u32 = #offset;
        });

        tokens.extend(quote! {
            pub mod #ident {
                #body
            }
        });
    }
}
