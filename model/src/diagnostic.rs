use std::fmt::Display;

use colored::Colorize;
use derive_more::{AsRef, Deref};
use indexmap::{IndexMap, IndexSet};
use ters::ters;

use crate::{
    entitlement::Entitlement,
    field::{Field, FieldNode},
    model::{Model, View},
    register::RegisterNode,
    variant::VariantNode,
};

/// Elaborates diagnostics that may be emitted during model validation.
#[ters]
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Diagnostic {
    #[get]
    rank: Rank,
    #[get]
    kind: Kind,
    #[get]
    message: String,
    notes: Vec<String>,
    #[get]
    context: Context,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Kind {
    // physical
    AddressUnaligned,
    Overlap,
    ExceedsDomain,
    ExpectedReset,
    InvalidReset,
    InterruptExists,

    // stasis
    Unresolvable = 1000,
    ReadCannotBeInert,

    // lexical
    Reserved = 2000,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Rank {
    Warning,
    Error,
}

impl Diagnostic {
    pub fn new(rank: Rank, kind: Kind, message: impl Into<String>, context: Context) -> Self {
        Self {
            rank,
            kind,
            message: message.into(),
            notes: Default::default(),
            context,
        }
    }

    /// address must be word aligned
    ///
    /// note: address {address} does not satisfy: address % 4 == 0
    pub fn address_unaligned(address: u32, context: Context) -> Self {
        let level = context.level();
        let address = format!("0x{address:08x}").bold();

        Self::new(
            Rank::Error,
            Kind::AddressUnaligned,
            format!("{level} address must be word aligned"),
            context,
        )
        .notes([format!(
            "address {address} does not satisfy: address % 4 == 0"
        )])
    }

    /// [lhs] and [rhs] overlap, occupying {occupied}
    pub fn overlap(
        lhs: &impl Display,
        rhs: &impl Display,
        occupied: &impl Display,
        context: Context,
    ) -> Self {
        let level = context.child_level();
        let lhs = format!("{lhs}").bold();
        let rhs = format!("{rhs}").bold();
        let occupied = format!("{occupied}").bold();

        Self::new(
            Rank::Error,
            Kind::Overlap,
            format!("{level}s [{lhs}] and [{rhs}] overlap, occupying {occupied}"),
            context,
        )
    }

    /// [foo] with domain {offending_domain} exceeds parent [bar] with domain {parent_domain}
    pub fn exceeds_domain(
        offending: &impl Display,
        offending_domain: &impl Display,
        parent_domain: &impl Display,
        context: Context,
    ) -> Self {
        let level = context.level();
        let child_level = context.child_level();
        let offending_domain = format!("{offending_domain}").bold();
        let parent_domain = format!("{parent_domain}").bold();

        Self::new(
            Rank::Error,
            Kind::ExceedsDomain,
            format!(
                "{child_level} [{offending}] with domain {offending_domain} exceeds parent {level} with domain {parent_domain}"
            ),
            context,
        )
    }

    /// a reset value must be specified for registers containing resolvable fields
    ///
    /// note: resolvable fields: [...]
    pub fn expected_reset<'cx>(register: &View<'cx, RegisterNode>, context: Context) -> Self {
        let resolvable_fields = register
            .fields()
            .filter(|field| field.is_resolvable())
            .map(|field| field.module_name().to_string().bold().to_string())
            .collect::<Vec<_>>()
            .join(", ");

        Self::new(
            Rank::Error,
            Kind::ExpectedReset,
            "a reset value must be specified",
            context,
        )
        .notes([
            "reset values must be specified for registers containing resolvable fields".to_string(),
            format!("resolvable fields in this register: [{resolvable_fields}]"),
        ])
    }

    pub fn invalid_reset<'cx>(
        field: &View<'cx, FieldNode>,
        variants: impl Iterator<Item = View<'cx, VariantNode>>,
        field_reset: u32,
        register_reset: u32,
        context: Context,
    ) -> Self {
        let variants = variants
            .map(|variant| {
                format!(
                    "{}: {}",
                    variant.type_name().to_string().bold(),
                    format!("0x{:x}", variant.bits).bold()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        Self::new(
            Rank::Error,
            Kind::InvalidReset,
            format!(
                "no variants of field [{}] correspond to reset value {}",
                field.module_name().to_string().bold(),
                field_reset.to_string().bold(),
            ),
            context,
        )
        .notes([
            format!(
                "register reset value: {}",
                format!("0x{:x}", register_reset).bold(),
            ),
            format!("field variants: [{variants}]"),
        ])
    }

    /// interrupt [foo] at position {offending_position} is already defined at position {existing_position}
    pub fn interrupt_exists(
        offending: &impl Display,
        offending_position: &impl Display,
        existing_position: &impl Display,
        context: Context,
    ) -> Self {
        let offending = format!("{offending}").bold();
        let offending_position = format!("{offending_position}").bold();
        let existing_position = format!("{existing_position}").bold();

        Self::new(
            Rank::Error,
            Kind::InterruptExists,
            format!(
                "interrupt [{offending}] at position {offending_position} is already defined at position {existing_position}"
            ),
            context,
        )
    }

    /// entitlement [foo] resides within unresolvable field [bar] and as such cannot be entitled to
    pub fn unresolvable(
        model: &Model,
        entitlement: &Entitlement,
        field: &Field,
        context: Context,
    ) -> Self {
        Self::new(
            Rank::Error,
            Kind::Unresolvable,
            format!(
                "entitlement [{}] resides within unresolvable field [{}] and as such cannot be entitled to",
                entitlement.to_string(model).bold(),
                field.module_name().to_string().bold()
            ),
            context,
        )
    }

    /// read-only variants cannot be inert
    pub fn read_cannot_be_inert(context: Context) -> Self {
        Self::new(
            Rank::Error,
            Kind::ReadCannotBeInert,
            "read-only variants cannot be inert",
            context,
        )
    }

    /// "foo" is a reserved keyword
    ///
    /// note: reserved keywords: [...]
    pub fn reserved<R: AsRef<str>>(
        offending: &impl Display,
        bank: impl Iterator<Item = R>,
        context: Context,
    ) -> Self {
        let level = context.level().to_string();
        let offending = format!("{offending}").bold();

        let reserved = bank
            .map(|r| r.as_ref().bold().to_string())
            .collect::<Vec<_>>()
            .join(", ");

        Self::new(
            Rank::Error,
            Kind::ReadCannotBeInert,
            format!("\"{offending}\" is a reserved keyword for {level}s"),
            context,
        )
        .notes([format!("reserved {level} keywords: [{reserved}]")])
    }

    pub fn notes<I>(mut self, notes: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.notes
            .extend(notes.into_iter().map(|e| e.as_ref().to_string()));

        self
    }

    pub fn report(diagnostics: &Diagnostics) -> String {
        let mut diagnostic_groups = IndexMap::new();

        for diagnostic in diagnostics {
            diagnostic_groups
                .entry(diagnostic.context.clone())
                .or_insert(vec![])
                .push(diagnostic);
        }

        diagnostic_groups
            .iter()
            .map(|(context, diagnostics)| {
                let diagnostics = diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");

                if context.is_empty() {
                    diagnostics.to_string()
                } else {
                    format!("in {context}:\n{diagnostics}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let notes = if !self.notes.is_empty() {
            format!(
                "\n{}",
                self.notes
                    .iter()
                    .map(|note| format!("  {}: {note}", "note".bright_blue().bold()))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            String::new()
        };

        let code = format!("[E{:04}]", self.kind as u32);

        let header = match &self.rank {
            Rank::Warning => format!("warning{code}").yellow().bold(),
            Rank::Error => format!("error{code}").red().bold(),
        };

        write!(f, "{header}: {}{notes}", self.message)
    }
}

pub type Diagnostics = IndexSet<Diagnostic>;

impl From<Diagnostic> for Diagnostics {
    fn from(diagnostic: Diagnostic) -> Self {
        IndexSet::from([diagnostic])
    }
}

#[ters]
#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRef, Deref)]
pub struct Context {
    #[get]
    path: Vec<String>,
}

#[expect(clippy::new_without_default)]
impl Context {
    pub fn new() -> Self {
        Context { path: Vec::new() }
    }

    pub fn with_path(path: Vec<String>) -> Self {
        Self { path }
    }

    pub fn and(mut self, ident: String) -> Self {
        self.path.push(ident);
        self
    }

    fn level(&self) -> &str {
        match self.path.len() {
            1 => "peripheral",
            2 => "register",
            3 => "field",
            4 => "variant",
            _ => "",
        }
    }
    fn child_level(&self) -> &str {
        match self.path.len() {
            1 => "register",
            2 => "field",
            3 => "variant",
            _ => "",
        }
    }
}

impl Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.path
                .iter()
                .map(|segment| segment.bold().to_string())
                .collect::<Vec<_>>()
                .join("/")
        )
    }
}
