//! Validation of [declarations](crate::ast::Declaration) after parsing, according to the qualifiers, properties, and
//! children each kind of declaration permits.

use crate::{
    ast::{
        AccessModality, Child, Declaration, DeclarationKind, File, Item, PropertyKind, Qualifier,
        RequirementQualifiers,
    },
    diagnostic::{self, Diagnostic},
    util::{Parsed, Spanned},
};

/// Validate the qualifiers, properties, and children of the declarations in the provided file.
pub(crate) fn validate(file: &File) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for item in &file.items {
        if let Parsed::Node(Item::Declaration(declaration)) = &**item {
            validate_declaration(declaration, None, None, &mut diagnostics);
        }
    }

    diagnostics
}

/// Validate the provided declaration, contained by the provided parent within the provided ancestor, and the
/// declarations it contains, adding the diagnostics to be emitted to the provided list.
///
/// *Note: The ancestor is the nearest enclosing declaration other than a group, as a group inherits the permissions of
/// the declaration enclosing the group.*
fn validate_declaration(
    declaration: &Declaration,
    parent: Option<&Declaration>,
    ancestor: Option<DeclarationKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = *declaration.kind;

    if !permitted_children(ancestor).contains(&kind) {
        diagnostics.push(diagnostic::misplaced_declaration(
            declaration,
            parent,
            permitted_children(ancestor).iter().copied(),
        ));
    }

    for qualifier in &declaration.qualifiers {
        if let Parsed::Node(inner) = **qualifier
            && !permitted_qualifiers(kind).contains(&inner)
        {
            diagnostics.push(diagnostic::unpermitted_qualifier(
                declaration,
                Spanned {
                    inner,
                    span: qualifier.span,
                },
                permitted_qualifiers(kind).iter().copied(),
            ));
        }
    }

    for property in &declaration.properties {
        if let Parsed::Node(inner) = &**property
            && !permitted_properties(kind, ancestor).contains(&inner.kind())
        {
            diagnostics.push(diagnostic::unpermitted_property(
                declaration,
                Spanned {
                    inner: inner.kind(),
                    span: property.span,
                },
                ancestor,
                permitted_properties(kind, ancestor).iter().copied(),
            ));
        }
    }

    let ancestor = match kind {
        DeclarationKind::Group => ancestor,
        kind => Some(kind),
    };

    for child in &declaration.children {
        match &**child {
            Parsed::Node(Child::Declaration(child)) => {
                validate_declaration(child, Some(declaration), ancestor, diagnostics)
            }
            Parsed::Node(Child::Interrupts(..)) if kind != DeclarationKind::Device => diagnostics
                .push(diagnostic::misplaced_interrupts(
                    child.span,
                    declaration,
                    permitted_children(ancestor).iter().copied(),
                )),
            _ => {}
        }
    }
}

/// The kinds of declaration the provided ancestor may contain, or a module may contain when there is no ancestor.
fn permitted_children(ancestor: Option<DeclarationKind>) -> &'static [DeclarationKind] {
    use DeclarationKind::*;

    match ancestor {
        None => &[Device, Peripheral, Register, Field, Schema],
        Some(Device) => &[Peripheral, Group],
        Some(Peripheral) => &[Register, Group],
        Some(Register) => &[Field, Group],
        Some(Field | Schema) => &[Variant],
        Some(Variant | Group) => &[],
    }
}

/// The qualifiers permitted on declarations of the provided kind.
fn permitted_qualifiers(kind: DeclarationKind) -> &'static [Qualifier] {
    use AccessModality::*;

    match kind {
        DeclarationKind::Peripheral | DeclarationKind::Register => &[Qualifier::Leaky],
        DeclarationKind::Field => &[
            Qualifier::Leaky,
            Qualifier::AccessModality(Read),
            Qualifier::AccessModality(Write),
            Qualifier::AccessModality(ReadWrite),
            Qualifier::AccessModality(Store),
            Qualifier::AccessModality(VolatileStore),
        ],
        DeclarationKind::Variant => &[
            Qualifier::Inert,
            Qualifier::AccessModality(Read),
            Qualifier::AccessModality(Write),
        ],
        DeclarationKind::Device | DeclarationKind::Schema | DeclarationKind::Group => &[],
    }
}

/// The kinds of property permitted on declarations of the provided kind within the provided ancestor.
fn permitted_properties(
    kind: DeclarationKind,
    ancestor: Option<DeclarationKind>,
) -> &'static [PropertyKind] {
    use PropertyKind::*;
    use RequirementQualifiers::*;

    match kind {
        DeclarationKind::Device => &[WordSize],
        DeclarationKind::Peripheral => &[Placement, Access, Refines, Requires(None)],
        DeclarationKind::Register => &[Placement, Reset, Access, Refines],
        DeclarationKind::Field => &[
            Placement,
            Reset,
            Assumes,
            Refines,
            Requires(None),
            Requires(Some(Write)),
            Requires(Some(HardwareWrite)),
        ],
        DeclarationKind::Schema => &[Refines],
        DeclarationKind::Variant if ancestor == Some(DeclarationKind::Field) => {
            &[Discriminant, Requires(None)]
        }
        DeclarationKind::Variant => &[Discriminant],
        DeclarationKind::Group => &[Placement],
    }
}

