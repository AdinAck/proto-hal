//! Semantic analysis: [`File`] → `Composition`.
//!
//! Elaboration is where the tree becomes hardware: templates are expanded,
//! arrays unrolled into elements, requiredness judged ("neither the definition
//! nor its template specify an offset"), and everything corporeal — the
//! contents of the `device` — is composed into the model.
//!
//! Diagnostics are [`Semantic`]: anchored to source spans, rendered by
//! [`report`](super::report). Elaboration never halts on an error; it emits
//! and continues with the healthy remainder, exactly like the parser.
//!
//! # Phases
//!
//! Elaboration is two-phase, mirroring the model's declaration-tree
//! construction interface:
//!
//! 1. **Structure** — the device is walked once, building [declaration
//!    trees](::model::decl) and inserting them whole. Entitlement clauses are
//!    recorded, not applied.
//! 2. **Entitlements** — with every structure in existence, the recorded
//!    clauses are resolved and registered. Declaration order therefore
//!    carries no meaning.
//!
//! # Entitlements
//!
//! Paths are anchored at the device root and name every module level, groups
//! included, mirroring the generated HAL:
//! `p_group.foo.r_group.foo0.f_group.a.V5`. Cycles among statewise
//! requirements are detected and forbidden.
//!
//! # Imports
//!
//! Elaboration receives one [`Unit`] per loaded file: the entry file first,
//! then its imports (transitively). Template references resolve against the
//! *referencing definition's* file — a single segment names that file's own
//! top-level definitions, and `alias.name` reaches through one of its
//! imports. Spans carry their [`SourceId`], so definitions merged across
//! files still attribute diagnostics correctly.
//!
//! # Not yet elaborated
//!
//! Constructs the model cannot yet express are parsed, merged, and then
//! *reported* rather than silently dropped: variant leakiness. Each carries
//! a warning so a model description's remaining distance from full
//! elaboration is always visible.

mod expand;
mod merge;
mod references;
mod shared;
mod structure;
mod templates;

use std::collections::HashMap;

use ::model::{
    Composition, Model,
    decl::{SchemaDecl, Side},
    field::{FieldIndex, access, numericity::Numericity},
    schema::SchemaIndex,
    variant::VariantIndex,
};
use heck::ToSnakeCase as _;
use proc_macro2::Span as IdentSpan;
use syn::Ident;

use syntax::ast::{
    Device, Entitled, Field, File, FileItem, Peripheral, Register, Schema, SourceId, Space, Span, Spanned, Variant,
};

use crate::model::semantic::{Diagnostic, Kind};

/// One parsed file and its import aliases.
pub struct Unit<'src> {
    pub file: File<'src>,
    /// The referenceable name of each import — its final path segment — and
    /// the source it loads.
    pub imports: HashMap<String, SourceId>,
}

/// The location of a variant as written: every module level of the generated
/// HAL, by element name — groups included.
type VariantPath = Vec<String>;

/// The chain that locates an item in the *model*: `(peripheral, register,
/// field)` element names. Groups are excluded — model lookups are by member.
type Chain = (String, String, String);

/// Where an element came from.
#[derive(Clone, Copy)]
pub struct Location {
    /// The head: the name through the last head property.
    pub head: Span,
    /// The `@` position — or a variant's `~` value — when written.
    pub domain: Option<Span>,
}

/// The model's context path for an item — its snake_cased element names —
/// mapped to where it came from. The model judges itself by context path;
/// this table anchors those judgements back to the source text.
pub type Locations = HashMap<Vec<String>, Location>;

