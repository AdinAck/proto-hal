//! The abstract syntax tree.
//!
//! AST nodes implement [`Display`](fmt::Display) rendered as source text in canonical form.

pub mod list;
pub mod set;

use std::fmt::{self, Write};

use derive_more::Display;
use indenter::indented;
use itertools::Itertools;

pub use list::List;
pub use set::Set;

use crate::{
    token::{Doc, Ident, Literal, Token, token},
    util::{Parsed, Spanned},
};

/// A source file. (*See [`Item`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File<'src> {
    /// The [file doc comments](Doc) at its start.
    pub docs: Vec<Spanned<Doc<'src>>>,
    /// The [items](Item), in order.
    pub items: Vec<Spanned<Parsed<Item<'src>>>>,
}

/// An item at the top level of a [`File`].
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Item<'src> {
    /// An [`Import`].
    Import(Import<'src>),
    /// A [`Declaration`].
    Declaration(Declaration<'src>),
}

/// An import i.e. `import foo::bar`, `import foo::bar as baz`, `import foo::{self, bar}`. (*See [`ImportTree`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import<'src> {
    /// The [`ImportTree`], from the project root.
    pub tree: Spanned<ImportTree<'src>>,
}

/// The tree provided to an [`Import`] i.e. `foo::bar`, `foo::bar as baz`, `foo::{self, bar::{baz, qux}}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportTree<'src> {
    /// A [`ModulePath`], binding its last segment or the name given by `as` i.e. `foo::bar`, `foo::bar as baz`.
    Path {
        /// The [`ModulePath`].
        path: Spanned<ModulePath<'src>>,
        /// The name bound in place of the path's last segment, after `as`.
        alias: Option<Spanned<Ident<'src>>>,
    },
    /// The prefix of the enclosing group itself, written `self` i.e. `self`, `self as foo`.
    Self_ {
        /// The name bound in place of the prefix's last segment, after `as`.
        alias: Option<Spanned<Ident<'src>>>,
    },
    /// Trees sharing a prefix, within braces i.e. `foo::{bar, baz as qux}`.
    Group {
        /// The [`ModulePath`] the trees share.
        prefix: Spanned<ModulePath<'src>>,
        /// The trees, in order.
        trees: Vec<Spanned<ImportTree<'src>>>,
    },
}

/// A single value i.e. `0x14`, `foo`, `15:0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Value<'src> {
    /// A [`Literal`].
    Literal(Literal<'src>),
    /// An [`Ident`].
    Ident(Ident<'src>),
    /// Inclusive bits, most significant first i.e. `15:0`.
    #[display("{msb}:{lsb}")]
    Domain {
        /// The most significant bit.
        msb: Literal<'src>,
        /// The least significant bit.
        lsb: Literal<'src>,
    },
}

/// An inclusive or exclusive range between two [`Literal`]s with an optional step i.e. `0..16`, `4..=60 by 4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range<'src> {
    /// The literal starting the range.
    pub start: Literal<'src>,
    /// The literal ending the range.
    pub end: Literal<'src>,
    /// Whether [`end`](Range::end) is included (`..=`) or not (`..`).
    pub inclusive: bool,
    /// The distance between successive literals, written with `by`.
    pub step: Option<Step<'src>>,
}

/// A step, written after `by` i.e. `4`, `-0x14`. (*See [`Range`] and [`Progression`](list::Progression)*)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step<'src> {
    /// Whether the step is negative i.e. `-4`.
    pub negative: bool,
    /// The distance between successive [literals](Literal), [values](Value), or sequences ([ranges](Range) or
    /// [lists](List)).
    pub distance: Literal<'src>,
}

/// A name i.e. `foo`, `foo[a, b, c]`, `foo[1..=3]bar`. (*See [`Fragment`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name<'src> {
    /// The fragments, in order.
    pub fragments: Vec<Spanned<Fragment<'src>>>,
}

