//! Every semantic diagnostic of model evaluation, in one place.
//!
//! Each diagnostic is a constructor here — its message, its labels, and its
//! notes — so the full catalog can be read (and changed) top to bottom
//! without digging through elaboration. The constructors are grouped the way
//! [`Kind`] groups its codes: sources, structure, templates, arrays,
//! schemas, entitlements.
//!
//! House style:
//! - the *message* states the judgement, naming every party with its
//!   relevant property in quotes: `field `foo` with modality `read` cannot
//!   assume schema `bar``,
//! - the *label* describes what the underlined span is — never a repeat of
//!   the message,
//! - secondary labels (blue) point at the definitions of the other parties,
//! - *notes* teach the rule that was broken, and name near-misses
//!   (`a similarly named variant `On` exists in field `c``).

use syntax::ast::Span;

pub use ::model::diagnostic::{Kind, Rank};

use super::elaborate::Locations;

/// A coded diagnostic anchored to the source text — emitted during
/// elaboration, or converted from the model's own judgements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub rank: Rank,
    pub kind: Kind,
    pub span: Span,
    /// The headline: the judgement itself.
    pub message: String,
    /// The primary label: what the underlined span *is*.
    pub label: String,
    /// Secondary labels: the definitions of the other involved parties.
    pub labels: Vec<(Span, String)>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    fn error(kind: Kind, span: Span, message: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            rank: Rank::Error,
            kind,
            span,
            message: message.into(),
            label: label.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn warning(
        kind: Kind,
        span: Span,
        message: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            rank: Rank::Warning,
            kind,
            span,
            message: message.into(),
            label: label.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push((span, message.into()));
        self
    }

    fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    fn note_if(self, note: Option<String>) -> Self {
        match note {
            Some(note) => self.note(note),
            None => self,
        }
    }
}

// sources
impl Diagnostic {
    /// the import `st.gpio` resolves to `st/gpio.phm`, which cannot be read
    pub fn unreadable_import(
        span: Span,
        written: &str,
        target: &str,
        cause: String,
        device: bool,
    ) -> Self {
        let rule = match device {
            true => "a device's imports resolve within the model's `components`",
            false => "import paths are relative to the importing file",
        };

        Self::error(
            Kind::UnreadableImport,
            span,
            format!("the import `{written}` resolves to `{target}`, which cannot be read"),
            "imported here",
        )
        .note(cause)
        .note(rule)
    }

    /// an import named `gpio` already exists in this file
    pub fn conflicting_import(span: Span, alias: &str, first: Option<Span>) -> Self {
        let mut diagnostic = Self::error(
            Kind::ConflictingImport,
            span,
            format!("an import named `{alias}` already exists in this file"),
            "the conflicting import",
        )
        .note("imports are referenced by their final path segment; two imports may not share one");

        if let Some(first) = first {
            diagnostic = diagnostic.label(first, format!("`{alias}` is first imported here"));
        }

        diagnostic
    }
}

// structure
impl Diagnostic {
    /// no device is defined
    pub fn no_device(span: Span) -> Self {
        Self::error(
            Kind::NoDevice,
            span,
            "no device is defined",
            "this file defines no device",
        )
        .note("the entry file of a model description must define exactly one `device`")
    }

    /// more than one device is defined
    pub fn many_devices(span: Span, first: Span) -> Self {
        Self::error(
            Kind::ManyDevices,
            span,
            "more than one device is defined",
            "a second device",
        )
        .label(first, "the first device is defined here")
        .note("the entry file of a model description must define exactly one `device`")
    }

    /// this device is not elaborated
    pub fn device_not_elaborated(span: Span) -> Self {
        Self::warning(
            Kind::DeviceNotElaborated,
            span,
            "this device is not elaborated",
            "defined outside the entry file",
        )
        .note("only the entry file's device is elaborated — named devices remain usable as templates")
    }

    /// the device has no body
    pub fn empty_device(span: Span) -> Self {
        Self::warning(
            Kind::EmptyDevice,
            span,
            "the device has no body",
            "nothing to elaborate",
        )
    }

    /// this {what} has no name
    pub fn unnamed(span: Span, what: &str) -> Self {
        Self::error(
            Kind::Unnamed,
            span,
            format!("this {what} has no name"),
            format!("anonymous {what}"),
        )
    }