/// Elaborate parsed files — the entry first, then its imports — into a model
/// composition.
///
/// The composition is returned even when diagnostics contain errors: it holds
/// whatever could be built, and the caller decides whether to proceed.
pub fn elaborate<'src>(units: &[Unit<'src>]) -> (Composition, Vec<Diagnostic>, Locations) {
    let mut diagnostics = Vec::new();

    let mut devices = units[0].file.items.iter().filter_map(|item| match &item.inner {
        FileItem::Device(device) => Some((device, item.span)),
        _ => None,
    });

    let (device, span) = match (devices.next(), devices.next()) {
        (None, ..) => {
            diagnostics.push(Diagnostic::no_device(Span {
                start: 0,
                end: 0,
                context: 0,
            }));
            return (Composition::new(), diagnostics, Locations::new());
        }
        (Some(device), None) => device,
        (Some((.., first)), Some((.., span))) => {
            diagnostics.push(Diagnostic::many_devices(span, first));
            return (Composition::new(), diagnostics, Locations::new());
        }
    };

    let mut context = Context {
        units,
        templates: units
            .iter()
            .map(|unit| Templates::collect(&unit.file))
            .collect(),
        variants: HashMap::new(),
        schemas: HashMap::new(),
        locations: Locations::new(),
        used_devices: std::collections::HashSet::new(),
        placements: Vec::new(),
        links: Vec::new(),
        pending: Vec::new(),
        resets: Vec::new(),
        edges: Vec::new(),
        diagnostics,
    };

    let mut composition = Composition::new();

    // phase one: structure
    if let Some(device) = context.resolve_device(device) {
        context.device(&mut composition, &device, span);
    }
    context.place_schemas(&mut composition);

    // phase two: references — every structure exists by now, so schema
    // assumptions and entitlements may point anywhere in the device
    context.links(&mut composition);
    context.resets(&mut composition);
    context.apply(&mut composition);

    context.detect_cycles();

    // only the entry file's device elaborates — an imported one is fine as
    // long as it served as a template. A family file offering several device
    // templates warns only when *none* of them was used.
    for unit in &units[1..] {
        let devices = unit.file.items.iter().filter_map(|item| match &item.inner {
            FileItem::Device(device) => Some((device, item.span)),
            _ => None,
        });

        let unused = devices.clone().all(|(device, ..)| {
            !context
                .used_devices
                .contains(&(device as *const Device as usize))
        });

        if !unused {
            continue;
        }

        for (.., span) in devices {
            context
                .diagnostics
                .push(Diagnostic::device_not_elaborated(span));
        }
    }

    (composition, context.diagnostics, context.locations)
}

/// The top-level definitions of the model description, usable as templates.
#[derive(Default)]
struct Templates<'ast, 'src> {
    devices: HashMap<&'src str, &'ast Device<'src>>,
    peripherals: HashMap<&'src str, &'ast Peripheral<'src>>,
    registers: HashMap<&'src str, &'ast Register<'src>>,
    fields: HashMap<&'src str, &'ast Field<'src>>,
    schemas: HashMap<&'src str, &'ast Schema<'src>>,
    variants: HashMap<&'src str, &'ast Variant<'src>>,
}

impl<'ast, 'src> Templates<'ast, 'src> {
    fn collect(file: &'ast File<'src>) -> Self {
        let mut templates = Self::default();

        for item in &file.items {
            match &item.inner {
                FileItem::Device(device) => {
                    if let Some(name) = &device.head.name {
                        templates.devices.insert(name.inner, device);
                    }
                }
                FileItem::Peripheral(peripheral) => {
                    if let Some(name) = &peripheral.head.name {
                        templates.peripherals.insert(name.inner, peripheral);
                    }
                }
                FileItem::Register(register) => {
                    if let Some(name) = &register.head.name {
                        templates.registers.insert(name.inner, register);
                    }
                }
                FileItem::Field(field) => {
                    if let Some(name) = &field.head.name {
                        templates.fields.insert(name.inner, field);
                    }
                }
                FileItem::Schema(schema) => {
                    if let Some(name) = &schema.head.name {
                        templates.schemas.insert(name.inner, schema);
                    }
                }
                FileItem::Variant(variant) => {
                    if let Some(name) = &variant.head.name {
                        templates.variants.insert(name.inner, variant);
                    }
                }
                _ => {}
            }
        }

        templates
    }
}