/// A fragment of a [`Name`] i.e. `foo`, `[a, b, c]`, `{1, 2}`, `[#foo]`.
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Fragment<'src> {
    /// Text, an [`Ident`].
    Text(Ident<'src>),
    /// A [`List`].
    List(List<'src>),
    /// A [`Set`].
    Set(Set<'src>),
    /// An iteration variable, inserted i.e. `[#foo]`.
    #[display("[#{_0}]")]
    Insertion(Ident<'src>),
}

/// A path of segments delineated by their [separator](PathSegment::SEPARATOR) i.e. `foo.bar`, `foo::bar`. (*See
/// [`DevicePath`] and [`ModulePath`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path<Segment> {
    /// The path segments, in order.
    pub segments: Vec<Spanned<Segment>>,
}

/// A segment of a [`Path`], which determines the segment separator.
pub trait PathSegment {
    /// The token separating segments i.e. `.`, `::`.
    const SEPARATOR: Token<'static>;
}

impl PathSegment for Name<'_> {
    const SEPARATOR: Token<'static> = token![.];
}

impl PathSegment for Ident<'_> {
    const SEPARATOR: Token<'static> = token![::];
}

/// A path to an item in the device i.e. `foo.bar`, `device.foo.Bar`, `foo{1, 2}.bar.{Baz, Qux}`. (*See [`Name`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevicePath<'src> {
    /// Whether the path starts at the device i.e. `device.foo`.
    pub rooted: bool,
    /// The path, of [names](Name).
    pub path: Path<Name<'src>>,
}

/// A path through the module tree, to a module or a definition within one i.e. `foo`, `foo::bar`.
pub type ModulePath<'src> = Path<Ident<'src>>;

/// An entitlement space i.e. `foo.Bar & baz.Qux | foo.Baz`. (*See [`Pattern`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitlementSpace<'src> {
    /// The [patterns](Pattern) specified in the entitlement space.
    pub patterns: Vec<Spanned<Pattern<'src>>>,
}

/// A pattern of an [`EntitlementSpace`] i.e. `foo.Bar & baz.Qux`. (*See [`DevicePath`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern<'src> {
    /// The [paths](DevicePath) specified in the pattern.
    pub paths: Vec<Spanned<DevicePath<'src>>>,
}

