use syn::{spanned::Spanned, Ident, Path};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Locality {
    Local,
    External(Ident),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Entitlement {
    pub peripheral: Locality,
    pub register: Locality,
    pub field: Ident,
    pub state: Ident,
}

impl Entitlement {
    pub fn from_path(path: &Path) -> syn::Result<Self> {
        let mut segments = path.segments.iter().rev();

        let state = segments.next().ok_or(syn::Error::new(path.span(), "entitlement path must resolve to a state"))?.ident.clone();
        let field = segments.next().ok_or(syn::Error::new(path.span(), "entitlement path must specify a field"))?.ident.clone();
        let register = segments.next().map_or(Locality::Local, |s| Locality::External(s.ident.clone()));
        let peripheral = segments.next().map_or(Locality::Local, |s| Locality::External(s.ident.clone()));

        Ok(Self {
            peripheral,
            register,
            field,
            state,
        })
    }
}