/// How many template derivations deep resolution will follow before deciding
/// it has found a cycle.
const TEMPLATE_DEPTH_LIMIT: u32 = 32;

/// An entitlement registration deferred to phase two.
struct Pending<'src> {
    target: Target,
    space: Spanned<Space<'src>>,
    /// The array definitions enclosing the writer, outermost first — array
    /// correspondences in the space bind against these.
    arrays: Vec<Enclosing>,
}

/// An enclosing array definition: the ordinal of the element being
/// elaborated, and how many elements the array has.
#[derive(Clone, Copy)]
struct Enclosing {
    ordinal: usize,
    length: usize,
    /// The array's element designators, for "the enclosing array" labels.
    span: Span,
}

/// Why an entitled path's array correspondence cannot resolve.
enum Correspondence {
    /// A bracketed segment written with no array enclosing the writer.
    OutsideArray(Span),
    /// More than one array encloses the writer — which one the bracket
    /// binds is ambiguous.
    Nested(Span),
    /// A second bracketed segment in one path.
    Several { first: Span, second: Span },
    /// The bracket names a different number of targets than the enclosing
    /// array has elements.
    Length { span: Span, targets: usize },
}

/// The written names of an entitled path — array correspondences resolved
/// against the writer's enclosing arrays.
///
/// A bracketed segment names one target per element of the single enclosing
/// array, zipped by ordinal: `ccr[2..10]` written from element 3 resolves to
/// `ccr5`. Correspondence is by *name* — the bracket writes target names, so
/// offset ranges are permitted.
fn corresponding(
    entitled: &Spanned<Entitled>,
    arrays: &[Enclosing],
) -> Result<Vec<(String, Span)>, Correspondence> {
    let mut bracketed: Option<Span> = None;

    entitled
        .inner
        .segments
        .iter()
        .map(|segment| {
            let name = match &segment.inner.elements {
                None => segment.inner.name.to_string(),
                Some(range) => {
                    if let Some(first) = bracketed {
                        return Err(Correspondence::Several {
                            first,
                            second: segment.span,
                        });
                    }

                    bracketed = Some(segment.span);

                    let array = match arrays {
                        [] => return Err(Correspondence::OutsideArray(segment.span)),
                        [array] => array,
                        [..] => return Err(Correspondence::Nested(segment.span)),
                    };

                    let targets = (range.end.inner + range.inclusive as u32)
                        .saturating_sub(range.start.inner) as usize;

                    if targets != array.length {
                        return Err(Correspondence::Length {
                            span: segment.span,
                            targets,
                        });
                    }

                    format!(
                        "{}{}",
                        segment.inner.name,
                        range.start.inner + array.ordinal as u32,
                    )
                }
            };

            Ok((name, segment.span))
        })
        .collect()
}

/// A schema reference deferred to phase two.
struct Link<'src> {
    /// The field, in the model.
    chain: Chain,
    /// The field, as written (groups included) — where an assumed schema's
    /// variants become referenceable.
    path: VariantPath,
    /// The field's declared modality, for schema compatibility.
    modality: Modality,
    /// The field's access words, for "declared here" labels.
    modality_span: Span,
    reference: Reference<'src>,
}

/// What a field's schema reference means.
enum Reference<'src> {
    /// `assumes` — exact reflection of one placed schema.
    Assume(Spanned<syntax::ast::Path<'src>>),
    /// `extends` — inherent copies from each referenced schema.
    Extend(Vec<Spanned<syntax::ast::Path<'src>>>),
}

/// A declared variant, array-expanded.
struct DeclaredVariant<'src> {
    name: String,
    variant: ::model::Variant,
    /// The declaration site.
    span: Span,
    /// The numericity the variant occupies, when written.
    side: Option<Spanned<syntax::ast::Side>>,
    /// Statewise requirements, forbidden within schemas.
    requires: Option<Spanned<Space<'src>>>,
}