/// A property of an item i.e. `@ 0x14`, `reset 0`, `refines foo`, `write requires foo.Bar`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Property<'src> {
    /// The placement of an item within its parent, denoted by `@` i.e. `@ 0x14`, `@ [1:0, ...]`.
    Placement(Spanned<Argument<'src>>),
    /// The discriminant of a variant, denoted by `~` i.e. `~ 0`.
    Discriminant(Spanned<Argument<'src>>),
    /// A reset value i.e. `reset 0`.
    Reset(Spanned<Argument<'src>>),
    /// An assumed schema i.e. `assumes foo`.
    Assumes(Spanned<ModulePath<'src>>),
    /// A template refinement clause i.e. `refines foo, bar::baz`.
    Refines(Vec<Spanned<ModulePath<'src>>>),
    /// An entitlement space i.e. `requires foo.Bar`, `write requires foo.Bar`.
    Requires {
        /// The qualifiers preceding `requires`, if any.
        qualifiers: Option<RequirementQualifiers>,
        /// The [`EntitlementSpace`].
        space: Spanned<EntitlementSpace<'src>>,
    },
}

/// The argument of a [`Property`].
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Argument<'src> {
    /// A [`Value`].
    Value(Value<'src>),
    /// A [`List`].
    List(List<'src>),
}

/// The qualifiers preceding the `requires` [property](Property).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[display(rename_all = "lowercase")]
pub enum RequirementQualifiers {
    /// `write`.
    Write,
    /// `hardware write`.
    #[display("hardware write")]
    HardwareWrite,
}

/// A declaration i.e. `leaky store field foo[0..16] as bar @ [1:0, ...] assumes baz`. (*See [`Qualifier`],
/// [`DeclarationKind`], [`Property`], and [`Child`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration<'src> {
    /// The [doc comments](Doc) preceding the declaration.
    pub docs: Vec<Spanned<Doc<'src>>>,
    /// The [qualifiers](Qualifier), in order.
    pub qualifiers: Vec<Spanned<Parsed<Qualifier>>>,
    /// The [`DeclarationKind`].
    pub kind: Spanned<DeclarationKind>,
    /// The [`Name`].
    pub name: Spanned<Name<'src>>,
    /// The iteration variable naming the name's expansion, after `as` i.e. `bar` in `foo[a, b] as bar`.
    pub variable: Option<Spanned<Ident<'src>>>,
    /// The [properties](Property), in order.
    pub properties: Vec<Spanned<Parsed<Property<'src>>>>,
    /// The [children](Child) within the body, in order.
    pub children: Vec<Spanned<Parsed<Child<'src>>>>,
}

/// A qualifier preceding a [declaration](Declaration)'s kind i.e. `leaky`, `inert`, `read write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[display(rename_all = "lowercase")]
pub enum Qualifier {
    /// `leaky`.
    Leaky,
    /// `inert`.
    Inert,
    /// An [`Access`].
    Access(Access),
}

/// An access qualifier i.e. `read`, `read write`, `volatile store`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[display(rename_all = "lowercase")]
pub enum Access {
    /// `read`.
    Read,
    /// `write`.
    Write,
    /// `read write`.
    #[display("read write")]
    ReadWrite,
    /// `store`.
    Store,
    /// `volatile store`.
    #[display("volatile store")]
    VolatileStore,
}

/// The kind of a [`Declaration`] i.e. `register`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[display(rename_all = "lowercase")]
pub enum DeclarationKind {
    /// `device`.
    Device,
    /// `peripheral`.
    Peripheral,
    /// `register`.
    Register,
    /// `field`.
    Field,
    /// `schema`.
    Schema,
    /// `variant`.
    Variant,
    /// `group`.
    Group,
}

/// A child within a [declaration](Declaration)'s body.
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Child<'src> {
    /// A [`Declaration`].
    Declaration(Declaration<'src>),
    /// An [`InterruptTable`].
    Interrupts(InterruptTable<'src>),
}

/// An interrupt table i.e. `interrupts { foo @ 0 }`. (*See [`Interrupt`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptTable<'src> {
    /// The [interrupts](Interrupt), in order.
    pub interrupts: Vec<Spanned<Parsed<Interrupt<'src>>>>,
}

/// An interrupt of an [`InterruptTable`] i.e. `foo @ 0`, `foo[0..=4] @ [6, ...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interrupt<'src> {
    /// The [doc comments](Doc) preceding the interrupt.
    pub docs: Vec<Spanned<Doc<'src>>>,
    /// The [`Name`].
    pub name: Spanned<Name<'src>>,
    /// The iteration variable naming the name's expansion, after `as` i.e. `bar` in `foo[a, b] as bar`.
    pub variable: Option<Spanned<Ident<'src>>>,
    /// The [properties](Property), in order.
    pub properties: Vec<Spanned<Parsed<Property<'src>>>>,
}

impl fmt::Display for File<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.docs
                .iter()
                .map(|doc| Token::FileDoc(**doc))
                .format("\n")
        )?;

        if !self.docs.is_empty() && !self.items.is_empty() {
            write!(f, "\n\n")?;
        }

        if let Some(first) = self.items.first() {
            write!(f, "{first}")?;
        }

        for (previous, item) in self.items.iter().tuple_windows() {
            // note: consecutive imports are kept together, and everything else is separated by a blank line
            match (&**previous, &**item) {
                (Parsed::Node(Item::Import(..)), Parsed::Node(Item::Import(..))) => writeln!(f)?,
                _ => write!(f, "\n\n")?,
            }

            write!(f, "{item}")?;
        }

        Ok(())
    }
}

impl fmt::Display for Import<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "import {}", self.tree)
    }
}

impl fmt::Display for ImportTree<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path { path, alias } => write!(f, "{}{}", path, alias_suffix(alias)),
            Self::Self_ { alias } => write!(f, "self{}", alias_suffix(alias)),
            Self::Group { prefix, trees } => {
                write!(f, "{}::{{{}}}", prefix, trees.iter().format(", "))
            }
        }
    }
}

impl fmt::Display for Range<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = if self.inclusive { "..=" } else { ".." };
        write!(f, "{}{op}{}", self.start, self.end)?;

        if let Some(step) = self.step {
            write!(f, " by {step}")?;
        }

        Ok(())
    }
}

impl fmt::Display for Step<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            write!(f, "-")?;
        }

        write!(f, "{}", self.distance)
    }
}

impl fmt::Display for Name<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.fragments.iter().format(""))
    }
}

