//! Syntactic analysis: [`Token`]s → [`File`].
//!
//! Each definition kind has its own grammar: symbols are contextual operators,
//! so `~` parses only after a variant's name, `reset` only on registers and
//! fields, and so on. Containers likewise parse only the item kinds that may
//! appear within them. The containment hierarchy is strictly top-down, so no
//! parser (except the token-level block skipper) is recursive.
//!
//! Head properties follow a canonical order: domain, schema reference, reset,
//! `requires` clauses (plain, `write`, `hardware write`), then the body.

use chumsky::{input::ValueInput, prelude::*};

use crate::{
    ast,
    ast::{
        Access, Device, DeviceItem, Domain, Field, FieldItem, FieldRequires, File, FileItem, Group,
        GroupItem, Head, Import, Indices, InterruptEntry, InterruptKind, Interrupts, ListEntry,
        NumRange, Path, Pattern, PeripheralItem, RegisterItem, Rest, ResetValue, Schema,
        SchemaRef, Space, Span, Spanned, Stride, ValueEntry, VariantValue,
    },
    lexer::Token,
};

type Extra<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, Span>>;

/// A brace-balanced block of arbitrary tokens.
fn block<'tokens, 'src: 'tokens, I>() -> impl Parser<'tokens, I, (), Extra<'tokens, 'src>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    recursive(|block| {
        let not_brace = any()
            .and_is(one_of([Token::LBrace, Token::RBrace]).not())
            .ignored();

        just(Token::LBrace)
            .ignore_then(choice((block, not_brace)).repeated())
            .then_ignore(just(Token::RBrace))
            .ignored()
    })
}

/// Consume a (nonempty) unparseable region up to the next plausible item or
/// enclosing closing brace — swallowing brace-balanced blocks whole — leaving
/// `fallback` in its place.
fn garbage<'tokens, 'src: 'tokens, I, T>(
    fallback: T,
) -> impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    T: Clone,
{
    let item_start = select! {
        Token::Doc(..) => (),
        Token::Device => (),
        Token::Peripheral => (),
        Token::Register => (),
        Token::Field => (),
        Token::Schema => (),
        Token::Variant => (),
        Token::Import => (),
        Token::Interrupts => (),
        Token::Read => (),
        Token::Write => (),
        Token::Store => (),
        Token::Volatile => (),
        Token::Leaky => (),
        Token::Inert => (),
    };

    let sync = item_start.or(one_of([Token::LBrace, Token::RBrace]).ignored());

    let first = choice((block(), any().and_is(just(Token::RBrace).not()).ignored()));

    let rest = choice((block(), any().and_is(sync.not()).ignored()));

    first.then(rest.repeated()).to(fallback)
}

/// A braced list of items, recovering from an unrecoverable inner error by
/// discarding the whole (brace-balanced) body.
fn body<'tokens, 'src: 'tokens, I, T>(
    item: impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone,
) -> impl Parser<'tokens, I, Spanned<Vec<Spanned<T>>>, Extra<'tokens, 'src>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    item.spanned()
        .repeated()
        .collect()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .spanned()
        .recover_with(via_parser(nested_delimiters(
            Token::LBrace,
            Token::RBrace,
            [
                (Token::LParen, Token::RParen),
                (Token::LBracket, Token::RBracket),
            ],
            |span| Spanned {
                inner: Vec::new(),
                span,
            },
        )))
}

/// A bracketed, comma-separated list, with each entry spanned.
fn bracketed_list<'tokens, 'src: 'tokens, I, T>(
    entry: impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone,
) -> impl Parser<'tokens, I, Vec<Spanned<T>>, Extra<'tokens, 'src>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    entry
        .spanned()
        .separated_by(just(Token::Comma))
        .at_least(1)
        .allow_trailing()
        .collect()
        .delimited_by(just(Token::LBracket), just(Token::RBracket))
}