/// What elaboration knows about a placed schema.
#[derive(Clone)]
struct PlacedSchema {
    /// Declared variants: name (as written), and — for sided ones — the side
    /// with its token's span.
    variants: Vec<(String, Option<(Side, Span)>)>,
    /// The model index, once inserted.
    index: Option<SchemaIndex>,
    /// The definition site — the schema's name at its placement.
    span: Span,
}

/// A schema placement awaiting insertion.
struct Placement {
    /// The scope its name is visible within.
    scope: VariantPath,
    locator: Locator,
    decl: SchemaDecl,
}

/// Where a placement manifests, by element name.
enum Locator {
    Root,
    PeripheralGroup(String),
    Peripheral(String),
    RegisterGroup(String),
    Register(String, String),
    FieldGroup(String),
}

/// What a pending registration applies to.
enum Target {
    /// Ontological entitlements of a peripheral.
    Peripheral(String),
    /// Ontological entitlements of a field.
    Field(Chain),
    /// Write entitlements of a field.
    Write(Chain),
    /// Hardware write entitlements of a field.
    HardwareWrite(Chain),
    /// Statewise entitlements of a variant.
    Variant(Chain, String),
}

struct Context<'ast, 'src> {
    units: &'ast [Unit<'src>],
    /// Per-file top-level definitions, indexed by [`SourceId`].
    templates: Vec<Templates<'ast, 'src>>,
    /// Every declared variant: its full written path (groups included) → the
    /// chain that locates it in the model.
    variants: HashMap<VariantPath, (Chain, String)>,
    /// Every placed schema, by scope and name.
    schemas: HashMap<(VariantPath, String), PlacedSchema>,
    /// Model context path → definition span, for anchoring the model's own
    /// judgements to the source text.
    locations: Locations,
    /// Imported devices that served as templates (by address) — they earn no
    /// not-elaborated warning.
    used_devices: std::collections::HashSet<usize>,
    /// Placements awaiting insertion, once the structures they sit within
    /// exist.
    placements: Vec<Placement>,
    /// Schema references awaiting phase two.
    links: Vec<Link<'src>>,
    /// Entitlement registrations awaiting phase two.
    pending: Vec<Pending<'src>>,
    /// Field resets given by variant name, awaiting phase two — the variant
    /// may come from a schema the field links to.
    resets: Vec<(Chain, String, Span)>,
    /// Statewise requirement edges (source variant, target variant, clause
    /// span), for cycle detection.
    edges: Vec<(VariantPath, VariantPath, Span)>,
    diagnostics: Vec<Diagnostic>,
}


#[derive(Clone, Copy, PartialEq)]
enum Modality {
    Read,
    Write,
    ReadWrite,
    Store,
    VolatileStore,
}

impl std::fmt::Display for Modality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Modality::Read => "read",
            Modality::Write => "write",
            Modality::ReadWrite => "read write",
            Modality::Store => "store",
            Modality::VolatileStore => "volatile store",
        })
    }
}

/// The (variant-less) access a modality describes.
fn modality_access(modality: Modality) -> access::Access {
    match modality {
        Modality::Read => access::Read::default().into(),
        Modality::Write => access::Write::default().into(),
        Modality::ReadWrite => access::ReadWrite::default().into(),
        Modality::Store => access::Store::default().into(),
        Modality::VolatileStore => access::VolatileStore::default().into(),
    }
}

/// The judgement of a variant side against a container's modality: [`None`]
/// when permitted — including the redundant case of a side matching a
/// single-modality container — or the kind of the violation.
fn side_verdict(modality: Modality, side: Side) -> Option<Kind> {
    match (modality, side) {
        (Modality::ReadWrite, ..) => None,
        (Modality::Read, Side::Read) | (Modality::Write, Side::Write) => None,
        (Modality::Read, Side::Write) | (Modality::Write, Side::Read) => {
            Some(Kind::ContradictorySide)
        }
        (Modality::Store | Modality::VolatileStore, ..) => Some(Kind::InvalidSide),
    }
}

