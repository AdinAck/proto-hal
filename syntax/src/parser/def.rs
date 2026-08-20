//! Definition parsers, composed bottom-up: variants, then fields and schemas,
//! then registers, peripherals, and finally the device.

use chumsky::prelude::*;

use super::{
    Extra, TokenInput,
    atom::{access, body, docs, ident, indexed_head, marker, plain_head, side},
    prop::{
        assumes, domain, extends, hardware_write_requires, requires, reset, value, write_requires,
    },
    recovery::garbage,
};
use crate::{
    ast::{
        Device, DeviceItem, Field, FieldGroup, FieldItem, FieldRequires, Group, GroupItem, Import,
        InterruptEntry, InterruptKind, Interrupts, Peripheral, PeripheralGroup, PeripheralItem,
        Register, RegisterGroup, RegisterItem, Schema, Variant,
    },
    parser::atom::path,
    token::token,
};

/// `variant Name ~ value requires ...`
pub(crate) fn variant<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Variant<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    docs()
        .then(marker(token![Leaky]))
        .then(marker(token![Inert]))
        .then(side())
        .then_ignore(just(token![Variant]))
        .then(marker(token![Array]))
        .then(indexed_head())
        .then(value())
        .then(requires())
        .map(
            |(((((((docs, leaky), inert), side), array), head), value), requires)| Variant {
                docs,
                leaky,
                inert,
                side,
                array,
                head,
                value,
                requires,
            },
        )
        .labelled("variant")
        .boxed()
}

/// A field or schema body: variants.
fn variant_body<'tokens, 'src: 'tokens, I>() -> impl Parser<
    'tokens,
    I,
    crate::ast::Spanned<Vec<crate::ast::Spanned<FieldItem<'src>>>>,
    Extra<'tokens, 'src>,
> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    body(
        variant()
            .map(FieldItem::Variant)
            .recover_with(via_parser(garbage(FieldItem::Error))),
    )
}

/// `schema name { variants }`
pub(crate) fn schema<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Schema<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    docs()
        .then(marker(token![Leaky]))
        .then_ignore(just(token![Schema]))
        .then(plain_head())
        .then(variant_body().or_not())
        .map(|(((docs, leaky), head), body)| Schema {
            docs,
            leaky,
            head,
            body,
        })
        .labelled("schema")
        .boxed()
}

/// `access field name @ domain { variants }`
pub(crate) fn field<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Field<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    docs()
        .then(marker(token![Leaky]))
        .then(access())
        .then_ignore(just(token![Field]))
        .then(marker(token![Array]))
        .then(indexed_head())
        .then(domain())
        .then(assumes())
        .then(extends())
        .then(reset())
        .then(requires())
        .then(write_requires())
        .then(hardware_write_requires())
        .then(variant_body().or_not())
        .map(
            |(
                (
                    (
                        (
                            (
                                (
                                    ((((((docs, leaky), access), array), head), domain), assumes),
                                    extends,
                                ),
                                reset,
                            ),
                            plain,
                        ),
                        write,
                    ),
                    hardware_write,
                ),
                body,
            )| Field {
                docs,
                leaky,
                access,
                array,
                head,
                domain,
                assumes,
                extends,
                reset,
                requires: FieldRequires {
                    inherent: plain,
                    write,
                    hardware_write,
                },
                body,
            },
        )
        .labelled("field")
        .boxed()
}

/// `field group name { fields and schemas }`
pub(crate) fn field_group<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, FieldGroup<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    docs()
        .then_ignore(just(token![Field]))
        .then_ignore(just(token![Group]))
        .then(ident().spanned().or_not())
        .then(body(
            choice((
                schema().map(GroupItem::Schema),
                field().map(GroupItem::Member),
            ))
            .recover_with(via_parser(garbage(GroupItem::Error))),
        ))
        .map(|((docs, name), body)| Group { docs, name, body })
        .labelled("field group")
        .boxed()
}

