pub struct Warning(pub String);
pub struct Error(pub String);

impl Warning {
    pub fn with_context(self, provider: &impl ContextProvider) -> Diagnostic {
        Diagnostic {
            kind: Kind::Warning(self),
            context: provider.context(),
        }
    }
}

impl Error {
    pub fn with_context(self, provider: &impl ContextProvider) -> Diagnostic {
        Diagnostic {
            kind: Kind::Error(self),
            context: provider.context(),
        }
    }
}

pub enum Kind {
    Warning(Warning),
    Error(Error),
}

pub struct Diagnostic {
    kind: Kind,
    context: Context,
}

pub type Context = String;

pub trait ContextProvider {
    fn context(&self) -> Context;
}