    /// this {what} does not specify a position
    pub fn expected_position(span: Span, what: &str) -> Self {
        Self::error(
            Kind::ExpectedPosition,
            span,
            format!("this {what} does not specify a position"),
            "missing `@`",
        )
        .note("neither the definition nor its template provide `@`")
    }

    /// this {what} does not specify an access modality
    pub fn expected_modality(span: Span, what: &str) -> Self {
        Self::error(
            Kind::ExpectedModality,
            span,
            format!("this {what} does not specify an access modality"),
            "missing an access modality",
        )
        .note(MODALITIES)
    }

    /// this access modality combination is invalid
    pub fn invalid_modality(span: Span) -> Self {
        Self::error(
            Kind::InvalidModality,
            span,
            "this access modality combination is invalid",
            "not a modality",
        )
        .note(MODALITIES)
    }

    /// field `{name}` does not fit within a register
    pub fn does_not_fit(span: Span, name: &str, offset: u32, width: u32) -> Self {
        let end = offset + width.max(1) - 1;

        Self::error(
            Kind::DoesNotFit,
            span,
            format!("field `{name}` does not fit within a register"),
            format!("occupies bits {offset}..={end}"),
        )
        .note("registers are 32 bits: 0..=31")
    }

    /// a register reset must be numeric
    pub fn non_numeric_register_reset(span: Span) -> Self {
        Self::error(
            Kind::NonNumericRegisterReset,
            span,
            "a register reset must be numeric",
            "a variant name",
        )
        .note("a variant name identifies a field-level reset — a register spans many fields")
    }

    /// field `{field}` has no variant named `{variant}`
    pub fn unknown_variant(
        span: Span,
        field: &str,
        variant: &str,
        suggestion: Option<String>,
    ) -> Self {
        Self::error(
            Kind::UnknownVariant,
            span,
            format!("field `{field}` has no variant named `{variant}`"),
            "no variant by this name",
        )
        .note_if(suggestion.map(|suggestion| {
            format!("a similarly named variant `{suggestion}` exists in field `{field}`")
        }))
    }
}

// templates
impl Diagnostic {
    /// no {what} template is named `{name}` /
    /// `{alias}` has no {what} template named `{name}`
    pub fn unknown_template(
        span: Span,
        what: &str,
        name: &str,
        alias: Option<&str>,
        suggestion: Option<String>,
    ) -> Self {
        let message = match alias {
            Some(alias) => format!("`{alias}` has no {what} template named `{name}`"),
            None => format!("no {what} template is named `{name}`"),
        };

        let suggestion = suggestion.map(|suggestion| match alias {
            Some(alias) => {
                format!("`{alias}` has a similarly named {what} template `{suggestion}`")
            }
            None => format!("a similarly named {what} template `{suggestion}` exists"),
        });

        Self::error(Kind::UnknownTemplate, span, message, "no such template")
            .note("templates are top-level definitions")
            .note_if(suggestion)
    }

    /// no import is named `{alias}`
    pub fn unknown_import(span: Span, alias: &str, suggestion: Option<String>) -> Self {
        Self::error(
            Kind::UnknownImport,
            span,
            format!("no import is named `{alias}`"),
            "no such import",
        )
        .note("imports are referenced by their final path segment")
        .note_if(
            suggestion
                .map(|suggestion| format!("a similarly named import `{suggestion}` exists")),
        )
    }

    /// template derivation is too deep — is there a cycle?
    pub fn template_depth_exceeded(span: Span) -> Self {
        Self::error(
            Kind::TemplateDepthExceeded,
            span,
            "template derivation is too deep — is there a cycle?",
            "derivation never bottoms out",
        )
    }

    /// template references name at most `import.definition`
    pub fn invalid_template_reference(span: Span) -> Self {
        Self::error(
            Kind::InvalidTemplateReference,
            span,
            "template references name at most `import.definition`",
            "too many segments",
        )
    }
}

// arrays
impl Diagnostic {
    /// this range is empty
    pub fn empty_range(span: Span) -> Self {
        Self::error(Kind::EmptyRange, span, "this range is empty", "empty")
    }