impl<Segment: PathSegment + fmt::Display> fmt::Display for Path<Segment> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let separator = Segment::SEPARATOR.to_string();

        write!(f, "{}", self.segments.iter().format(&separator))
    }
}

impl fmt::Display for DevicePath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.rooted {
            write!(f, "device.")?;
        }

        write!(f, "{}", self.path)
    }
}

impl fmt::Display for EntitlementSpace<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.patterns.iter().format(" | "))
    }
}

impl fmt::Display for Pattern<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.paths.iter().format(" & "))
    }
}

impl Property<'_> {
    /// Determine whether the property is intrinsic, introduced by a symbol i.e. `@ 0x14`, `~ 0`.
    pub fn is_intrinsic(&self) -> bool {
        matches!(self, Self::Placement(..) | Self::Discriminant(..))
    }
}

impl fmt::Display for Property<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Placement(argument) => write!(f, "@ {argument}"),
            Self::Discriminant(argument) => write!(f, "~ {argument}"),
            Self::Reset(argument) => write!(f, "reset {argument}"),
            Self::Assumes(definition) => write!(f, "assumes {definition}"),
            Self::Refines(definitions) => write!(f, "refines {}", definitions.iter().format(", ")),
            Self::Requires { qualifiers, space } => {
                if let Some(qualifiers) = qualifiers {
                    write!(f, "{qualifiers} ")?;
                }

                write!(f, "requires {space}")
            }
        }
    }
}

impl fmt::Display for Declaration<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for doc in &self.docs {
            writeln!(f, "{}", Token::Doc(**doc))?;
        }

        for qualifier in &self.qualifiers {
            write!(f, "{qualifier} ")?;
        }

        write!(f, "{} {}", self.kind, self.name)?;

        if let Some(variable) = &self.variable {
            write!(f, " as {variable}")?;
        }

        write_properties(f, &self.properties)?;

        if !self.children.is_empty() {
            // note: a head spanning lines puts the body's `{` on a line of its own
            match self
                .properties
                .iter()
                .all(|property| is_intrinsic(property))
            {
                true => write!(f, " ")?,
                false => writeln!(f)?,
            }

            write_block(f, &self.children)?;
        }

        Ok(())
    }
}

impl fmt::Display for InterruptTable<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "interrupts ")?;
        write_block(f, &self.interrupts)
    }
}

impl fmt::Display for Interrupt<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for doc in &self.docs {
            writeln!(f, "{}", Token::Doc(**doc))?;
        }

        write!(f, "{}", self.name)?;

        if let Some(variable) = &self.variable {
            write!(f, " as {variable}")?;
        }

        write_properties(f, &self.properties)
    }
}

/// Write intrinsic `properties` on the same line, and the rest on lines of their own with indentation.
fn write_properties(
    f: &mut fmt::Formatter<'_>,
    properties: &[Spanned<Parsed<Property>>],
) -> fmt::Result {
    let (intrinsic, worded): (Vec<_>, Vec<_>) = properties
        .iter()
        .partition(|property| is_intrinsic(property));

    for property in intrinsic {
        write!(f, " {property}")?;
    }

    for property in worded {
        write!(f, "\n    {property}")?;
    }

    Ok(())
}

/// Write `nodes` within braces, one per line, indented.
fn write_block<T: fmt::Display>(f: &mut fmt::Formatter<'_>, nodes: &[Spanned<T>]) -> fmt::Result {
    write!(f, "{{")?;

    for node in nodes {
        writeln!(f)?;
        write!(indented(f).with_str("    "), "{}", node)?;
    }

    if !nodes.is_empty() {
        writeln!(f)?;
    }

    write!(f, "}}")
}

/// Display the name an import binds in place of a path's last segment, after `as`, if any.
fn alias_suffix<'a>(alias: &'a Option<Spanned<Ident<'_>>>) -> impl fmt::Display + 'a {
    fmt::from_fn(move |f| match alias {
        Some(alias) => write!(f, " as {alias}"),
        None => Ok(()),
    })
}

/// Determine whether a parsed property is intrinsic. A property that failed to parse is not.
fn is_intrinsic(property: &Parsed<Property>) -> bool {
    matches!(property, Parsed::Node(property) if property.is_intrinsic())
}
