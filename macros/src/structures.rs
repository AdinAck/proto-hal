use darling::FromMeta;
use proc_macro2::Span;
use syn::{spanned::Spanned as _, Attribute};

use crate::utils::Spanned;

pub mod block;
pub mod field;
pub mod field_array;
pub mod interrupts;
pub mod register;
pub mod schema;
pub mod variant;
pub mod variant_array;
pub mod entitlement;

pub trait Args: FromMeta + Sized {
    const NAME: &str;

    fn get<'a>(attrs: impl Iterator<Item = &'a Attribute>) -> syn::Result<Option<Spanned<Self>>> {
        let mut found = None;

        for attr in attrs {
            if attr.path().is_ident(Self::NAME) {
                if found.is_none() {
                    found.replace(Self::from_meta(&attr.meta)?.with_span(attr.meta.span()));
                } else {
                    Err(syn::Error::new_spanned(
                        attr,
                        format!(
                            "the '{}' attribute can only be used once per item",
                            Self::NAME
                        ),
                    ))?
                }
            }
        }

        Ok(found)
    }

    fn with_span(self, span: Span) -> Spanned<Self> {
        Spanned::new(self, span)
    }
}