    /// position lists belong to `array` definitions
    pub fn positions_without_array(span: Span) -> Self {
        Self::error(
            Kind::ExpectedArray,
            span,
            "position lists belong to `array` definitions",
            "a position list",
        )
    }

    /// element designators require the `array` keyword
    pub fn designators_without_array(span: Span) -> Self {
        Self::error(
            Kind::ExpectedArray,
            span,
            "element designators require the `array` keyword",
            "element designators",
        )
    }

    /// array definitions require element designators
    pub fn expected_elements(span: Span) -> Self {
        Self::error(
            Kind::ExpectedElements,
            span,
            "array definitions require element designators",
            "no element designators",
        )
        .note("designators name each element: `[a, b, c]` or `[0..=3]`")
    }

    /// an array requires a position list
    pub fn expected_positions(span: Span) -> Self {
        Self::error(
            Kind::ExpectedPositions,
            span,
            "an array requires a position list",
            "not a list",
        )
    }

    /// a variant array requires a value list
    pub fn expected_value_list(span: Span) -> Self {
        Self::error(
            Kind::ExpectedPositions,
            span,
            "a variant array requires a value list",
            "not a list",
        )
    }

    /// this variant does not specify a value (`~`)
    pub fn expected_value(span: Span) -> Self {
        Self::error(
            Kind::ExpectedValue,
            span,
            "this variant does not specify a value (`~`)",
            "missing `~`",
        )
    }

    /// a {what} occupies a single position, not a bit domain
    pub fn scalar_position(span: Span, what: &str) -> Self {
        Self::error(
            Kind::ExpectedScalar,
            span,
            format!("a {what} occupies a single position, not a bit domain"),
            "a bit domain",
        )
    }

    /// a single variant occupies a single value
    pub fn scalar_value(span: Span) -> Self {
        Self::error(
            Kind::ExpectedScalar,
            span,
            "a single variant occupies a single value",
            "a value list",
        )
    }

    /// this position is a single value, not a bit domain
    pub fn scalar_entry(span: Span) -> Self {
        Self::error(
            Kind::ExpectedScalar,
            span,
            "this position is a single value, not a bit domain",
            "a bit domain",
        )
    }

    /// a bare `...` has no default step at this position
    pub fn no_default_stride(span: Span, explanation: &str) -> Self {
        Self::error(Kind::UnsupportedRest, span, explanation, "bare `...`")
    }

    /// `...` in the middle of a list is not yet supported
    pub fn unsupported_rest(span: Span) -> Self {
        Self::error(
            Kind::UnsupportedRest,
            span,
            "`...` in the middle of a list is not yet supported",
            "mid-list `...`",
        )
    }

    /// `...` requires at least one explicit element before it
    pub fn dangling_rest(span: Span) -> Self {
        Self::error(
            Kind::DanglingRest,
            span,
            "`...` requires at least one explicit element before it",
            "nothing precedes it",
        )
    }

    /// `...` produced the out-of-range position {value}
    pub fn out_of_range(span: Span, value: i64) -> Self {
        Self::error(
            Kind::OutOfRange,
            span,
            format!("`...` produced the out-of-range position {value}"),
            format!("continues to {value}"),
        )
    }

    /// expected {expected} {what}, found {found}
    pub fn count_mismatch(span: Span, what: &str, expected: usize, found: usize) -> Self {
        Self::error(
            Kind::CountMismatch,
            span,
            format!("expected {expected} {what}, found {found}"),
            format!("{found} of {expected}"),
        )
    }
}

// schemas
impl Diagnostic {
    /// no schema named `{name}` is placed within reach of field `{field}`
    pub fn unplaced_schema(
        span: Span,
        name: &str,
        field: &str,
        suggestion: Option<String>,
        template_exists: bool,
    ) -> Self {
        let mut diagnostic = Self::error(
            Kind::UnplacedSchema,
            span,
            format!("no schema named `{name}` is placed within reach of field `{field}`"),
            "not placed within reach",
        )
        .note(
            "a schema must be placed — given a location in the device — in one \
             of the field's enclosing scopes to be assumed",
        );

        diagnostic = diagnostic.note_if(suggestion.map(|suggestion| {
            format!("a similarly named schema `{suggestion}` is placed within reach")
        }));

        if template_exists {
            diagnostic = diagnostic.note(format!(
                "a schema template named `{name}` exists — placing it \
                 (`schema #{name}`) in an enclosing scope makes it assumable",
            ));
        }

        diagnostic
    }

