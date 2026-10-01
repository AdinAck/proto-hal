# Design

The axioms of the modeling language: what a description means, and the rules every construct
follows. Syntax that has not been settled is left out; [open questions](#open-questions) are listed
at the end.

## Declarative

A description states what the hardware *is*. Nothing in it is computed or executed.

## The device

`device` is the root of elaboration, as a top module is in Verilog. Only what the device contains
exists in the model.

Everything outside the device is *non-corporeal*: definitions the device uses, or library
components shared between devices. A definition occupies nothing until the device uses it.

Devices, peripherals, registers, and fields are *structures*: they hold values, and occupy space.
Variants and interrupts are not; they are values that structures inhabit.

## Declarations

Every declaration has one shape: its [qualifiers](#qualifiers), its kind, its name, its
[properties](#properties), and its body.

```
leaky store field mode[0..16] as pin @ [1:0, ...] assumes mode { ... }
```

Everything about an item is written before its body. The body, in braces, holds its children:

| kind | children |
|---|---|
| device | peripherals, groups of peripherals, its interrupt table |
| peripheral | registers, groups of registers |
| register | fields, groups of fields |
| field | variants (its inherent schema) |
| schema | variants |

```
schema toggle {
    variant Disabled ~ 0
    variant Enabled ~ 1
}
```

A *group* collects children under a name: `group channels { ... }`. Its members are of the kind its
parent holds, so a group has no kind of its own. Groups appear only where the model has them: in a
device, a peripheral, or a register.

A group may be an array, and may be placed: its members' placements are relative to its own. So a
group describes a structure that repeats, like a DMA channel's four registers:

```
group ch[1, ...] @ [0x8, ... by 0x14] {
    register ccr @ 0x0 { ... }
    register cndtr @ 0x4 { ... }
    register cpar @ 0x8 { ... }
    register cmar @ 0xc { ... }
}
```

A doc comment, `///`, documents the declaration that follows it.

## Imports

An import brings another file's definitions into scope, binding the last segment of its path, or
the name given by `as`:

```
import st.gpio          // gpio.port, gpio.mode
import st.gpio as g     // g.port, g.mode
```

Import paths are relative to the project root, marked by a manifest file (provisionally
`mioml.toml`). Each segment but the last names a directory, and the last a file: `import st.gpio`
reads `st/gpio.mio`.

## Templates

Peripherals are largely the same across microcontroller families, but differ in small ways. A
template states the common structure, and an instance adds what is particular to it.

An instance *refines* its templates by naming them: `peripheral gpioa refines gpio`. It may refine
several, i.e. `refines a, b`.

`refines` and `assumes` name *definitions*: those at the top level of a file, and those imported;
never structures in the device. So an instance may share its template's name:
`peripheral cordic refines cordic`.

An instance and its templates are combined by *unification*. `refines` comes before it: it says
what a declaration is unified with, so it is not itself unified. A template's child and an
instance's child may each refine templates of their own, and all of them apply.

For every other property, and every qualifier:

| template | instance | result |
|---|---|---|
| unset | unset | unset; fails validation if the property is required |
| set | unset | the template's value |
| unset | set | the instance's value |
| set | set, equal | that value |
| set | set, different | fails validation |

Nothing is overridden. An omitted qualifier is inherited (`field gif` refines a template's
`read field gif`), and there is no way to remove one. Children unify the same way: an instance
refines a template's child by naming it, kind and all.

```
peripheral gpio {
    register moder @ 0x0              // no reset: it differs between ports
}

peripheral gpioa refines gpio @ 0x4800_0000 {
    register moder reset 0xabffffff   // refines the template's `moder`
}
```

Arrays unify element by element: `register ccr[1..=6]` in an instance refines `ccr1` through
`ccr6`, and `register ccr3 { ... }` refines `ccr3` alone. Arrays, sets, and progressions are
notation for describing many elements at once; the model holds every element, and each is directly
referable.

A consequence is that a template must leave open whatever varies between its instances. This is
intended: a template states only what is truly common.

A template is *closed*: nothing within it can see beyond it, so it is verifiable on its own. Within
a template, its own name refers to itself, and so to each of its instances. What lies outside a
template is stated where it is used.

## Qualifiers

The words before an item's kind are its *qualifiers*. They are written in a fixed order:

```
leaky inert <access> <kind>
```

i.e. `leaky volatile store field`.

There is no `array` qualifier: a [name expansion](#arrays) makes an item an array.

## Properties

What follows an item's name are its *properties*, i.e. `reset`, `assumes`, `requires`.

Two properties are *intrinsic*: every item of their kind has them, so they are always required
(after unification). Other properties are not required by the structure, though one may be required
by meaning, i.e. a resolvable field needs a reset.

- `@`, the *placement* operator, gives where an item sits in its parent: a peripheral's address, a
  register's offset, a field's domain.
- `~`, the *discriminant* operator, gives a variant's discriminant.

Intrinsic properties are introduced by symbols, as they appear on every item of their kind; all
other properties are introduced by words.

Properties may be written in any order, each at most once.

## Schemas

A schema is a reusable set of variants. Each variant is plain, `read`, or `write`.

Schemas have no access modality; fields do. A field's variants come from its *inherent* schema
(written within the field), and from schemas it uses:

- **Assuming** a schema shares it: the field's variants are the schema's, types and all. Where the
  types are generated is an emergent property of the description: in the deepest module that
  encloses every field assuming the schema. Since `assumes` names only the schema's definition, a
  template that assumes a schema is still verifiable on its own.
- **Refining** schemas copies their variants into the field's inherent schema, as refining any
  [template](#templates) does. A field may refine several schemas.

A schema is distinct from other templates only in that it can be assumed.

Whether assumed or refined, a schema must be compatible with the field's access modality. A
variant's marking and the field's modality are separate statements, which must agree:

| field | may use |
|---|---|
| `read` | plain and `read` variants |
| `write` | plain and `write` variants |
| `read write` | any variants |
| `store`, `volatile store` | plain variants only |

An incompatible variant is an error, never ignored.

A `read write` field has two numericities, one read and one write. A variant marked `read` or
`write` belongs to that numericity; unmarked, it belongs to both.

Variants in a schema carry no requirements: statewise [entitlements](#entitlements) are only ever
specified in fields. A field gives requirements to the variants of a schema it assumes by naming
them, as an instance refines a template's child. Naming a variant the schema does not have is an
error: an assumed schema is shared, so it cannot grow.

## Entitlements

On a field:

- `requires ...` alone is the field's *ontological* entitlement space.
- `write requires ...` is its *affordance* entitlement space.
- `hardware write requires ...` is its *hardware affordance* entitlement space.

Each is a value: written at most once, and unified as any value is. A second clause could add a
pattern to the space or an entitlement to every pattern, and nothing would say which; so a template
whose instances add requirements leaves them unset.

### Requirements

A requirement is an entitlement space. `|` separates its patterns, and `&` joins a pattern's
entitlements. There are no parentheses: `&` always joins first.

| written | meaning |
|---|---|
| `a.x.On` | a space with one pattern, holding one entitlement |
| `a.x.{On, Off}` | a space with one pattern, holding two entitlements of one field |
| `a.x.On & b.y.On` | a space with one pattern, holding entitlements of two fields |
| `a.x.On \| b.y.On` | a space with two patterns, each holding one entitlement |
| `a.x.On & b.y.On \| c.z.Off` | a space with two patterns: the first holding entitlements of two fields, the second one entitlement |
| `a.x.{On, Off} & b.y.On` | a space with one pattern, holding two entitlements of one field and one of another |
| `a.x.{On, Off} \| b.y.On` | a space with two patterns: the first holding two entitlements of one field, the second one entitlement |
| `adc{1, 2}.cr.aden.Disabled` | a space with one pattern, holding entitlements of two fields |
| `adc{1, 2}.cr.aden.Disabled \| c.z.Off` | a space with two patterns: the first holding entitlements of two fields, the second one entitlement |

Separating one field's entitlements, `a.x.On | a.x.Off`, warns: it is written `a.x.{On, Off}`.

These fail validation:

| written | why |
|---|---|
| `a.x.On & a.x.Off` | `&` joining one field's entitlements would read as "and" but mean "or"; it is written `a.x.{On, Off}` |
| `a.x.On & (b.y.On \| c.z.Off)` | not a space of patterns: an entitlement joined to a space |
| `a.x.On &`, `\| a.x.On` | a missing operand |
| a second `requires` | `requires` is a value |

Long requirements are written one pattern per line, `|` leading:

```
store field foo @ 3
    requires a.x.On & b.y.On
        | c.z.Off
```

### Paths

A requirement names a state by its path, i.e. `en.Disabled`, `rcc.ahb1enr.dma1en.Enabled`.

Paths are scoped lexically. A path's first name is looked up outward from the item the path belongs
to: among the item's siblings, then its parent's siblings, and so on up to the device, or to the
boundary of the [template](#templates) the path is written in. The nearest match wins. The rest of
the path descends from what the first name found. `device` roots a path explicitly:
`device.rcc.ahb1enr.dma1en.Enabled`.

A template's paths resolve within the template, so in each instance they name that instance's
items. Requirements on what lies outside are written where the template is used:

```
peripheral dma {
    register ccr[1..=6] @ [0x8, ... by 0x14] reset 0 {
        store field tcie @ 1 assumes toggle
            write requires en.Disabled      // the sibling field `en`
    }

    register cpar[1..=6] as ch @ [0x10, ... by 0x14] reset 0 {
        store field pa32 @ 29:0
            requires ccr[#ch].psize.Bits32  // the `ccr` of the same channel
    }
}

peripheral dma1 refines dma @ 0x4002_0000 requires rcc.ahb1enr.dma1en.Enabled
```

Groups are named in paths, as they are in the generated modules.

A path names one element of an array by its designator, i.e. `ccr3`, or by a
[name expansion](#arrays). In a path, an expansion chooses which items to name, and its list is
[coupled](#iteration) with the iteration the path is within: the names it expands to are paired
with that iteration's elements. Coupling within no iteration fails validation.

The element of the same iteration is named with an [iteration variable](#iteration-variables):

```
register cpar[1..=4] as ch {
    store field pa32 requires ccr[#ch].psize.Bits32           // cpar1: ccr1, cpar2: ccr2, ...
}
```

A list names elements explicitly, for relationships that are not one-to-one:

```
register cpar[1..=4] {
    store field pa32 requires ccr[2, 1, 4, 3].psize.Bits32  // cpar1: ccr2, cpar2: ccr1, ...
}
```

Progressions and [iteration scopes](#iteration-scope) work in paths as anywhere:

```
peripheral dma[1, 2] as d refines dma {
    register ccr {
        store field en write requires dmamux.c[0..=5, ... in #d]cr.dmareq.Enabled
        // dma1.ccr1–6: c0cr–c5cr; dma2.ccr1–6: c6cr–c11cr
    }
}
```

Braces in a path are a *set*, applied wherever they are written. A set always means all of its
members; what that means depends on what they are. A set of items requires each of them, and a set
of states permits each of them (a field is in one state at a time):

```
store field ckmode @ 17:16
    write requires adc{1, 2}.cr.aden.Disabled   // adc1's and adc2's aden are both Disabled

store field mode @ 1:0
    requires cfg.sel.{A, B}                     // sel is A or B
```

In a path, a list or set may expand within a name, `gpio[a, b]`, or hold whole identifiers,
`[gpioa, gpiob]`; the two are the same.

A list's entries may be sets; a set's entries may not be lists or sets. A set of lists would
restate a list of sets, and a set of sets is one larger set.

```
peripheral adc[12, 345]_common {
    register ccr @ 0x8 {
        store field ckmode @ 17:16
            write requires adc[{1, 2}, {3..=5}].cr.aden.Disabled
            // adc12_common: adc1 and adc2; adc345_common: adc3, adc4, and adc5
    }
}
```

## Values

There are three kinds of single value:

- **literal**: an integer, i.e. `0x14`.
- **identifier**: i.e. `afsel`.
- **domain**: inclusive bits, most significant first, i.e. `15:0`. A literal `n` written where a
  domain is expected is sugar for `n:n`.

## Iteration

Brackets, and only brackets, mean iteration. They do two distinct things:

- *Expansion*: brackets within a name make names ([arrays](#arrays)).
- *Coupling*: a bracketed list's entries are paired, in order, with the elements of an iteration.

A bracketed name in a path does both: it expands to names, coupled with the iteration the path is
within.

- An unbracketed value is one value, shared by every element it applies to.
- A bracketed list is a *coupled list*: it gives one entry per element of the nearest array at or
  above what it is written on. That is the item itself if it is an array, and otherwise its nearest
  enclosing array:

```
peripheral gpio[a, b, c] refines port @ [0x4800_0000, ... by 0x400] {
    register moder reset [0xabff_ffff, 0xffff_febf, 0xffff_ffff]  // one reset per gpio
}
```

### Arrays

Brackets within a name are a *name expansion*, and make the item an array: `gpio[a, b, c]`,
`mode[0..16]`. The expansion gives each element's *designator*, i.e. `gpioa`, `mode15`. It may sit
anywhere in the name: `my[1..=3]thing` expands to `my1thing`, `my2thing`, `my3thing`.

A name is a sequence of *fragments*: text, lists, sets, and
[insertions](#iteration-variables). Adjacent fragments are never of the same kind, so a name ends
where a fragment would follow one of its own kind: `foo bar` is `foo`, then `bar`. Whitespace is
insignificant: `my [1..=3] thing` is `my[1..=3]thing`.

An expansion may also be the whole name, giving elements with no common text, i.e.
`variant [Cos, Sin] ~ [0, 1] requires scale.N0`. An array's properties, its docs included, are
shared by its elements; any element can be refined, and so documented, by its designator.

### Iteration variables

An iteration is anonymous unless named with `as`, after the name: `gpio[a, b, c] as port`. It is
referred to only through its *iteration variable*, `#port`, which stands for the entry the current
element's expansion contributed, wherever in the name that sits (`a`, `b`, `c`).

```
peripheral gpio[a, b, c] as port refines port_template @ [0x4800_0000, ... by 0x400]
    requires rcc.ahb2enr.gpio[#port]en.Enabled      // gpioa: gpioaen, gpiob: gpioben, ...
```

A variable is substituted, never computed. It is visible within its array, the nearest winning,
and never beyond the boundary of a [template](#templates).

A variable appears in only two places: *inserted* into a name in a path, alone within brackets
(`gpio[#port]en`), and after `in`. An insertion is not a list: `[#port, ...]` and `[0, #port]` are
errors. A declaration's name never holds one, as its expansions create elements, where a variable
refers to existing ones.

### Lists

A list is a comma-separated sequence of entries. Each entry is one of:

1. A **value**, which contributes itself: `[a, b]`, `[3:0, 7:4]`.
2. A **range**, which contributes literals, and only literals: `0..16` (exclusive) or `0..=15`
   (inclusive), with an optional step, i.e. `4..=60 by 4`.
3. A **nested list**, a list within the list: `[[0, 2..=7], ...]`.
4. A **progression**, `...`, which may only be the last entry.

A nested list is only meaningful before a progression; elsewhere it is the same as writing its
entries out (`[[0, 2, 5]]` is `[0, 2, 5]`), so it fails validation.

### Count

A list with a progression is *open*; any other list is *closed*. An array's count comes from the
closed lists that pair with it: there must be at least one, and they must all agree. Open lists
stop where the closed lists do.

A template may leave the count open, so long as it is closed once unified. An open list unifies
with a closed list that follows it:

```
peripheral dma { register ccr[1, ...] @ [0x8, ... by 0x14] { ... } }  // count left open
peripheral dma1 refines dma { register ccr[1..=6] }                  // six channels
peripheral dma2 refines dma { register ccr[1..=8] }                  // eight channels
```

### Progression

A progression continues the entry before it. A progression after an identifier fails validation,
as an identifier has no next.

**After a value**, values continue, each the *immediate next* of the previous:

| context | immediate next |
|---|---|
| peripheral address | none: `by` is required |
| register offset | the next register |
| field domain | the adjacent bits |
| discriminant, numbering, interrupt position | the next integer |
| reset | the same value |
| group | the adjacent group, after the extent of the previous |

**After a range or a nested list**, that sequence is the entries for one element of the *iteration
scope*, and each next element repeats it, starting right after the previous ends:

```
field afsel[0..=7]               // restarts in each register: afsel0–7 in both afrl and afrh
field afsel[0..=7, ...]          // afrl: afsel0–7; afrh: afsel8–15
field afsel[0..=7, ... by 16]    // afrl: afsel0–7; afrh: afsel16–23
field afsel[[0, 2..=7], ...]     // afrl: afsel0, afsel2–7; afrh: afsel8, afsel10–15
```

`by` overrides the distance between successive values, or between successive sequences' starts,
and may be negative: `[0x8, ... by 0x14]`, `[1:0, ... by 4]`.

### Iteration scope

The iteration scope of a progression after a range or a nested list is the nearest enclosing
array. `in`, which may only follow such a progression, names another enclosing iteration by its
variable:

```
peripheral gpio[a, b] as port {
    register afr[l, h] {
        field afsel[0..=7, ... in #port]  // continues through every afr of every gpio
    }
}
```

The elements between the scope and the item are taken outermost first: `gpioa.afrl`,
`gpioa.afrh`, `gpiob.afrl`, ...

## Interrupts

A device's interrupts are listed in its `interrupts` table. Each interrupt is an item whose
placement is its vector position. A position no interrupt occupies is reserved. Two interrupts at
the same position fail validation, as do two registers at the same offset.

Interrupts unify by name like any other children, so a device refines its template's table by
adding the interrupts the template leaves out:

```
device g4_common {
    interrupts {
        /// Window Watchdog
        wwdg @ 0
        pvd_pvm @ 1
        exti[0..=4] @ [6, ...]
        dma1_ch[1..=6] @ [11, ...]
    }
}

device g4x4 refines g4_common {
    interrupts {
        dma1_ch7 @ 17
        adc[3, 4, 5] @ [47, 61, 62]
    }
}
```

## Future

- Interrupts owned by the peripherals that raise them, should the model come to distinguish them.
- Parameterized templates ("factories"), i.e. one DMA template given its channel count. Deliberately
  avoided for now: it would bring functions into a declarative language.
- Iteration variables in doc comments, so that an array's elements may be documented apart.

## Open questions

None at present.
