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
peripheral::register::transition(|reg| reg.foo(&dyn_foo).bar());
```
> Statically resolve a *resolvable* but currently dynamic field state.