    /// an assumed schema is referenced by its placed name
    pub fn assumed_by_path(span: Span) -> Self {
        Self::error(
            Kind::UnplacedSchema,
            span,
            "an assumed schema is referenced by its placed name",
            "a path, not a placed name",
        )
        .note(
            "a placement is visible within its enclosing scope by its bare \
             name — assumption paths never cross files",
        )
    }

    /// `{side}` variant `{variant}` of schema `{schema}` cannot occupy field
    /// `{field}` with modality `{modality}`
    ///
    /// `relation` is how the field references the schema: `assumed` or
    /// `extended`.
    #[expect(clippy::too_many_arguments)]
    pub fn incompatible_schema_variant(
        kind: Kind,
        reference: Span,
        relation: &str,
        side: &str,
        variant: &str,
        schema: &str,
        field: &str,
        modality: String,
        side_span: Span,
        modality_span: Span,
    ) -> Self {
        Self::error(
            kind,
            reference,
            format!(
                "`{side}` variant `{variant}` of schema `{schema}` cannot \
                 occupy field `{field}` with modality `{modality}`"
            ),
            format!("{relation} here"),
        )
        .label(side_span, format!("`{variant}` is declared `{side}` here"))
        .label(
            modality_span,
            format!("field `{field}` is declared `{modality}` here"),
        )
        .note(side_rule(kind, side))
    }

    /// `{side}` variant `{variant}` cannot occupy field `{field}` with
    /// modality `{modality}`
    pub fn incompatible_variant(
        kind: Kind,
        side_span: Span,
        side: &str,
        variant: &str,
        field: &str,
        modality: String,
        modality_span: Span,
    ) -> Self {
        Self::error(
            kind,
            side_span,
            format!(
                "`{side}` variant `{variant}` cannot occupy field \
                 `{field}` with modality `{modality}`"
            ),
            format!("declared `{side}`"),
        )
        .label(
            modality_span,
            format!("field `{field}` is declared `{modality}` here"),
        )
        .note(side_rule(kind, side))
    }

    /// assuming field `{field}` cannot declare its own variants
    pub fn assumed_variants(span: Span, field: &str, reference: Span) -> Self {
        Self::error(
            Kind::AssumedVariants,
            span,
            format!("assuming field `{field}` cannot declare its own variants"),
            "inherent variants",
        )
        .label(reference, "the assumed schema")
        .note(
            "an assuming field exactly reflects its schema — nothing may be \
             added to it; to add variants, extend instead",
        )
    }

    /// field `{field}` cannot both assume and extend
    pub fn assumes_and_extends(span: Span, field: &str, assumes: Span) -> Self {
        Self::error(
            Kind::AssumesAndExtends,
            span,
            format!("field `{field}` cannot both assume and extend"),
            "extended here",
        )
        .label(assumes, "assumed here")
        .note(
            "an assuming field exactly reflects its schema — nothing may be \
             added to it; to add variants, extend instead",
        )
    }

    /// variant `{variant}` of schema `{schema}` cannot have requirements
    pub fn templated_entitlements(span: Span, variant: &str, schema: &str) -> Self {
        Self::error(
            Kind::TemplatedEntitlements,
            span,
            format!("variant `{variant}` of schema `{schema}` cannot have requirements"),
            "statewise requirements",
        )
        .note("entitlements are intrinsically application-time — they cannot be templated")
    }

    /// a schema named `{name}` is already placed in this scope
    pub fn duplicate_placement(span: Span, name: &str, first: Span) -> Self {
        Self::error(
            Kind::Exists,
            span,
            format!("a schema named `{name}` is already placed in this scope"),
            "placed again here",
        )
        .label(first, format!("`{name}` is first placed here"))
        .note("a schema's identity is its placement and its name")
    }
}

