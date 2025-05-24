use log::trace;
use syn::{parse_quote, Path};
use ters::ters;

#[ters]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Entitlement {
    #[get]
    path: String,
}

impl Entitlement {
    pub fn to(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    pub fn render(&self) -> Path {
        trace!("Rendering entitlement with path: \"{}\".", self.path());

        let path = syn::parse_str::<Path>(self.path()).unwrap();
        parse_quote! {
            crate::#path
        }
    }
}
