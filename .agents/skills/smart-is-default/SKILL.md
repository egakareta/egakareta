---
name: smart-is-default
description: Use the `smart-default` and `smart-is-default` Rust crates to derive custom `Default` impls and per-field `is_default__<field>` helpers — paired with `serde`'s `skip_serializing_if` to omit default-valued fields from JSON. Load when writing or reviewing code that combines `#[derive(SmartDefault)]`, `#[derive(SmartIsDefault)]`, the `#[smart_is_default(...)]` container/field attributes, the auto-`Serialize` flag, or skipped/private helpers.
---

# smart-default + smart-is-default

`smart-default` generates a `Default` impl that respects per-field `#[default(...)]` annotations. `smart-is-default` generates per-field `is_default__<field>` helpers and an optional auto-`Serialize` impl that omits fields equal to `Self::default()`. Used together, you get clean round-trips where `{}` round-trips back to a fully default instance.

## Quick Start

Add the dependencies, derive both macros, and wire `skip_serializing_if` per field (or use the auto-`Serialize` flag):

```rust
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use smart_is_default::SmartIsDefault;

#[derive(SmartDefault, SmartIsDefault, Serialize, Deserialize, PartialEq, Debug)]
#[serde(default)]
struct Config {
    #[default = 12]
    #[serde(skip_serializing_if = "Config::is_default__a")]
    a: i32,

    #[default("four".to_string())]
    #[serde(skip_serializing_if = "Config::is_default__e")]
    e: String,
}

fn main() {
    // Fully default -> `{}`
    assert_eq!(serde_json::to_string(&Config::default()).unwrap(), "{}");
    // `a` changed -> only `a` is emitted.
    let cfg = Config { a: 99, ..Config::default() };
    assert_eq!(serde_json::to_string(&cfg).unwrap(), r#"{"a":99}"#);
    // Round-trip restores the smart default.
    let back: Config = serde_json::from_str("{}").unwrap();
    assert_eq!(back, Config::default());
}
```

If you don't want to hand-write `skip_serializing_if` on every field, opt into the auto-`Serialize` mode:

```rust
#[derive(SmartDefault, SmartIsDefault, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
#[smart_is_default(serde)] // <-- auto-generate a skipping Serialize impl
struct Config { /* ... */ }
```

---

## Core Concepts

### What `smart-default` does

`SmartDefault` is not a real trait — deriving it `impl Default` for you, honoring field-level `#[default(...)]` annotations:

| Form                                | Meaning                                                                                                 |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `#[default = expr]`                 | Literal/expression default. Works for primitives, `&str`, `String`, `Vec::new()`, `None`, etc.          |
| `#[default(expr)]`                  | Parens form for expressions the `= ...` form can't parse (e.g. `#[default(Some(Default::default()))]`). |
| `#[default = "code"]`               | Stringified code for tokens the parser rejects (e.g. `#[default(_code = "vec![1, 2, 3]")]`).            |
| `#[default]` (no value, on variant) | Marks the default enum variant. Exactly one variant may carry it.                                       |
| _no attribute_                      | Falls back to `<FieldType as Default>::default()`.                                                      |

### What `smart-is-default` does

For a `struct Foo { a: T, e: U }`, `#[derive(SmartIsDefault)]` emits one method per field plus an optional whole-struct method:

```rust
impl Foo {
    fn is_default__a(v: &i32) -> bool { v == &Self::default().a }
    fn is_default__e(v: &String) -> bool { v == &Self::default().e }
    fn is_default(&self) -> bool
    where
        Self: PartialEq,
    { self == &Self::default() }
}
```

- **Named fields** → helpers named `is_default__<field>`.
- **Tuple structs** → helpers named `is_default__<index>` (`is_default__0`, `is_default__1`, ...).
- **Unit structs / enums** → only the whole-struct `is_default` (or `is_default` is suppressed if you ask).
- Helpers compare **by value** (`==` against `Self::default().<field>`), not by identity.

### Container attributes