// entitlements
impl Diagnostic {
    /// a pattern names field `{field}` twice
    pub fn repeated_field(span: Span, field: &str, first: Span) -> Self {
        Self::error(
            Kind::RepeatedField,
            span,
            format!("a pattern names field `{field}` twice"),
            "the same field again",
        )
        .label(first, format!("`{field}` is first entitled here"))
        .note(format!(
            "`&` conjoins distinct fields — a field cannot inhabit two \
             variants at once; for membership in a set of variants, write \
             `{field}.{{A, B}}`",
        ))
    }

    /// an array correspondence outside any array
    pub fn correspondence_outside_array(span: Span) -> Self {
        Self::error(
            Kind::InvalidCorrespondence,
            span,
            "an array correspondence outside any array",
            "no array encloses this requirement",
        )
        .note(
            "`name[a..b]` names one target per element of the enclosing \
             array — a scalar writer has no elements to correspond",
        )
    }

    /// an array correspondence within nested arrays
    pub fn nested_correspondence(span: Span, enclosing: Vec<Span>) -> Self {
        let mut diagnostic = Self::error(
            Kind::InvalidCorrespondence,
            span,
            "an array correspondence within nested arrays",
            "which array this binds is ambiguous",
        );

        for array in enclosing {
            diagnostic = diagnostic.label(array, "an enclosing array");
        }

        diagnostic.note(
            "a correspondence binds the writer's one enclosing array — \
             correspondence within nested arrays is not yet supported",
        )
    }

    /// a path carries several array correspondences
    pub fn several_correspondences(span: Span, first: Span) -> Self {
        Self::error(
            Kind::InvalidCorrespondence,
            span,
            "a path carries several array correspondences",
            "a second correspondence",
        )
        .label(first, "the first correspondence is here")
        .note(
            "one bracketed segment binds the enclosing array — several in \
             one path are not yet supported",
        )
    }

    /// the correspondence names {targets} targets across {elements} elements
    pub fn correspondence_length(
        span: Span,
        targets: usize,
        elements: usize,
        array: Span,
    ) -> Self {
        let plural = |count: usize| if count == 1 { "" } else { "s" };

        Self::error(
            Kind::InvalidCorrespondence,
            span,
            format!(
                "the correspondence names {targets} target{} across {elements} element{}",
                plural(targets),
                plural(elements),
            ),
            format!("{targets} target{}", plural(targets)),
        )
        .label(
            array,
            format!("the enclosing array has {elements} element{}", plural(elements)),
        )
        .note(
            "a correspondence zips by ordinal: it must name exactly one \
             target per element of the enclosing array",
        )
    }

    /// read-only field `{field}` cannot have write entitlements
    pub fn write_requires_readonly(span: Span, field: &str) -> Self {
        Self::error(
            Kind::EntitlementModality,
            span,
            format!("read-only field `{field}` cannot have write entitlements"),
            "write entitlements",
        )
        .note("write entitlements guard writes — a read-only field admits none")
    }

    /// field `{field}` with modality `{modality}` cannot have hardware write
    /// entitlements
    pub fn hardware_write_requires(span: Span, field: &str, modality: String) -> Self {
        Self::error(
            Kind::EntitlementModality,
            span,
            format!(
                "field `{field}` with modality `{modality}` cannot have \
                 hardware write entitlements"
            ),
            "hardware write entitlements",
        )
        .note("hardware only writes to `volatile store` fields")
    }

    /// `{segment}` does not exist within `{prefix}` / at the device root
    pub fn unknown_path_segment(
        span: Span,
        segment: &str,
        prefix: Option<String>,
        suggestion: Option<String>,
    ) -> Self {
        let (message, similar) = match &prefix {
            Some(prefix) => (
                format!("`{segment}` does not exist within `{prefix}`"),
                suggestion.map(|suggestion| {
                    format!("a similarly named `{suggestion}` exists within `{prefix}`")
                }),
            ),
            None => (
                format!("`{segment}` does not exist at the device root"),
                suggestion.map(|suggestion| {
                    format!("a similarly named `{suggestion}` exists at the device root")
                }),
            ),
        };

        Self::error(Kind::UnknownPath, span, message, "does not exist")
            .note(PATH_ANATOMY)
            .note_if(similar)
    }