#[cfg(test)]
mod tests {
    use ::diagnostic::Kind as _;

    use crate::{diagnostic::Kind, parse};

    #[test]
    fn qualifiers() {
        expect(
            "leaky device foo wordsize 4 { leaky group bar { leaky peripheral baz @ 0 } } \
             store peripheral qux @ 0 \
             schema corge { read write variant A ~ 0 inert write variant B ~ 1 } \
             leaky volatile store field grault @ 0 { inert variant C ~ 0 }",
            &[
                (
                    Kind::UnpermittedQualifier,
                    "unexpected qualifier `leaky` on device `foo`",
                ),
                (
                    Kind::UnpermittedQualifier,
                    "unexpected qualifier `leaky` on group `bar`",
                ),
                (
                    Kind::UnpermittedQualifier,
                    "unexpected qualifier `store` on peripheral `qux`",
                ),
                (
                    Kind::UnpermittedQualifier,
                    "unexpected qualifier `read write` on variant `A`",
                ),
            ],
            "expected a diagnostic to be emitted for each qualifier the kind of the qualified declaration does not \
             permit, and none for the qualifiers of `baz`, `B`, `grault`, and `C`",
        );
    }

    #[test]
    fn properties() {
        expect(
            "device foo wordsize 4 @ 0 \
             peripheral bar @ 0 write requires baz.qux.A { register corge @ 0 wordsize 4 requires baz.qux.A } \
             schema grault { variant B ~ 0 requires baz.qux.A } \
             store field garply @ 0 hardware write requires baz.qux.A { variant C ~ 0 requires baz.qux.A }",
            &[
                (
                    Kind::UnpermittedProperty,
                    "unexpected property `@` on device `foo`",
                ),
                (
                    Kind::UnpermittedProperty,
                    "unexpected property `write requires` on peripheral `bar`",
                ),
                (
                    Kind::UnpermittedProperty,
                    "unexpected property `wordsize` on register `corge`",
                ),
                (
                    Kind::UnpermittedProperty,
                    "unexpected property `requires` on register `corge`",
                ),
                (
                    Kind::UnpermittedProperty,
                    "unexpected property `requires` on variant `B`",
                ),
            ],
            "expected a diagnostic to be emitted for each property the kind of the declaration does not permit, and \
             none for the properties of `garply` and `C`",
        );
    }

    #[test]
    fn placement() {
        expect(
            "device foo wordsize 4 { \
                 register bar @ 0 group baz { field qux @ 0 peripheral corge @ 0 } interrupts { A @ 0 } \
             } \
             peripheral grault @ 0 { interrupts { B @ 0 } } \
             variant C ~ 0",
            &[
                (Kind::Misplaced, "unexpected register `bar` in device `foo`"),
                (Kind::Misplaced, "unexpected field `qux` in group `baz`"),
                (
                    Kind::Misplaced,
                    "unexpected interrupt table in peripheral `grault`",
                ),
                (Kind::Misplaced, "unexpected variant `C`"),
            ],
            "expected a diagnostic to be emitted for each declaration or interrupt table the enclosing module or \
             declaration cannot contain, and none for `baz`, `corge`, or the interrupt table of `foo`",
        );
    }

    #[test]
    fn notes() {
        for (s, expected) in [
            (
                "device foo wordsize 4 { register bar @ 0 }",
                vec!["children of devices may be peripherals, groups, or interrupt tables"],
            ),
            (
                "device foo wordsize 4 { group bar { field baz @ 0 } }",
                vec!["children of groups may be peripherals or groups"],
            ),
            (
                "variant A ~ 0",
                vec!["module items may be devices, peripherals, registers, fields, or schemas"],
            ),
            (
                "leaky device foo wordsize 4",
                vec!["devices have no qualifiers"],
            ),
            ("store register foo @ 0", vec!["expected `leaky`"]),
            (
                "schema foo { variant A ~ 0 requires bar.baz.B }",
                vec![
                    "expected `~`",
                    "statewise entitlements may be imposed only by variants of fields",
                ],
            ),
            (
                "register foo @ 0 requires bar.baz.A",
                vec!["expected one of `@`, `reset`, `access`, or `refines`"],
            ),
        ] {
            let (.., diagnostics) = parse(s, 0);

            similar_asserts::assert_eq!(
                expected: expected,
                found: diagnostics
                    .iter()
                    .flat_map(|diagnostic| &diagnostic.notes)
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                "expected the diagnostics emitted by parsing `{s}` to carry the listed notes",
            );
        }
    }

    /// Assert that parsing the provided source text emits diagnostics of the provided kinds and messages, in order,
    /// failing with the provided assertion message.
    fn expect(s: &str, expected: &[(Kind, &str)], message: &str) {
        let (.., diagnostics) = parse(s, 0);

        similar_asserts::assert_eq!(
            expected: expected
                .iter()
                .map(|&(kind, message)| (kind.code(), message))
                .collect::<Vec<_>>(),
            found: diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.id.code, diagnostic.message.as_str()))
                .collect::<Vec<_>>(),
            "{message}",
        );
    }
}
