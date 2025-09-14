My ramblings so I don't forget good ideas.

# Dynamics

Every field, no matter what, will have a "state" called `Dynamic`. This "state" allows
for statically trackable (resolvable) field states to be demoted to runtime tracked.
Additionally, unresolvable field states which necessarily are *not* statically tracked
**only** have the `Dynamic` state such that ownership rules are still enforced.

It wouldn't be good for different places to interact with hardware, unaware of the fact
that other places are interacting with the same hardware, so writes (or any operation
with implicative effects) will require `&mut some_field::Dynamic`.

Another benefit to requiring `Dynamic` references to perform operations on fields, is that
fields within transient peripherals cannot accidentally be interacted with as the `Dynamic`
instance is dispatched when unmasking the peripheral, necessarily meaning the peripheral
is present and ready for use.

Examples:

```rust
peripheral::register::modify(|_, w| w.foo(&old.foo).bar());
```
> Read or write requires `&Dynamic` to enforce ownership and peripheral availability.

```rust
// converting a resolved state into a dynamic is a noop
let dyn_foo = old.foo.into_dynamic();

// now foo can be used dynamically
peripheral::register::modify(|_, w| w.foo(&dyn_foo).bar());
```

```rust
peripheral::register::transition(|reg| reg.foo(dyn_foo).bar());
```
> Statically resolve a *resolvable* but currently dynamic field state.

## Implicative Effects

As it is now, reading or writing to a dynamic field requires `&mut Dynamic`. But,
if reading a field has no implicative effects, really only a `&Dynamic` is needed.
As it is now, proto-hal does not track nor express effects (yet). So I will leave
it as is in the interest of erring on the side of caution, but once effects are
tracked, the `&mut Dynamic` can be relaxed to `&Dynamic`.

# Field Traits

There are three situations where a field state constraint could be applied:
1. The bound for a state that pertains to a particular field.
1. The bound for a state to be trainsitioned from.
1. The bound for a state to be transitioned to.

1 is useful for the transition builder, requiring each generic to specifically be a state
for the corresponding field of that generic position.

2 implies 1, requires no additional implementation details, and is implemented for less
types. For example, 2 is not implemented for `Unresolved` but 1 is.

3 imples 1, requires transition related implementation details, and is implemented for
less types than 1, and only includes types from 2 which are resolvable. For example,
3 is not implemented for `Dynamic` but 2 is.

## Signatures

```rust
pub trait Position<T> {}
```

```rust
pub trait Outgoing<T> {}
```

```rust
pub trait Incoming<T> {
    type Raw;
    const RAW: Self::Raw;
}
```

# Interness

Some fields accept values which do nothing. This is useful for "set" or "clear" registers (registers
designed for setting or clearing specific fields without touching the other fields).
This noop variant is **inert**. Encapsulating this quality in the `Variant` ir struct
will enable useful functionality later...

# The Ultimate Writer

Currently, proto-hal MMIO access is facilitated by the following structures:
1. UnsafeReader
1. UnsafeWriter
1. Reader
1. Writer
1. TransitionBuilder<...>

The unsafe reader/writers expose all fields by borrowing the appropriate `Dynamic`s.
Naturally, reading or writing from any field without proto-hal's type-enforced
guarantees may be unsound, so these operations are `unsafe`.

The normal reader/writers expose fields which *are not* statically tracked, this includes:
1. Unresolvable fields, which *can not* be statically tracked.
1. Resolvable fields that are not entitled to and are currently dynamic.

> Note: For now, only fields which are not entitled to can become dynamic, because
entitlements are enforced in the type system. It may be possible that dynamic entitlement enforcement can be implemented, but I haven't thought of a sound way to do that yet.

The transition builder exposes resolvable fields to have their state transitioned statically.

The distinction between these access types expresses that different fields within the same
register have different access qualities. For instance, in a register with read-only fields, and write-only fields, the reader and writer only exposes the appropriate fields and fills
the invalid fields with dummy values (probably 0).

But what if all fields can be written to, but only some values induce action? What if
those fields are superpositioned with other fields with different schemas?

What if you wanted to transition states *and* write to unresolvable fields at the same time?

I realized that I needed a design that was fully generalized. Rather than handling a constant
number of field read/write qualities, it needed to arbitrarily extend to any future
constraints.

---

Consider: What if there was a single writer type, and no transition builder. The writer
fully expresses the fields with generics (like the transition builder) but unlike the
transition builder, expresses unresolvable fields as well.

But how does the writer constrain what field values/states may be written/transitioned?
This will be achieved via the gate functions. The writer type is the same, but the bounds
applied by the gate determine what actions the writer permits.

Registers with readable and writable fields expose `modify`:

```rust
let some::reg::States { foo, .. } = some::reg::modify(|r, w| {
    w
        .foo(foo).baz()
        .bar(&mut bar, r.bar() + 1)
});
```
> An example of performing a state transition and reading/writing to an unresolvable field
> simultaneously.
>
> `foo` is a resolvable enumerated field. `bar` is an unresolvable numeric field.
> The generics in `States` are still only for resolvable fields.

Registers with writable fields expose `write`:

```rust
let some::reg::States { foo, .. } = some::reg::write(|w| {
    w
        .foo(foo).baz()
        .bar(&mut bar, 0xdeadbeef)
});
```
> An example of performing a state transition and writing to an unresolvable field
> simultaneously. Since unspecified fields have inert values, a read need not be performed
> and this "blind write" is sound.
>
> Unlike the previous example, `write` requires fields without inert values to be specified.
>
> `foo` is a resolvable enumerated field. `bar` is an unresolvable numeric field.
> All other fields have at least one inert write variant.
> The generics in `States` are still only for resolvable fields.

# Access Entitlement Quandaries

If a field's read access is conditional, then there exists a set of
entitlements such that the field is writable but not readable. This means the `modify`
function would exist, and could read unreadable fields, then write back to them. This
could potentially read undefined values and write them back. Of course, an unacceptable
scenario.

This means that for fields of this kind, the `modify` function must:

1. Prefill an inert value if one exists.
1. Require the final value in the writer to be corporeal.

This is the same logic employed by `write`, but only for the fields of discussion, rather
than all writable fields.

If at some point the field *is* readable and writable with no inert variants and the user
intends to retain the value while modifying other fields, the user must use the reader to
capture the value, and forward it to the writer.

This kind of feels bad... also, could this not be represented by two superpositioned fields?
Does that mean if someone where to do so it would be unsound?

For now, conditional readability with any kind of writability will be forbidden.

# Arrays

TODO:
- ~~New trait in stasis to constrain index types.~~
- ~~Make structs for each element based on provided ident factory.~~
- Variant needs to look up peripheral, register, field from entitlement to correctly format the path.
- New validation step that ensures peripheral array idents don't collide with other peripherals.
- New validation step that ensures field array idents don't collide with field variant names.