pub fn file_parser<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, File<'src>, Extra<'tokens, 'src>>
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    // atoms

    let ident = select! { Token::Ident(s) => s }.labelled("identifier");
    let num = select! { Token::Num(n) => n }.labelled("number");
    let doc = select! { Token::Doc(s) => s }
        .labelled("doc comment")
        .spanned();
    let docs = doc.clone().repeated().collect::<Vec<_>>();

    let path = ident
        .spanned()
        .separated_by(just(Token::Dot))
        .at_least(1)
        .collect()
        .map(|segments| Path { segments })
        .labelled("path");

    let range = num
        .spanned()
        .then(
            select! { Token::DotDot => false, Token::DotDotEq => true }.labelled("range operator"),
        )
        .then(num.spanned())
        .map(|((start, inclusive), end)| NumRange {
            start,
            end,
            inclusive,
        })
        .labelled("range");

    let leaky = just(Token::Leaky)
        .to(())
        .spanned()
        .map(|s: Spanned<()>| s.span)
        .or_not();
    let inert = just(Token::Inert)
        .to(())
        .spanned()
        .map(|s: Spanned<()>| s.span)
        .or_not();
    let array = just(Token::Array)
        .to(())
        .spanned()
        .map(|s: Spanned<()>| s.span)
        .or_not();

    // access modalities compose as a set; each word may appear at most once
    let access = choice((
        just(Token::Volatile)
            .then(just(Token::Store))
            .to(Access::VolatileStore)
            .spanned()
            .map(|a| vec![a]),
        just(Token::Store)
            .to(Access::Store)
            .spanned()
            .map(|a| vec![a]),
        just(Token::Read)
            .to(Access::Read)
            .spanned()
            .then(just(Token::Write).to(Access::Write).spanned().or_not())
            .map(|(r, w)| std::iter::once(r).chain(w).collect()),
        just(Token::Write)
            .to(Access::Write)
            .spanned()
            .then(just(Token::Read).to(Access::Read).spanned().or_not())
            .map(|(w, r)| std::iter::once(w).chain(r).collect()),
    ))
    .labelled("access modality")
    .or_not()
    .map(Option::unwrap_or_default);

    let indices = choice((
        range
            .clone()
            .map(Indices::Range)
            .delimited_by(just(Token::LBracket), just(Token::RBracket)),
        bracketed_list(ident).map(Indices::Names),
    ))
    .spanned()
    .labelled("array indices");

    // `#template`, `#template as name`, or `name`
    let plain_head = choice((
        just(Token::Hash)
            .ignore_then(path.clone().spanned())
            .then(just(Token::As).ignore_then(ident.spanned()).or_not())
            .map(|(template, name)| Head {
                template: Some(template),
                name,
                indices: None,
            }),
        ident.spanned().map(|name| Head {
            template: None,
            name: Some(name),
            indices: None,
        }),
    ))
    .or_not()
    .map(Option::unwrap_or_default);

    // as above, where names may carry array indices
    let indexed_head = {
        let name_part = ident.spanned().then(indices.or_not());

        choice((
            just(Token::Hash)
                .ignore_then(path.clone().spanned())
                .then(just(Token::As).ignore_then(name_part.clone()).or_not())
                .map(|(template, name_part)| match name_part {
                    Some((name, indices)) => Head {
                        template: Some(template),
                        name: Some(name),
                        indices,
                    },
                    None => Head {
                        template: Some(template),
                        name: None,
                        indices: None,
                    },
                }),
            name_part.map(|(name, indices)| Head {
                template: None,
                name: Some(name),
                indices,
            }),
        ))
        .or_not()
        .map(Option::unwrap_or_default)
    };

    // `...`, `...+0x4`, `...-0x4` — continue the pattern, optionally with an
    // explicit stride
    let rest = just(Token::Ellipsis)
        .ignore_then(
            choice((just(Token::Plus).to(false), just(Token::Minus).to(true)))
                .then(num)
                .map(|(negative, magnitude)| Stride {
                    negative,
                    magnitude,
                })
                .spanned()
                .or_not(),
        )
        .map(|stride| Rest { stride });

    let list_entry = choice((
        rest.clone().map(ListEntry::Rest),
        range.clone().map(ListEntry::Range),
        num.map(ListEntry::Value),
    ));

    let domain = just(Token::At)
        .ignore_then(
            choice((
                bracketed_list(list_entry).map(Domain::List),
                range.clone().map(Domain::Range),
                num.map(Domain::Value),
            ))
            .spanned()
            .labelled("domain"),
        )
        .or_not();

    let schema_ref = choice((
        just(Token::Extends)
            .ignore_then(path.clone())
            .map(SchemaRef::Extends),
        just(Token::Assumes)
            .ignore_then(path.clone())
            .map(SchemaRef::Assumes),
    ))
    .spanned()
    .or_not();

    let reset = just(Token::Reset)
        .ignore_then(
            choice((num.map(ResetValue::Value), ident.map(ResetValue::Variant)))
                .spanned()
                .labelled("reset value"),
        )
        .or_not();

    let value = {
        let entry = choice((
            rest.clone().map(ValueEntry::Rest),
            num.map(ValueEntry::Value),
        ));

        just(Token::Tilde)
            .ignore_then(
                choice((
                    bracketed_list(entry).map(VariantValue::List),
                    num.map(VariantValue::Value),
                ))
                .spanned()
                .labelled("value"),
            )
            .or_not()
    };

    // a requirement space in disjunctive form: patterns of `&`-joined entitlements,
    // joined by `|`, with parentheses permitted (only) around each pattern
    let space = {
        let pattern = path
            .clone()
            .spanned()
            .separated_by(just(Token::Amp))
            .at_least(1)
            .collect()
            .map(|entitlements| Pattern { entitlements });

        choice((
            pattern
                .clone()
                .delimited_by(just(Token::LParen), just(Token::RParen)),
            pattern,
        ))
        .spanned()
        .separated_by(just(Token::Pipe))
        .at_least(1)
        .collect()
        .map(|patterns| Space { patterns })
        .labelled("requirement")
    };

    let requires = just(Token::Requires)
        .ignore_then(space.clone().spanned())
        .or_not();

    let write_requires = just(Token::Write)
        .ignore_then(just(Token::Requires))
        .ignore_then(space.clone().spanned())
        .or_not();

    let hardware_write_requires = just(Token::Hardware)
        .ignore_then(just(Token::Write))
        .ignore_then(just(Token::Requires))
        .ignore_then(space.clone().spanned())
        .or_not();

    // definitions, bottom-up

    let variant = docs
        .clone()
        .then(leaky.clone())
        .then(inert)
        .then_ignore(just(Token::Variant))
        .then(array.clone())
        .then(indexed_head.clone())
        .then(value)
        .then(requires.clone())
        .map(
            |((((((docs, leaky), inert), array), head), value), requires)| ast::Variant {
                docs,
                leaky,
                inert,
                array,
                head,
                value,
                requires,
            },
        )
        .labelled("variant")
        .boxed();

    let variant_body = body(
        variant
            .clone()
            .map(FieldItem::Variant)
            .recover_with(via_parser(garbage(FieldItem::Error))),
    );

    let schema = docs
        .clone()
        .then(leaky.clone())
        .then(access.clone())
        .then_ignore(just(Token::Schema))
        .then(plain_head.clone())
        .then(variant_body.clone().or_not())
        .map(|((((docs, leaky), access), head), body)| Schema {
            docs,
            leaky,
            access,
            head,
            body,
        })
        .labelled("schema")
        .boxed();

    let field = docs
        .clone()
        .then(leaky.clone())
        .then(access.clone())
        .then_ignore(just(Token::Field))
        .then(array.clone())
        .then(indexed_head.clone())
        .then(domain.clone())
        .then(schema_ref)
        .then(reset.clone())
        .then(requires.clone())
        .then(write_requires)
        .then(hardware_write_requires)
        .then(variant_body.or_not())
        .map(
            |(
                (
                    (
                        (
                            (((((((docs, leaky), access), array), head), domain), schema), reset),
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
                schema,
                reset,
                requires: FieldRequires {
                    plain,
                    write,
                    hardware_write,
                },
                body,
            },
        )
        .labelled("field")
        .boxed();

    let field_group = docs
        .clone()
        .then_ignore(just(Token::Field))
        .then_ignore(just(Token::Group))
        .then(ident.spanned().or_not())
        .then(body(
            choice((
                schema.clone().map(GroupItem::Schema),
                field.clone().map(GroupItem::Member),
            ))
            .recover_with(via_parser(garbage(GroupItem::Error))),
        ))
        .map(|((docs, name), body)| Group { docs, name, body })
        .labelled("field group")
        .boxed();

    let register = docs
        .clone()
        .then(leaky.clone())
        .then_ignore(just(Token::Register))
        .then(array.clone())
        .then(indexed_head.clone())
        .then(domain.clone())
        .then(reset)
        .then(
            body(
                choice((
                    field_group.clone().map(RegisterItem::FieldGroup),
                    field.clone().map(RegisterItem::Field),
                    schema.clone().map(RegisterItem::Schema),
                ))
                .recover_with(via_parser(garbage(RegisterItem::Error))),
            )
            .or_not(),
        )
        .map(
            |((((((docs, leaky), array), head), domain), reset), body)| ast::Register {
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
        .boxed();

    let register_group = docs
        .clone()
        .then_ignore(just(Token::Register))
        .then_ignore(just(Token::Group))
        .then(ident.spanned().or_not())
        .then(body(
            choice((
                schema.clone().map(GroupItem::Schema),
                register.clone().map(GroupItem::Member),
            ))
            .recover_with(via_parser(garbage(GroupItem::Error))),
        ))
        .map(|((docs, name), body)| Group { docs, name, body })
        .labelled("register group")
        .boxed();

    let peripheral = docs
        .clone()
        .then(leaky)
        .then_ignore(just(Token::Peripheral))
        .then(array)
        .then(indexed_head)
        .then(domain)
        .then(requires)
        .then(
            body(
                choice((
                    register_group.clone().map(PeripheralItem::RegisterGroup),
                    register.clone().map(PeripheralItem::Register),
                    schema.clone().map(PeripheralItem::Schema),
                ))
                .recover_with(via_parser(garbage(PeripheralItem::Error))),
            )
            .or_not(),
        )
        .map(
            |((((((docs, leaky), array), head), domain), requires), body)| ast::Peripheral {
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
        .boxed();

    let peripheral_group = docs
        .clone()
        .then_ignore(just(Token::Peripheral))
        .then_ignore(just(Token::Group))
        .then(ident.spanned().or_not())
        .then(body(
            choice((
                schema.clone().map(GroupItem::Schema),
                peripheral.clone().map(GroupItem::Member),
            ))
            .recover_with(via_parser(garbage(GroupItem::Error))),
        ))
        .map(|((docs, name), body)| Group { docs, name, body })
        .labelled("peripheral group")
        .boxed();

    let interrupts = {
        let entry = docs
            .clone()
            .then(choice((
                just(Token::Reserved).to(InterruptKind::Reserved),
                ident.spanned().map(InterruptKind::Handler),
            )))
            .map(|(docs, kind)| InterruptEntry { docs, kind })
            .spanned();

        just(Token::Interrupts)
            .ignore_then(
                entry
                    .repeated()
                    .collect()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map(|entries| Interrupts { entries })
            .labelled("interrupts")
    };

    let device = docs
        .clone()
        .then_ignore(just(Token::Device))
        .then(plain_head)
        .then(
            body(
                choice((
                    peripheral_group.clone().map(DeviceItem::PeripheralGroup),
                    peripheral.clone().map(DeviceItem::Peripheral),
                    schema.clone().map(DeviceItem::Schema),
                    interrupts.map(DeviceItem::Interrupts),
                ))
                .recover_with(via_parser(garbage(DeviceItem::Error))),
            )
            .or_not(),
        )
        .map(|((docs, head), body)| Device { docs, head, body })
        .labelled("device")
        .boxed();

    let import = just(Token::Import)
        .ignore_then(path.spanned())
        .map(|path| Import { path })
        .labelled("import");

    let item = choice((
        import.map(FileItem::Import),
        device.map(FileItem::Device),
        peripheral_group.map(FileItem::PeripheralGroup),
        peripheral.map(FileItem::Peripheral),
        register_group.map(FileItem::RegisterGroup),
        register.map(FileItem::Register),
        field_group.map(FileItem::FieldGroup),
        field.map(FileItem::Field),
        schema.map(FileItem::Schema),
        variant.map(FileItem::Variant),
    ))
    .recover_with(via_parser(garbage(FileItem::Error)));

    item.spanned()
        .repeated()
        .collect()
        .map(|items| File { items })
        .then_ignore(end())
}