/// `register name @ offset reset value { fields }`
pub(crate) fn register<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Register<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    docs()
        .then(marker(token![Leaky]))
        .then_ignore(just(token![Register]))
        .then(marker(token![Array]))
        .then(indexed_head())
        .then(domain())
        .then(reset())
        .then(
            body(
                choice((
                    field_group().map(RegisterItem::FieldGroup),
                    field().map(RegisterItem::Field),
                    schema().map(RegisterItem::Schema),
                ))
                .recover_with(via_parser(garbage(RegisterItem::Error))),
            )
            .or_not(),
        )
        .map(
            |((((((docs, leaky), array), head), domain), reset), body)| Register {
                docs,
                leaky,
                array,
                head,
                domain,
                reset,
                body,
            },
        )
        .labelled("register")
        .boxed()
}

/// `register group name { registers and schemas }`
pub(crate) fn register_group<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, RegisterGroup<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    docs()
        .then_ignore(just(token![Register]))
        .then_ignore(just(token![Group]))
        .then(ident().spanned().or_not())
        .then(body(
            choice((
                schema().map(GroupItem::Schema),
                register().map(GroupItem::Member),
            ))
            .recover_with(via_parser(garbage(GroupItem::Error))),
        ))
        .map(|((docs, name), body)| Group { docs, name, body })
        .labelled("register group")
        .boxed()
}

/// `peripheral name @ base requires ... { registers }`
pub(crate) fn peripheral<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Peripheral<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    docs()
        .then(marker(token![Leaky]))
        .then_ignore(just(token![Peripheral]))
        .then(marker(token![Array]))
        .then(indexed_head())
        .then(domain())
        .then(requires())
        .then(
            body(
                choice((
                    register_group().map(PeripheralItem::RegisterGroup),
                    register().map(PeripheralItem::Register),
                    schema().map(PeripheralItem::Schema),
                ))
                .recover_with(via_parser(garbage(PeripheralItem::Error))),
            )
            .or_not(),
        )
        .map(
            |((((((docs, leaky), array), head), domain), requires), body)| Peripheral {
                docs,
                leaky,
                array,
                head,
                domain,
                requires,
                body,
            },
        )
        .labelled("peripheral")
        .boxed()
}

/// `peripheral group name { peripherals and schemas }`
pub(crate) fn peripheral_group<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, PeripheralGroup<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    docs()
        .then_ignore(just(token![Peripheral]))
        .then_ignore(just(token![Group]))
        .then(ident().spanned().or_not())
        .then(body(
            choice((
                schema().map(GroupItem::Schema),
                peripheral().map(GroupItem::Member),
            ))
            .recover_with(via_parser(garbage(GroupItem::Error))),
        ))
        .map(|((docs, name), body)| Group { docs, name, body })
        .labelled("peripheral group")
        .boxed()
}

/// `interrupts { ... }` — the vector table, in order.
pub(crate) fn interrupts<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Interrupts<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    let entry = docs()
        .then(choice((
            just(token![Reserved]).to(InterruptKind::Reserved),
            ident().spanned().map(InterruptKind::Handler),
        )))
        .map(|(docs, kind)| InterruptEntry { docs, kind })
        .spanned();

    just(token![Interrupts])
        .ignore_then(
            entry
                .repeated()
                .collect()
                .delimited_by(just(token![LBrace]), just(token![RBrace])),
        )
        .map(|entries| Interrupts { entries })
        .labelled("interrupts")
}

/// `device name { peripherals, schemas, interrupts }`
pub(crate) fn device<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Device<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    docs()
        .then_ignore(just(token![Device]))
        .then(plain_head())
        .then(
            body(
                choice((
                    peripheral_group().map(DeviceItem::PeripheralGroup),
                    peripheral().map(DeviceItem::Peripheral),
                    schema().map(DeviceItem::Schema),
                    interrupts().map(DeviceItem::Interrupts),
                ))
                .recover_with(via_parser(garbage(DeviceItem::Error))),
            )
            .or_not(),
        )
        .map(|((docs, head), body)| Device { docs, head, body })
        .labelled("device")
        .boxed()
}

/// `import path`
pub(crate) fn import<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Import<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    just(token![Import])
        .ignore_then(path().spanned())
        .map(|path| Import { path })
        .labelled("import")
}
