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
}