| Attribute                            | Effect                                                                                                                                        |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `#[smart_is_default(no_is_default)]` | Suppress the whole-struct `is_default` so you can hand-write your own.                                                                        |
| `#[smart_is_default(serde)]`         | Auto-generate a `serde::Serialize` impl that skips fields equal to `Self::default()`. See [Common Patterns](#auto-serialize-with-serde-flag). |
| `#[smart_is_default(no_pub)]`        | Emit per-field helpers without `pub`, keeping them module-private. Default is `pub`.                                                          |

### Field attributes

| Attribute                   | Effect                                                                                    |
| --------------------------- | ----------------------------------------------------------------------------------------- |
| `#[smart_is_default(skip)]` | Suppress the per-field helper. In the auto-`Serialize` impl, the field is always emitted. |

### Required trait bounds

- Per-field helpers: `Self` must be constructible via `Self::default()` (i.e. `SmartDefault` must be derived, or `Default` must be impl'd).
- The whole-struct `is_default` method requires `Self: PartialEq` (added as a `where Self: PartialEq` bound on the method).
- Generics: every generic parameter that appears in a field type must have a `Default + PartialEq` bound declared on the type, e.g. `struct Pair<T: Default + PartialEq> { ... }`. The derive does **not** add bounds for you.

---

## Common Patterns

### Auto-Serialize with `serde` flag

Skip writing `#[serde(skip_serializing_if = "...")]` on every field. Add `#[smart_is_default(serde)]` to the container and the macro emits a `Serialize` impl that computes the field count at runtime and emits only fields that differ from `Self::default()`:

```rust
#[derive(SmartDefault, SmartIsDefault, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
#[smart_is_default(serde)]
struct Config {
    #[default = 12] a: i32,
    #[default("four".to_string())] e: String,
    #[default = true] flag: bool,
    // No `#[default]` -> uses `<bool as Default>::default()` = false.
    false_flag: bool,
    opt: Option<String>,
    #[default(Vec::new())] list: Vec<i32>,
}

assert_eq!(serde_json::to_string(&Config::default()).unwrap(), "{}");
let cfg = Config { a: 99, ..Config::default() };
assert_eq!(serde_json::to_string(&cfg).unwrap(), r#"{"a":99}"#);
```

Behavior notes for the `serde` flag:

- **Named structs**: uses `serialize_struct` with a runtime-computed length; fields are emitted in declared order. Values that compare equal to the default are skipped (value-based, not identity-based).
- **Tuple structs**: emits a plain `serialize_tuple_struct` impl. Skipping tuple elements would break deserialization, so the flag is effectively a no-op for skipping on tuples — but you still get a `Serialize` impl.
- **Unit structs / enums / unions**: no `Serialize` impl is generated.
- Fields marked `#[smart_is_default(skip)]` are always emitted (no helper to consult).

### Round-tripping with `#[serde(default)]`

Pair the derive with `#[serde(default)]` on the container so `{}` deserializes to the smart default:

```rust
#[derive(SmartDefault, SmartIsDefault, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct Config { /* ... */ }

let back: Config = serde_json::from_str("{}").unwrap();
assert_eq!(back, Config::default());
```

If you put `#[serde(default)]` on the **field** instead of the container, that field falls back to `<FieldType as Default>::default()` — not the smart default. See [Gotchas](#serde-default-on-fields-vs-container).

### Nested structs

Nested types work as long as each level derives `SmartDefault` and uses `is_default__<field>` (or the `serde` flag) for the skip check:

```rust
#[derive(SmartDefault, SmartIsDefault, Serialize, Deserialize, PartialEq, Debug)]
struct Inner { #[default = 5] x: i32, #[default("inner".to_string())] y: String }

#[derive(SmartDefault, SmartIsDefault, Serialize, Deserialize, PartialEq, Debug)]
struct Outer {
    #[default(Inner::default())]
    #[serde(skip_serializing_if = "Outer::is_default__inner", default)]
    inner: Inner,
    #[default = 0]
    #[serde(skip_serializing_if = "Outer::is_default__count", default)]
    count: i32,
}

assert_eq!(serde_json::to_string(&Outer::default()).unwrap(), "{}");
```

### Generics

Declare `Default + PartialEq` on the type parameters you use in fields:

```rust
#[derive(SmartDefault, SmartIsDefault, Serialize, Deserialize, PartialEq, Debug)]
struct Pair<T: Default + PartialEq> {
    #[default(T::default())]
    #[serde(skip_serializing_if = "Pair::is_default__first", default)]
    first: T,
    #[default(T::default())]
    #[serde(skip_serializing_if = "Pair::is_default__second", default)]
    second: T,
}
let p: Pair<i32> = Pair { first: 1, second: 0 };
assert_eq!(serde_json::to_string(&p).unwrap(), r#"{"first":1}"#);
```

### Enums

Per-field helpers are not generated for enums, but the whole-struct `is_default` works. Hand-write `impl Default` to choose the default variant:

```rust
#[derive(SmartIsDefault, PartialEq, Debug)]
enum Setting { Named { count: i32 }, Tuple(String), Off }

impl Default for Setting {
    fn default() -> Self { Setting::Named { count: 12 } }
}

assert!(Setting::default().is_default());
assert!(!Setting::Off.is_default());
```

### Custom whole-struct `is_default`

Suppress the generated one and provide your own with `#[smart_is_default(no_is_default)]`:

```rust
#[derive(SmartDefault, SmartIsDefault, PartialEq, Debug)]
#[smart_is_default(no_is_default)]
struct Item {
    #[default = 0] count: i32,
}
impl Item {
    fn is_default(&self) -> bool { self.count == 1 }
}
```

### Private helpers

By default, helpers are `pub` so you can reference them from a different module (e.g. on a wrapper type). Use `#[smart_is_default(no_pub)]` to keep them module-private:

```rust
#[derive(SmartDefault, SmartIsDefault, PartialEq, Debug)]
#[smart_is_default(no_pub)]
struct PrivateHelpers {
    #[default = 3] n: i32,
    #[default = true] flag: bool,
}
PrivateHelpers::is_default__n(&0); // OK inside the same module; would be a privacy error elsewhere.
```

---

## Gotchas

### `#[serde(default)]` on fields vs. container

`#[serde(default)]` on a field uses `<FieldType as Default>::default()`, **not** the smart default. To make `{}` deserialize to the full smart default, put `#[serde(default)]` on the **container**, not on each field. See the [serde test in the workspace](smartisdefault-tests/tests/serde.rs) for the contrasting behaviors.

### `#[serde(skip_serializing_if = "...")]` needs the right path

The string in `skip_serializing_if` must point at `is_default__<field>` (or `is_default__<index>` for tuple structs), and the helpers are emitted as inherent methods on the type. If you use `#[smart_is_default(no_pub)]`, the helpers won't be reachable from other modules and `skip_serializing_if` strings referring to them will fail to resolve.

### Forgetting `PartialEq` for the whole-struct `is_default`

The whole-struct `is_default` is a method on the type with a `where Self: PartialEq` bound. You must derive `PartialEq` (or implement it manually) for the method to be callable. Per-field helpers work without `PartialEq` on `Self`, only on the field types.

### Generic types need explicit bounds

The derive does not add `T: Default` or `T: PartialEq` bounds. If a field is generic, declare them on the type:

```rust
// Wrong — will fail to compile.
struct Pair<T> {
    #[default(T::default())] first: T,
}

// Right.
struct Pair<T: Default + PartialEq> {
    #[default(T::default())] first: T,
}
```

### Tuple structs can't skip fields in JSON

`#[smart_is_default(serde)]` on a tuple struct still emits a plain `Serialize` impl — it cannot skip tuple elements because that would shift indices and break deserialization. Use named-field structs if you need per-field skip behavior.

### The `serde` flag generates its own `Serialize` impl

Don't combine `#[smart_is_default(serde)]` with `#[derive(Serialize)]` on the same type — you'll get a duplicate `impl Serialize` error. Either use the flag (and derive nothing else for `Serialize`) or skip the flag and write `#[derive(Serialize)]` plus per-field `skip_serializing_if`.

### Value comparison, not identity

Helpers compare with `==`, so `is_default__a(&12)` returns `true` even though `12` is a fresh value, not the same instance the smart default would produce. That's the intended behavior, but it means a custom `PartialEq` impl overrides the natural semantics.

### `#[smart_is_default(skip)]` removes the helper

You can hand-write a same-named helper in the same module (the derive won't emit it). In `#[smart_is_default(serde)]` mode, skipped fields are always emitted — no skip check is performed. Use this when a field is cheap or its presence carries meaning.

### `#[default(_code = "...")]` for unparseable expressions

Anything containing a `!` (macro calls) or other tokens the parser rejects must be written as a string: `#[default(_code = "vec![1, 2, 3]")]`. The `=` form only handles things the macro's expression parser accepts.

### The `is_default` method shadowing

If you add `#[smart_is_default(no_is_default)]` and forget to add your own `is_default` method, calls like `cfg.is_default()` won't compile (no method, no trait to fall back on). The derive won't warn about the missing manual impl.

### SmartDefault is not a trait

Don't try to add `SmartDefault` to a trait bound list — deriving it directly generates `impl Default`. For bounds, use `Default`.