    /// this entitlement path is incomplete
    pub fn incomplete_path(span: Span, continuations: Vec<String>) -> Self {
        Self::error(
            Kind::UnknownPath,
            span,
            "this entitlement path is incomplete",
            "stops short of a variant",
        )
        .note(PATH_ANATOMY)
        .note(format!(
            "the path continues with one of: {}",
            continuations
                .iter()
                .map(|segment| format!("`{segment}`"))
                .collect::<Vec<_>>()
                .join(", "),
        ))
    }

    /// this entitlement could not be located in the model
    pub fn unlocated_entitlement(span: Span) -> Self {
        Self::error(
            Kind::UnlocatedEntitlement,
            span,
            "this entitlement could not be located in the model",
            "unlocatable",
        )
    }

    /// statewise requirements form a cycle
    pub fn entitlement_cycle(span: Span, cycle: String) -> Self {
        Self::error(
            Kind::EntitlementCycle,
            span,
            "statewise requirements form a cycle",
            "closes the cycle",
        )
        .note(format!("the cycle: {cycle}"))
    }
}

// not yet expressible in the model
impl Diagnostic {
    /// {what} is not yet supported by the model
    pub fn unsupported(span: Span, what: &str) -> Self {
        Self::warning(
            Kind::Unsupported,
            span,
            format!("{what} is not yet supported by the model"),
            "not yet supported",
        )
    }
}

// the model's own judgements
impl Diagnostic {
    /// Anchor a model diagnostic to the source text. The model judges itself
    /// by context path; elaboration remembers where each element came from,
    /// so its judgements report like any other — same renderer, same style,
    /// a span.
    ///
    /// Physical judgements — overlaps, alignment, domains — anchor at the
    /// party's `@` position; the rest at its head. Related parties earn
    /// their own labels.
    pub fn model(diagnostic: &::model::diagnostic::Diagnostic, locations: &Locations) -> Self {
        let physical = matches!(
            diagnostic.kind(),
            Kind::Overlap | Kind::AddressUnaligned | Kind::ExceedsDomain,
        );

        let place = |path: &[String]| {
            locations.get(path).map(|location| {
                if physical {
                    location.domain.unwrap_or(location.head)
                } else {
                    location.head
                }
            })
        };

        let location = place(diagnostic.context().path());

        let mut semantic = Self {
            rank: diagnostic.rank().clone(),
            kind: *diagnostic.kind(),
            span: location.unwrap_or(Span {
                start: 0,
                end: 0,
                context: 0,
            }),
            message: diagnostic.message().clone(),
            label: match diagnostic.kind() {
                Kind::Overlap => "this domain".to_string(),
                _ if diagnostic.context().path().is_empty() => "here".to_string(),
                _ => format!("in {}", diagnostic.context()),
            },
            labels: diagnostic
                .get_related()
                .iter()
                .filter_map(|related| {
                    let span = place(related.path())?;

                    Some((
                        span,
                        match diagnostic.kind() {
                            Kind::Overlap => "overlapping domain here".to_string(),
                            _ => format!("in {related}"),
                        },
                    ))
                })
                .collect(),
            notes: diagnostic.get_notes().to_vec(),
        };

        // a judgement of an element the table doesn't know still names it
        if location.is_none() && !diagnostic.context().path().is_empty() {
            semantic = semantic.note(format!("in {}", diagnostic.context()));
        }

        semantic
    }
}

const MODALITIES: &str =
    "the access modalities: `read`, `write`, `read write`, `store`, `volatile store`";

const PATH_ANATOMY: &str =
    "entitlement paths are anchored at the device root and name every module \
     level — groups included — ending at a variant";

/// The rule a side violation breaks, as a note.
fn side_rule(kind: Kind, side: &str) -> String {
    match kind {
        Kind::InvalidSide => "`store` and `volatile store` fields have a single numericity — \
                              every variant occupies it wholly, so sides do not apply"
            .to_string(),
        _ => {
            let (numericity, able, admits) = match side {
                "read" => ("read", "readable", "`read` or `read write`"),
                _ => ("write", "writable", "`write` or `read write`"),
            };

            format!(
                "a `{side}` variant occupies the {numericity} numericity — \
                 its field must be {able}: {admits}"
            )
        }
    }
}
