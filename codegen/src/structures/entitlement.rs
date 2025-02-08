use syn::{spanned::Spanned, Ident, Path};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Locality {
    Local,
    External(Ident),
}

#[derive(Debug)]
pub struct Local;
#[derive(Debug)]
pub struct External(pub Ident);

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Entitlement<Peripheral, Register> {
    pub peripheral: Peripheral,
    pub register: Register,
    pub field: Ident,
    pub state: Ident,
}

pub type Unrefined = Entitlement<Locality, Locality>;

impl Unrefined {
    pub fn from_path(path: &Path) -> syn::Result<Self> {
        let mut segments = path.segments.iter().rev();

        let state = segments
            .next()
            .ok_or(syn::Error::new(
                path.span(),
                "entitlement path must resolve to a state",
            ))?
            .ident
            .clone();
        let field = segments
            .next()
            .ok_or(syn::Error::new(
                path.span(),
                "entitlement path must specify a field",
            ))?
            .ident
            .clone();
        let register = segments
            .next()
            .map_or(Locality::Local, |s| Locality::External(s.ident.clone()));
        let peripheral = segments
            .next()
            .map_or(Locality::Local, |s| Locality::External(s.ident.clone()));

        Ok(Self {
            peripheral,
            register,
            field,
            state,
        })
    }
}

impl<Peripheral> Entitlement<Peripheral, Locality> {
    pub fn refine_register_as_external(self) -> Result<Entitlement<Peripheral, External>, Self> {
        let Locality::External(register) = self.register else {
            return Err(self);
        };

        Ok(Entitlement {
            peripheral: self.peripheral,
            register: External(register),
            field: self.field,
            state: self.state,
        })
    }
}