/// The written word of a side.
fn side_name(side: Side) -> &'static str {
    match side {
        Side::Read => "read",
        Side::Write => "write",
    }
}

/// The head span of a definition: its name through the end of its last head
/// property — the area a diagnostic about the whole item points at, rather
/// than an unreadable span over its entire body.
///
/// Template merging mixes spans across files, so only properties written in
/// the same file as the name extend the span.
fn head_span(name: Option<&Spanned<&str>>, fallback: Span, ends: &[Option<Span>]) -> Span {
    let Some(name) = name else {
        return fallback;
    };

    let end = ends
        .iter()
        .flatten()
        .filter(|span| span.context == name.span.context)
        .map(|span| span.end)
        .fold(name.span.end, usize::max);

    Span {
        start: name.span.start,
        end,
        context: name.span.context,
    }
}

/// The model-side counterpart of a written variant side.
fn side_decl(side: &Spanned<syntax::ast::Side>) -> Side {
    match side.inner {
        syntax::ast::Side::Read => Side::Read,
        syntax::ast::Side::Write => Side::Write,
    }
}

/// The best near-miss among candidates — worth suggesting only within an
/// edit-distance budget scaled to the name's length.
fn did_you_mean(name: &str, candidates: impl IntoIterator<Item = String>) -> Option<String> {
    let budget = (name.len() / 3).max(1);

    candidates
        .into_iter()
        .filter(|candidate| candidate != name)
        .map(|candidate| (strsim::damerau_levenshtein(name, &candidate), candidate))
        .filter(|(distance, ..)| *distance <= budget)
        .min_by_key(|(distance, ..)| *distance)
        .map(|(.., candidate)| candidate)
}

/// Every variant name of a field, as declared.
fn variant_names(model: &Model, chain: &Chain) -> Vec<String> {
    let (peripheral, register, field) = chain;

    let Some(peripheral) = model.try_get_peripheral(ident(peripheral).into()) else {
        return Vec::new();
    };
    let Some(register) = peripheral.try_get_register(&ident(register)) else {
        return Vec::new();
    };
    let Some(field) = register.try_get_field(&ident(field)) else {
        return Vec::new();
    };

    [
        field.access.access().get_read(),
        field.access.access().get_write(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|numericity| match numericity {
        Numericity::Enumerated(enumerated) => Some(enumerated.variants(model)),
        Numericity::Numeric(..) => None,
    })
    .flatten()
    .map(|variant| variant.type_name().to_string())
    .collect()
}

/// The model identifier of an element name.
fn ident(name: &str) -> Ident {
    Ident::new(&name.to_snake_case(), IdentSpan::call_site())
}

/// Locate a field in the model by its [`Chain`].
fn find_field(model: &Model, chain: &Chain) -> Option<FieldIndex> {
    let (peripheral, register, field) = chain;

    let peripheral = model.try_get_peripheral(ident(peripheral).into())?;
    let register = peripheral.try_get_register(&ident(register))?;
    let field = register.try_get_field(&ident(field))?;

    Some(*field.index())
}

/// Locate a variant in the model by its field's [`Chain`] and its name.
fn find_variant(model: &Model, chain: &Chain, variant: &str) -> Option<(FieldIndex, VariantIndex)> {
    let (peripheral, register, field) = chain;

    let peripheral = model.try_get_peripheral(ident(peripheral).into())?;
    let register = peripheral.try_get_register(&ident(register))?;
    let field = register.try_get_field(&ident(field))?;

    let variant = ident(variant);

    let index = [
        field.access.access().get_read(),
        field.access.access().get_write(),
    ]
    .into_iter()
    .flatten()
    .find_map(|numericity| match numericity {
        Numericity::Enumerated(enumerated) => enumerated.variants.get(&variant).copied(),
        Numericity::Numeric(..) => None,
    })?;

    Some((*field.index(), index))
}
