use crate::{field::access::Access, variant::ParentIndex};

#[derive(Debug, Clone)]
pub enum Source {
    Inherent(Access),
    Linked { parent: ParentIndex, access: Access },
}

impl Source {
    /// Get the [`Access`] of the field whether it is [`inherent`](Source::Inherent) or [`linked`](Source::Linked).
    pub fn access(&self) -> &Access {
        match self {
            Self::Inherent(access) => access,
            Self::Linked { access, .. } => access,
        }
    }

    /// Get an [`inherent`](Source::Inherent) [`Access`] or [`None`].
    pub fn inherent(&self) -> Option<&Access> {
        match self {
            Self::Inherent(access) => Some(access),
            Self::Linked { .. } => None,
        }
    }

    /// [`inherent`](Source::inherent) but mutable.
    pub(crate) fn inherent_mut(&mut self) -> Option<&mut Access> {
        match self {
            Self::Inherent(access) => Some(access),
            Self::Linked { .. } => None,
        }
    }

    /// Get a [`linked`](Source::Linked) [`Access`] or [`None`].
    pub fn linked(&self) -> Option<&Access> {
        match self {
            Self::Linked { access, .. } => Some(access),
            Self::Inherent(..) => None,
        }
    }
}
