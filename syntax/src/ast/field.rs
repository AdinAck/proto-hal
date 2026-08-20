//! Fields: spans of bits within a register.

use super::{Access, Domain, Head, Path, ResetValue, Space, Span, Spanned, Variant};

/// `access field name @ domain { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct Field<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    /// `leaky` — all interactions with the field are `unsafe`.
    pub leaky: Option<Span>,
    /// Access modality words, as written (a *set* — see [`Access`]).
    pub access: Vec<Spanned<Access>>,
    /// The span of the `array` keyword.
    pub array: Option<Span>,
    pub head: Head<'src>,
    /// `@` — the bit domain(s) within the parent register. Field arrays may
    /// auto-increment (`[0..=1, ...]`): the field width is known.
    pub domain: Option<Spanned<Domain>>,
    /// `assumes path` — the field's schema is *linked*: it reflects the
    /// referenced schema exactly, re-exporting its types. The schema must
    /// therefore be placed somewhere in the device.
    pub assumes: Option<Spanned<Path<'src>>>,
    /// `extends a, b` — the field's schema is *inherent*, extending the
    /// referenced schemas with its own variants. A field may extend many
    /// schemas: comma lists and repeated clauses accumulate. The copied
    /// definitions land in the extending field's module, so extension
    /// requires no schema placement.
    pub extends: Vec<Spanned<Path<'src>>>,
    /// The field's reset value — a number or one of its variants by name.
    /// The parent register may also specify one; where both exist they must
    /// agree.
    pub reset: Option<Spanned<ResetValue<'src>>>,
    pub requires: FieldRequires<'src>,
    pub body: Option<Spanned<Vec<Spanned<FieldItem<'src>>>>>,
}

/// A field's entitlements, one space per kind.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FieldRequires<'src> {
    /// `requires ...`
    ///
    /// when the `requires` property is specified alone, it corresponds with the *ontological* entitlement space
    /// of the item.
    pub inherent: Option<Spanned<Space<'src>>>,

    /// `write requires ...`
    ///
    /// when the `requires` property is specified after `write` (spelling *write requires*), it corresponds with the
    /// *affordance* entitlement space of the item.
    pub write: Option<Spanned<Space<'src>>>,
    /// `hardware write requires ...`
    ///
    /// when the `requires` property is specified after `hardware write` (spelling *hardware write requires*), it
    /// corresponds with the *hardware affordance* entitlement space of the item.
    pub hardware_write: Option<Spanned<Space<'src>>>,
}

/// What may appear within a field: variants only.
///
/// Notably *not* schema placements — a field's inherent variants are private
/// to it, so fields cannot host shareable definitions.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldItem<'src> {
    Variant(Variant<'src>),
    /// A region that failed to parse. The error has already been reported;
    /// elaboration skips these.
    Error,
}
