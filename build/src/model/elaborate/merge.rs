//! Template merging.
//!
//! An invocation may override any property of its template by simply
//! specifying it, and may only *append* children. Merging is same-type by
//! construction — a register template can only produce a register — so no
//! kind judgements happen here.

use syntax::ast::{Device, Field, Head, Peripheral, Register, Schema, Spanned, Variant};

pub(super) fn device<'src>(template: Device<'src>, invocation: &Device<'src>) -> Device<'src> {
    Device {
        docs: docs(template.docs, &invocation.docs),
        head: head(template.head, &invocation.head),
        body: bodies(template.body, invocation.body.clone()),
    }
}

pub(super) fn peripheral<'src>(
    template: Peripheral<'src>,
    invocation: &Peripheral<'src>,
) -> Peripheral<'src> {
    Peripheral {
        docs: docs(template.docs, &invocation.docs),
        leaky: invocation.leaky.or(template.leaky),
        array: invocation.array.or(template.array),
        head: head(template.head, &invocation.head),
        domain: invocation.domain.clone().or(template.domain),
        requires: invocation.requires.clone().or(template.requires),
        body: bodies(template.body, invocation.body.clone()),
    }
}

pub(super) fn register<'src>(
    template: Register<'src>,
    invocation: &Register<'src>,
) -> Register<'src> {
    Register {
        docs: docs(template.docs, &invocation.docs),
        leaky: invocation.leaky.or(template.leaky),
        array: invocation.array.or(template.array),
        head: head(template.head, &invocation.head),
        domain: invocation.domain.clone().or(template.domain),
        reset: invocation.reset.or(template.reset),
        body: bodies(template.body, invocation.body.clone()),
    }
}

pub(super) fn field<'src>(template: Field<'src>, invocation: &Field<'src>) -> Field<'src> {
    Field {
        docs: docs(template.docs, &invocation.docs),
        leaky: invocation.leaky.or(template.leaky),
        access: if invocation.access.is_empty() {
            template.access
        } else {
            invocation.access.clone()
        },
        array: invocation.array.or(template.array),
        head: head(template.head, &invocation.head),
        domain: invocation.domain.clone().or(template.domain),
        assumes: invocation.assumes.clone().or(template.assumes),
        // extends accumulate — the template's, then the invocation's
        extends: template
            .extends
            .into_iter()
            .chain(invocation.extends.iter().cloned())
            .collect(),
        reset: invocation.reset.or(template.reset),
        requires: syntax::ast::FieldRequires {
            plain: invocation.requires.plain.clone().or(template.requires.plain),
            write: invocation.requires.write.clone().or(template.requires.write),
            hardware_write: invocation
                .requires
                .hardware_write
                .clone()
                .or(template.requires.hardware_write),
        },
        body: bodies(template.body, invocation.body.clone()),
    }
}

pub(super) fn schema<'src>(template: Schema<'src>, invocation: &Schema<'src>) -> Schema<'src> {
    Schema {
        docs: docs(template.docs, &invocation.docs),
        leaky: invocation.leaky.or(template.leaky),
        head: head(template.head, &invocation.head),
        body: bodies(template.body, invocation.body.clone()),
    }
}

pub(super) fn variant<'src>(template: Variant<'src>, invocation: &Variant<'src>) -> Variant<'src> {
    Variant {
        docs: docs(template.docs, &invocation.docs),
        leaky: invocation.leaky.or(template.leaky),
        inert: invocation.inert.or(template.inert),
        side: invocation.side.or(template.side),
        array: invocation.array.or(template.array),
        head: head(template.head, &invocation.head),
        value: invocation.value.clone().or(template.value),
        requires: invocation.requires.clone().or(template.requires),
    }
}

/// The instance keeps its own name (and designators) if given, and takes its
/// template's otherwise. The merged head is no longer a template reference.
fn head<'src>(template: Head<'src>, invocation: &Head<'src>) -> Head<'src> {
    Head {
        template: None,
        name: invocation.name.or(template.name),
        indices: invocation.indices.clone().or(template.indices),
    }
}

/// Template docs come first; the invocation's extend them.
fn docs<'src>(
    template: Vec<Spanned<&'src str>>,
    invocation: &[Spanned<&'src str>],
) -> Vec<Spanned<&'src str>> {
    let mut docs = template;
    docs.extend(invocation.iter().cloned());
    docs
}

/// The template's children come first; the invocation appends its own.
fn bodies<T: Clone>(
    template: Option<Spanned<Vec<Spanned<T>>>>,
    invocation: Option<Spanned<Vec<Spanned<T>>>>,
) -> Option<Spanned<Vec<Spanned<T>>>> {
    match (template, invocation) {
        (template, None) => template,
        (None, invocation) => invocation,
        (Some(template), Some(invocation)) => Some(Spanned {
            inner: template
                .inner
                .into_iter()
                .chain(invocation.inner)
                .collect(),
            span: invocation.span,
        }),
    }
}
