# SemVer Compatibility

This chapter provides details on what is conventionally considered a
compatible or breaking SemVer change for new releases of a package. See the
[SemVer compatibility] section for details on what SemVer is, and how Cargo
uses it to ensure compatibility of libraries.

These are only *guidelines*, and not necessarily hard-and-fast rules that all
projects will obey. The [Change categories] section details how this guide
classifies the level and severity of a change. Most of this guide focuses on
changes that will cause `cargo` and `rustc` to fail to build something that
previously worked. Almost every change carries some risk that it will
negatively affect the runtime behavior, and for those cases it is usually a
judgment call by the project maintainers whether or not it is a
SemVer-incompatible change.

See also [rust-semverver], which is an experimental tool that attempts to
programmatically check compatibility rules.

[Change categories]: #change-categories
[rust-semverver]: https://github.com/rust-lang/rust-semverver
[SemVer compatibility]: resolver.md#semver-compatibility

## Change categories

All of the policies listed below are categorized by the level of change:

* **Major change**: a change that requires a major SemVer bump.
* **Minor change**: a change that requires only a minor SemVer bump.
* **Possibly-breaking change**: a change that some projects may consider major
  and others consider minor.

The "Possibly-breaking" category covers changes that have the *potential* to
break during an update, but may not necessarily cause a breakage. The impact
of these changes should be considered carefully. The exact nature will depend
on the change and the principles of the project maintainers.

Some projects may choose to only bump the patch number on a minor change. It
is encouraged to follow the SemVer spec, and only apply bug fixes in patch
releases. However, a bug fix may require an API change that is marked as a
"minor change", and shouldn't affect compatibility. This guide does not take a
stance on how each individual "minor change" should be treated, as the
difference between minor and patch changes are conventions that depend on the
nature of the change.

Some changes are marked as "minor", even though they carry the potential risk
of breaking a build. This is for situations where the potential is extremely
low, and the potentially breaking code is unlikely to be written in idiomatic
Rust, or is specifically discouraged from use.

This guide uses the terms "major" and "minor" assuming this relates to a
"1.0.0" release or later. Initial development releases starting with "0.y.z"
can treat changes in "y" as a major release, and "z" as a minor release.
"0.0.z" releases are always major changes. This is because Cargo uses the
convention that only changes in the left-most non-zero component are
considered incompatible.

* API compatibility
    * Items
        * [Major: renaming/moving/removing any public items](#item-remove)
        * [Minor: adding new public items](#item-new)
    * Types
        * [Major: Changing the alignment, layout, or size of a well-defined type](#type-layout)
    * Structs
        * [Major: adding a private struct field when all current fields are public](#struct-add-private-field-when-public)
        * [Major: adding a public field when no private field exists](#struct-add-public-field-when-no-private)
        * [Minor: adding or removing private fields when at least one already exists](#struct-private-fields-with-private)
        * [Minor: going from a tuple struct with all private fields (with at least one field) to a normal struct, or vice versa](#struct-tuple-normal-with-private)
    * Enums
        * [Major: adding new enum variants (without `non_exhaustive`)](#enum-variant-new)
        * [Major: adding new fields to an enum variant](#enum-fields-new)
    * Traits
        * [Major: adding a non-defaulted trait item](#trait-new-item-no-default)
        * [Major: any change to trait item signatures](#trait-item-signature)
        * [Possibly-breaking: adding a defaulted trait item](#trait-new-default-item)
        * [Major: adding a trait item that makes the trait non-object safe](#trait-object-safety)
        * [Major: adding a type parameter without a default](#trait-new-parameter-no-default)
        * [Minor: adding a defaulted trait type parameter](#trait-new-parameter-default)
    * Implementations
        * [Possibly-breaking change: adding any inherent items](#impl-item-new)
    * Generics
        * [Major: tightening generic bounds](#generic-bounds-tighten)
        * [Minor: loosening generic bounds](#generic-bounds-loosen)
        * [Minor: adding defaulted type parameters](#generic-new-default)
        * [Minor: generalizing a type to use generics (with identical types)](#generic-generalize-identical)
        * [Major: generalizing a type to use generics (with possibly different types)](#generic-generalize-different)
        * [Minor: changing a generic type to a more generic type](#generic-more-generic)
    * Functions
        * [Major: adding/removing function parameters](#fn-change-arity)
        * [Possibly-breaking: introducing a new function type parameter](#fn-generic-new)
        * [Minor: generalizing a function to use generics (supporting original type)](#fn-generalize-compatible)
        * [Major: generalizing a function to use generics with type mismatch](#fn-generalize-mismatch)
        * [Minor: making an `unsafe` function safe](#fn-unsafe-safe)
    * Attributes
        * [Major: switching from `no_std` support to requiring `std`](#attr-no-std-to-std)
        * [Major: adding `non_exhaustive` to an existing enum, variant, or struct with no private fields](#attr-adding-non-exhaustive)
* Tooling and environment compatibility
    * [Possibly-breaking: changing the minimum version of Rust required](#env-new-rust)
    * [Possibly-breaking: changing the platform and environment requirements](#env-change-requirements)
    * [Minor: introducing new lints](#new-lints)
    * Cargo
        * [Minor: adding a new Cargo feature](#cargo-feature-add)
        * [Major: removing a Cargo feature](#cargo-feature-remove)
        * [Major: removing a feature from a feature list if that changes functionality or public items](#cargo-feature-remove-another)
        * [Possibly-breaking: removing an optional dependency](#cargo-remove-opt-dep)
        * [Minor: changing dependency features](#cargo-change-dep-feature)
        * [Minor: adding dependencies](#cargo-dep-add)
* [Application compatibility](#application-compatibility)

## API compatibility

All of the examples below contain three parts: the original code, the code
after it has been modified, and an example usage of the code that could appear
in another project. In a minor change, the example usage should successfully
build with both the before and after versions.

### Major: renaming/moving/removing any public items {#item-remove}

The absence of a publicly exposed [item][items] will cause any uses of that item to
fail to compile.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub fn foo() {}

///////////////////////////////////////////////////////////
// After
// ... item has been removed

///////////////////////////////////////////////////////////
// Example usage that will break.
fn main() {
    updated_crate::foo(); // Error: cannot find function `foo`
}
```

This includes adding any sort of [`cfg` attribute] which can change which
items or behavior is available based on [conditional compilation].

Mitigating strategies:
* Mark items to be removed as [deprecated], and then remove them at a later
  date in a SemVer-breaking release.
* Mark renamed items as [deprecated], and use a [`pub use`] item to re-export
  to the old name.

### Minor: adding new public items {#item-new}

Adding new, public [items] is a minor change.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
// ... absence of item

///////////////////////////////////////////////////////////
// After
pub fn foo() {}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
// `foo` is not used since it didn't previously exist.
```

Note that in some rare cases this can be a **breaking change** due to glob
imports. For example, if you add a new trait, and a project has used a glob
import that brings that trait into scope, and the new trait introduces an
associated item that conflicts with any types it is implemented on, this can
cause a compile-time error due to the ambiguity. Example:

```rust,ignore
// Breaking change example

///////////////////////////////////////////////////////////
// Before
// ... absence of trait

///////////////////////////////////////////////////////////
// After
pub trait NewTrait {
    fn foo(&self) {}
}

impl NewTrait for i32 {}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::*;

pub trait LocalTrait {
    fn foo(&self) {}
}

impl LocalTrait for i32 {}

fn main() {
    123i32.foo(); // Error:  multiple applicable items in scope
}
```

This is not considered a major change because conventionally glob imports are
a known forwards-compatibility hazard. Glob imports of items from external
crates should be avoided.

### Major: Changing the alignment, layout, or size of a well-defined type {#type-layout}

It is a breaking change to change the alignment, layout, or size of a type that was previously well-defined.

In general, types that use the [the default representation] do not have a well-defined alignment, layout, or size.
The compiler is free to alter the alignment, layout, or size, so code should not make any assumptions about it.

> **Note**: It may be possible for external crates to break if they make assumptions about the alignment, layout, or size of a type even if it is not well-defined.
> This is not considered a SemVer breaking change since those assumptions should not be made.

Some examples of changes that are not a breaking change are (assuming no other rules in this guide are violated):

* Adding, removing, reordering, or changing fields of a default representation struct, union, or enum in such a way that the change follows the other rules in this guide (for example, using `non_exhaustive` to allow those changes, or changes to private fields that are already private).
  See [struct-add-private-field-when-public](#struct-add-private-field-when-public), [struct-add-public-field-when-no-private](#struct-add-public-field-when-no-private), [struct-private-fields-with-private](#struct-private-fields-with-private), [enum-fields-new](#enum-fields-new).
* Adding variants to a default representation enum, if the enum uses `non_exhaustive`.
  This may change the alignment or size of the enumeration, but those are not well-defined.
  See [enum-variant-new](#enum-variant-new).
* Adding, removing, reordering, or changing private fields of a `repr(C)` struct, union, or enum, following the other rules in this guide (for example, using `non_exhaustive`, or adding private fields when other private fields already exist).
  See [repr-c-private-change](#repr-c-private-change).
* Adding variants to a `repr(C)` enum, if the enum uses `non_exhaustive`.
  See [repr-c-enum-variant-new](#repr-c-enum-variant-new).
* Adding `repr(C)` to a default representation struct, union, or enum.
  See [repr-c-add](#repr-c-add).
* Adding `repr(<int>)` [primitive representation] to an enum.
  See [repr-int-enum-add](#repr-int-enum-add).
* Adding `repr(transparent)` to a default representation struct or enum.
  See [repr-transparent-add](#repr-transparent-add).

Types that use the [`repr` attribute] can be said to have an alignment and layout that is defined in some way that code may make some assumptions about that may break as a result of changing that type.

In some cases, types with a `repr` attribute may not have an alignment, layout, or size that is well-defined.
In these cases, it may be safe to make changes to the types, though care should be exercised.
For example, types with private fields that do not otherwise document their alignment, layout, or size guarantees cannot be relied upon by external crates since the public API does not fully define the alignment, layout, or size of the type.

A common example where a type with *private* fields is well-defined is a type with a single private field with a generic type, using `repr(transparent)`,
and the prose of the documentation discusses that it is transparent to the generic type.
For example, see [`UnsafeCell`].

Some examples of breaking changes are:

* Adding `repr(packed)` to a struct or union.
  See [repr-packed-add](#repr-packed-add).
* Adding `repr(align)` to a struct, union, or enum.
  See [repr-align-add](#repr-align-add).
* Removing `repr(packed)` from a struct or union.
  See [repr-packed-remove](#repr-packed-remove).
* Changing the value N of `repr(packed(N))` if that changes the alignment or layout.
  See [repr-packed-n-change](#repr-packed-n-change).
* Changing the value N of `repr(align(N))` if that changes the alignment.
  See [repr-align-n-change](#repr-align-n-change).
* Removing `repr(align)` from a struct, union, or enum.
  See [repr-align-remove](#repr-align-remove).
* Changing the order of public fields of a `repr(C)` type.
  See [repr-c-shuffle](#repr-c-shuffle).
* Removing `repr(C)` from a struct, union, or enum.
  See [repr-c-remove](#repr-c-remove).
* Removing `repr(<int>)` from an enum.
  See [repr-int-enum-remove](#repr-int-enum-remove).
* Changing the primitive representation of a `repr(<int>)` enum.
  See [repr-int-enum-change](#repr-int-enum-change).
* Removing `repr(transparent)` from a struct or enum.
  See [repr-transparent-remove](#repr-transparent-remove).

[the default representation]: ../../reference/type-layout.html#the-default-representation
[primitive representation]: ../../reference/type-layout.html#primitive-representations
[`repr` attribute]: ../../reference/type-layout.html#representations
[`std::mem::transmute`]: ../../std/mem/fn.transmute.html
[`UnsafeCell`]: ../../std/cell/struct.UnsafeCell.html#memory-layout

#### Minor: `repr(C)` add, remove, or change a private field {#repr-c-private-change}

It is usually safe to add, remove, or change a private field of a `repr(C)` struct, union, or enum, assuming it follows the other guidelines in this guide (see [struct-add-private-field-when-public](#struct-add-private-field-when-public), [struct-add-public-field-when-no-private](#struct-add-public-field-when-no-private), [struct-private-fields-with-private](#struct-private-fields-with-private), [enum-fields-new](#enum-fields-new)).

For example, adding private fields can only be done if there are already other private fields, or it is `non_exhaustive`.
Public fields may be added if there are private fields, or it is `non_exhaustive`, and the addition does not alter the layout of the other fields.

However, this may change the size and alignment of the type.
Care should be taken if the size or alignment changes.
Code should not make assumptions about the size or alignment of types with private fields or `non_exhaustive` unless it has a documented size or alignment.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[derive(Default)]
#[repr(C)]
pub struct Example {
    pub f1: i32,
    f2: i32, // a private field
}

///////////////////////////////////////////////////////////
// After
#[derive(Default)]
#[repr(C)]
pub struct Example {
    pub f1: i32,
    f2: i32,
    f3: i32, // a new field
}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
fn main() {
    // NOTE: Users should not make assumptions about the size or alignment
    // since they are not documented.
    let f = updated_crate::Example::default();
}
```

#### Minor: `repr(C)` add enum variant {#repr-c-enum-variant-new}

It is usually safe to add variants to a `repr(C)` enum, if the enum uses `non_exhastive`.
See [enum-variant-new](#enum-variant-new) for more discussion.

Note that this may be a breaking change since it changes the size and alignment of the type.
See [repr-c-private-change](#repr-c-private-change) for similar concerns.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(C)]
#[non_exhaustive]
pub enum Example {
    Variant1 { f1: i16 },
    Variant2 { f1: i32 },
}

///////////////////////////////////////////////////////////
// After
#[repr(C)]
#[non_exhaustive]
pub enum Example {
    Variant1 { f1: i16 },
    Variant2 { f1: i32 },
    Variant3 { f1: i64 }, // added
}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
fn main() {
    // NOTE: Users should not make assumptions about the size or alignment
    // since they are not specified. For example, this raised the size from 8
    // to 16 bytes.
    let f = updated_crate::Example::Variant2 { f1: 123 };
}
```

#### Minor: Adding `repr(C)` to a default representation {#repr-c-add}

It is safe to add `repr(C)` to a struct, union, or enum with [the default representation].
This is safe because users should not make assumptions about the alignment, layout, or size of types with with the default representation.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Example {
    pub f1: i32,
    pub f2: i16,
}

///////////////////////////////////////////////////////////
// After
#[repr(C)] // added
pub struct Example {
    pub f1: i32,
    pub f2: i16,
}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
fn main() {
    let f = updated_crate::Example { f1: 123, f2: 456 };
}
```

#### Minor: Adding `repr(<int>)` to an enum {#repr-int-enum-add}

It is safe to add `repr(<int>)` [primitive representation] to an enum with [the default representation].
This is safe because users should not make assumptions about the alignment, layout, or size of an enum with the default representation.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub enum E {
    Variant1,
    Variant2(i32),
    Variant3 { f1: f64 },
}

///////////////////////////////////////////////////////////
// After
#[repr(i32)] // added
pub enum E {
    Variant1,
    Variant2(i32),
    Variant3 { f1: f64 },
}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
fn main() {
    let x = updated_crate::E::Variant3 { f1: 1.23 };
}
```

#### Minor: Adding `repr(transparent)` to a default representation struct or enum {#repr-transparent-add}

It is safe to add `repr(transparent)` to a struct or enum with [the default representation].
This is safe because users should not make assumptions about the alignment, layout, or size of a struct or enum with the default representation.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[derive(Default)]
pub struct Example<T>(T);

///////////////////////////////////////////////////////////
// After
#[derive(Default)]
#[repr(transparent)] // added
pub struct Example<T>(T);

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
fn main() {
    let x = updated_crate::Example::<i32>::default();
}
```

#### Major: Adding `repr(packed)` to a struct or union {#repr-packed-add}

It is a breaking change to add `repr(packed)` to a struct or union.
Making a type `repr(packed)` makes changes that can break code, such as being invalid to take a reference to a field, or causing truncation of disjoint closure captures.

<!-- TODO: If all fields are private, should this be safe to do? -->

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Example {
    pub f1: u8,
    pub f2: u16,
}

///////////////////////////////////////////////////////////
// After
#[repr(packed)] // added
pub struct Example {
    pub f1: u8,
    pub f2: u16,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
fn main() {
    let f = updated_crate::Example { f1: 1, f2: 2 };
    let x = &f.f2; // Error: reference to packed field is unaligned
}
```

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Example(pub i32, pub i32);

///////////////////////////////////////////////////////////
// After
#[repr(packed)]
pub struct Example(pub i32, pub i32);

///////////////////////////////////////////////////////////
// Example usage that will break.
fn main() {
    let mut f = updated_crate::Example(123, 456);
    let c = || {
        // Without repr(packed), the closure precisely captures `&f.0`.
        // With repr(packed), the closure captures `&f` to avoid undefined behavior.
        let a = f.0;
    };
    f.1 = 789; // Error: cannot assign to `f.1` because it is borrowed
    c();
}
```

#### Major: Adding `repr(align)` to a struct, union, or enum {#repr-align-add}

It is a breaking change to add `repr(align)` to a struct, union, or enum.
Making a type `repr(align)` would break any use of that type in a `repr(packed)` type because that combination is not allowed.

<!-- TODO: This seems like it should be extraordinarily rare. Should there be any exceptions carved out for this? -->

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Aligned {
    pub a: i32,
}

///////////////////////////////////////////////////////////
// After
#[repr(align(8))] // added
pub struct Aligned {
    pub a: i32,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Aligned;

#[repr(packed)]
pub struct Packed { // Error: packed type cannot transitively contain a `#[repr(align)]` type
    f1: Aligned,
}

fn main() {
    let p = Packed {
        f1: Aligned { a: 123 },
    };
}
```

#### Major: Removing `repr(packed)` from a struct or union {#repr-packed-remove}

It is a breaking change to remove `repr(packed)` from a struct or union.
This may change the alignment or layout that extern crates are relying on.

If any fields are public, then removing `repr(packed)` may change the way disjoint closure captures work.
In some cases, this can cause code to break, similar to those outlined in the [edition guide][edition-closures].

[edition-closures]: ../../edition-guide/rust-2021/disjoint-capture-in-closures.html

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(C, packed)]
pub struct Packed {
    pub a: u8,
    pub b: u16,
}

///////////////////////////////////////////////////////////
// After
#[repr(C)] // removed packed
pub struct Packed {
    pub a: u8,
    pub b: u16,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Packed;

fn main() {
    let p = Packed { a: 1, b: 2 };
    // Some assumption about the size of the type.
    // Without `packed`, this fails since the size is 4.
    const _: () = assert!(std::mem::size_of::<Packed>() == 3); // Error: evaluation of constant value failed
}
```

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(C, packed)]
pub struct Packed {
    pub a: *mut i32,
    pub b: i32,
}
unsafe impl Send for Packed {}

///////////////////////////////////////////////////////////
// After
#[repr(C)] // removed packed
pub struct Packed {
    pub a: *mut i32,
    pub b: i32,
}
unsafe impl Send for Packed {}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Packed;

fn main() {
    let mut x = 123;

    let p = Packed {
        a: &mut x as *mut i32,
        b: 456,
    };

    // When the structure was packed, the closure captures `p` which is Send.
    // When `packed` is removed, this ends up capturing `p.a` which is not Send.
    std::thread::spawn(move || unsafe {
        *(p.a) += 1; // Error: cannot be sent between threads safely
    });
}
```

#### Major: Changing the value N of `repr(packed(N))` if that changes the alignment or layout {#repr-packed-n-change}

It is a breaking change to change the value of N of `repr(packed(N))` if that changes the alignment or layout.
This may change the alignment or layout that external crates are relying on.

If the value `N` is lowered below the alignment of a public field, then that would break any code that attempts to take a reference of that field.

Note that some changes to `N` may not change the alignment or layout, for example increasing it when the current value is already equal to the natural alignment of the type.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(packed(4))]
pub struct Packed {
    pub a: u8,
    pub b: u32,
}

///////////////////////////////////////////////////////////
// After
#[repr(packed(2))] // changed to 2
pub struct Packed {
    pub a: u8,
    pub b: u32,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Packed;

fn main() {
    let p = Packed { a: 1, b: 2 };
    let x = &p.b; // Error: reference to packed field is unaligned
}
```

#### Major: Changing the value N of `repr(align(N))` if that changes the alignment {#repr-align-n-change}

It is a breaking change to change the value `N` of `repr(align(N))` if that changes the alignment.
This may change the alignment that external crates are relying on.

This change should be safe to make if the type is not well-defined as discussed in [type layout](#type-layout) (such as having any private fields and having an undocumented alignment or layout).

Note that some changes to `N` may not change the alignment or layout, for example decreasing it when the current value is already equal to or less than the natural alignment of the type.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(align(8))]
pub struct Packed {
    pub a: u8,
    pub b: u32,
}

///////////////////////////////////////////////////////////
// After
#[repr(align(4))] // changed to 4
pub struct Packed {
    pub a: u8,
    pub b: u32,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Packed;

fn main() {
    let p = Packed { a: 1, b: 2 };
    // Some assumption about the size of the type.
    // The alignment has changed from 8 to 4.
    const _: () = assert!(std::mem::align_of::<Packed>() == 8); // Error: evaluation of constant value failed
}
```

#### Major: Removing `repr(align)` from a struct, union, or enum {#repr-align-remove}

It is a breaking change to remove `repr(align)` from a struct, union, or enum, if their layout was well-defined.
This may change the alignment or layout that external crates are relying on.

This change should be safe to make if the type is not well-defined as discussed in [type layout](#type-layout) (such as having any private fields and having an undocumented alignment).

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(C, align(8))]
pub struct Packed {
    pub a: u8,
    pub b: u32,
}

///////////////////////////////////////////////////////////
// After
#[repr(C)] // removed align
pub struct Packed {
    pub a: u8,
    pub b: u32,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Packed;

fn main() {
    let p = Packed { a: 1, b: 2 };
    // Some assumption about the size of the type.
    // The alignment has changed from 8 to 4.
    const _: () = assert!(std::mem::align_of::<Packed>() == 8); // Error: evaluation of constant value failed
}
```

#### Major: Changing the order of public fields of a `repr(C)` type {#repr-c-shuffle}

It is a breaking change to change the order of public fields of a `repr(C)` type.
External crates may be relying on the specific ordering of the fields.

```rust,ignore,run-fail
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(C)]
pub struct SpecificLayout {
    pub a: u8,
    pub b: u32,
}

///////////////////////////////////////////////////////////
// After
#[repr(C)]
pub struct SpecificLayout {
    pub b: u32, // changed order
    pub a: u8,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::SpecificLayout;

extern "C" {
    // This C function is assuming a specific layout defined in a C header.
    fn c_fn_get_b(x: &SpecificLayout) -> u32;
}

fn main() {
    let p = SpecificLayout { a: 1, b: 2 };
    unsafe { assert_eq!(c_fn_get_b(&p), 2) } // Error: value not equal to 2
}

# mod cdep {
#     // This simulates what would normally be something included from a build script.
#     // This definition would be in a C header.
#     #[repr(C)]
#     pub struct SpecificLayout {
#         pub a: u8,
#         pub b: u32,
#     }
#
#     #[no_mangle]
#     pub fn c_fn_get_b(x: &SpecificLayout) -> u32 {
#         x.b
#     }
# }
```

#### Major: Removing `repr(C)` from a struct, union, or enum {#repr-c-remove}

It is a breaking change to remove `repr(C)` from a struct, union, or enum.
External crates may be relying on the specific layout of the type.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(C)]
pub struct SpecificLayout {
    pub a: u8,
    pub b: u32,
}

///////////////////////////////////////////////////////////
// After
// removed repr(C)
pub struct SpecificLayout {
    pub a: u8,
    pub b: u32,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::SpecificLayout;

extern "C" {
    // This C function is assuming a specific layout defined in a C header.
    fn c_fn_get_b(x: &SpecificLayout) -> u32; // Error: is not FFI-safe
}

fn main() {
    let p = SpecificLayout { a: 1, b: 2 };
    unsafe { assert_eq!(c_fn_get_b(&p), 2) }
}

# mod cdep {
#     // This simulates what would normally be something included from a build script.
#     // This definition would be in a C header.
#     #[repr(C)]
#     pub struct SpecificLayout {
#         pub a: u8,
#         pub b: u32,
#     }
#
#     #[no_mangle]
#     pub fn c_fn_get_b(x: &SpecificLayout) -> u32 {
#         x.b
#     }
# }
```

#### Major: Removing `repr(<int>)` from an enum {#repr-int-enum-remove}

It is a breaking change to remove `repr(<int>)` from an enum.
External crates may be assuming that the discriminant is a specific size.
For example, [`std::mem::transmute`] of an enum may fail.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(u16)]
pub enum Example {
    Variant1,
    Variant2,
    Variant3,
}

///////////////////////////////////////////////////////////
// After
// removed repr(u16)
pub enum Example {
    Variant1,
    Variant2,
    Variant3,
}

///////////////////////////////////////////////////////////
// Example usage that will break.

fn main() {
    let e = updated_crate::Example::Variant2;
    let i: u16 = unsafe { std::mem::transmute(e) }; // Error: cannot transmute between types of different sizes
}
```

#### Major: Changing the primitive representation of a `repr(<int>)` enum {#repr-int-enum-change}

It is a breaking change to change the primitive representation of a `repr(<int>)` enum.
External crates may be assuming that the discriminant is a specific size.
For example, [`std::mem::transmute`] of an enum may fail.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(u16)]
pub enum Example {
    Variant1,
    Variant2,
    Variant3,
}

///////////////////////////////////////////////////////////
// After
#[repr(u8)] // changed repr size
pub enum Example {
    Variant1,
    Variant2,
    Variant3,
}

///////////////////////////////////////////////////////////
// Example usage that will break.

fn main() {
    let e = updated_crate::Example::Variant2;
    let i: u16 = unsafe { std::mem::transmute(e) }; // Error: cannot transmute between types of different sizes
}
```

#### Major: Removing `repr(transparent)` from a struct or enum {#repr-transparent-remove}

It is a breaking change to remove `repr(transparent)` from a struct or enum.
External crates may be relying on the type having the alignment, layout, or size of the transparent field.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[repr(transparent)]
pub struct Transparent<T>(T);

///////////////////////////////////////////////////////////
// After
// removed repr
pub struct Transparent<T>(T);

///////////////////////////////////////////////////////////
// Example usage that will break.
#![deny(improper_ctypes)]
use updated_crate::Transparent;

extern "C" {
    fn c_fn() -> Transparent<f64>; // Error: is not FFI-safe
}

fn main() {}
```

### Major: adding a private struct field when all current fields are public {#struct-add-private-field-when-public}

When a private field is added to a struct that previously had all public fields,
this will break any code that attempts to construct it with a [struct literal].

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Foo {
    pub f1: i32,
}

///////////////////////////////////////////////////////////
// After
pub struct Foo {
    pub f1: i32,
    f2: i32,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
fn main() {
    let x = updated_crate::Foo { f1: 123 }; // Error: cannot construct `Foo`
}
```

Mitigation strategies:
* Do not add new fields to all-public field structs.
* Mark structs as [`#[non_exhaustive]`][non_exhaustive] when first introducing
  a struct to prevent users from using struct literal syntax, and instead
  provide a constructor method and/or [Default] implementation.

### Major: adding a public field when no private field exists {#struct-add-public-field-when-no-private}

When a public field is added to a struct that has all public fields, this will
break any code that attempts to construct it with a [struct literal].

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Foo {
    pub f1: i32,
}

///////////////////////////////////////////////////////////
// After
pub struct Foo {
    pub f1: i32,
    pub f2: i32,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
fn main() {
    let x = updated_crate::Foo { f1: 123 }; // Error: missing field `f2`
}
```

Mitigation strategies:
* Do not add new new fields to all-public field structs.
* Mark structs as [`#[non_exhaustive]`][non_exhaustive] when first introducing
  a struct to prevent users from using struct literal syntax, and instead
  provide a constructor method and/or [Default] implementation.

### Minor: adding or removing private fields when at least one already exists {#struct-private-fields-with-private}

It is safe to add or remove private fields from a struct when the struct
already has at least one private field.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[derive(Default)]
pub struct Foo {
    f1: i32,
}

///////////////////////////////////////////////////////////
// After
#[derive(Default)]
pub struct Foo {
    f2: f64,
}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
fn main() {
    // Cannot access private fields.
    let x = updated_crate::Foo::default();
}
```

This is safe because existing code cannot use a [struct literal] to construct
it, nor exhaustively match its contents.

Note that for tuple structs, this is a **major change** if the tuple contains
public fields, and the addition or removal of a private field changes the
index of any public field.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[derive(Default)]
pub struct Foo(pub i32, i32);

///////////////////////////////////////////////////////////
// After
#[derive(Default)]
pub struct Foo(f64, pub i32, i32);

///////////////////////////////////////////////////////////
// Example usage that will break.
fn main() {
    let x = updated_crate::Foo::default();
    let y = x.0; // Error: is private
}
```

### Minor: going from a tuple struct with all private fields (with at least one field) to a normal struct, or vice versa {#struct-tuple-normal-with-private}

Changing a tuple struct to a normal struct (or vice-versa) is safe if all
fields are private.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[derive(Default)]
pub struct Foo(i32);

///////////////////////////////////////////////////////////
// After
#[derive(Default)]
pub struct Foo {
    f1: i32,
}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
fn main() {
    // Cannot access private fields.
    let x = updated_crate::Foo::default();
}
```

This is safe because existing code cannot use a [struct literal] to construct
it, nor match its contents.

### Major: adding new enum variants (without `non_exhaustive`) {#enum-variant-new}

It is a breaking change to add a new enum variant if the enum does not use the
[`#[non_exhaustive]`][non_exhaustive] attribute.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub enum E {
    Variant1,
}

///////////////////////////////////////////////////////////
// After
pub enum E {
    Variant1,
    Variant2,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
fn main() {
    use updated_crate::E;
    let x = E::Variant1;
    match x { // Error: `E::Variant2` not covered
        E::Variant1 => {}
    }
}
```

Mitigation strategies:
* When introducing the enum, mark it as [`#[non_exhaustive]`][non_exhaustive]
  to force users to use [wildcard patterns] to catch new variants.

### Major: adding new fields to an enum variant {#enum-fields-new}

It is a breaking change to add new fields to an enum variant because all
fields are public, and constructors and matching will fail to compile.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub enum E {
    Variant1 { f1: i32 },
}

///////////////////////////////////////////////////////////
// After
pub enum E {
    Variant1 { f1: i32, f2: i32 },
}

///////////////////////////////////////////////////////////
// Example usage that will break.
fn main() {
    use updated_crate::E;
    let x = E::Variant1 { f1: 1 }; // Error: missing f2
    match x {
        E::Variant1 { f1 } => {} // Error: missing f2
    }
}
```

Mitigation strategies:
* When introducing the enum, mark the variant as [`non_exhaustive`][non_exhaustive]
  so that it cannot be constructed or matched without wildcards.
  ```rust,ignore,skip
  pub enum E {
      #[non_exhaustive]
      Variant1{f1: i32}
  }
  ```
* When introducing the enum, use an explicit struct as a value, where you can
  have control over the field visibility.
  ```rust,ignore,skip
  pub struct Foo {
     f1: i32,
     f2: i32,
  }
  pub enum E {
      Variant1(Foo)
  }
  ```

### Major: adding a non-defaulted trait item {#trait-new-item-no-default}

It is a breaking change to add a non-defaulted item to a trait. This will
break any implementors of the trait.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub trait Trait {}

///////////////////////////////////////////////////////////
// After
pub trait Trait {
    fn foo(&self);
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Trait;
struct Foo;

impl Trait for Foo {}  // Error: not all trait items implemented
```

Mitigation strategies:
* Always provide a default implementation or value for new associated trait
  items.
* When introducing the trait, use the [sealed trait] technique to prevent
  users outside of the crate from implementing the trait.

### Major: any change to trait item signatures {#trait-item-signature}

It is a breaking change to make any change to a trait item signature. This can
break external implementors of the trait.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub trait Trait {
    fn f(&self, x: i32) {}
}

///////////////////////////////////////////////////////////
// After
pub trait Trait {
    // For sealed traits or normal functions, this would be a minor change
    // because generalizing with generics strictly expands the possible uses.
    // But in this case, trait implementations must use the same signature.
    fn f<V>(&self, x: V) {}
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Trait;
struct Foo;

impl Trait for Foo {
    fn f(&self, x: i32) {}  // Error: trait declaration has 1 type parameter
}
```

Mitigation strategies:
* Introduce new items with default implementations to cover the new
  functionality instead of modifying existing items.
* When introducing the trait, use the [sealed trait] technique to prevent
  users outside of the crate from implementing the trait.

### Possibly-breaking: adding a defaulted trait item {#trait-new-default-item}

It is usually safe to add a defaulted trait item. However, this can sometimes
cause a compile error. For example, this can introduce an ambiguity if a
method of the same name exists in another trait.

```rust,ignore
// Breaking change example

///////////////////////////////////////////////////////////
// Before
pub trait Trait {}

///////////////////////////////////////////////////////////
// After
pub trait Trait {
    fn foo(&self) {}
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Trait;
struct Foo;

trait LocalTrait {
    fn foo(&self) {}
}

impl Trait for Foo {}
impl LocalTrait for Foo {}

fn main() {
    let x = Foo;
    x.foo(); // Error: multiple applicable items in scope
}
```

Note that this ambiguity does *not* exist for name collisions on [inherent
implementations], as they take priority over trait items.

See [trait-object-safety](#trait-object-safety) for a special case to consider
when adding trait items.

Mitigation strategies:
* Some projects may deem this acceptable breakage, particularly if the new
  item name is unlikely to collide with any existing code. Choose names
  carefully to help avoid these collisions. Additionally, it may be acceptable
  to require downstream users to add [disambiguation syntax] to select the
  correct function when updating the dependency.

### Major: adding a trait item that makes the trait non-object safe {#trait-object-safety}

It is a breaking change to add a trait item that changes the trait to not be
[object safe].

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub trait Trait {}

///////////////////////////////////////////////////////////
// After
pub trait Trait {
    // An associated const makes the trait not object-safe.
    const CONST: i32 = 123;
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Trait;
struct Foo;

impl Trait for Foo {}

fn main() {
    let obj: Box<dyn Trait> = Box::new(Foo); // Error: cannot be made into an object
}
```

It is safe to do the converse (making a non-object safe trait into a safe
one).

### Major: adding a type parameter without a default {#trait-new-parameter-no-default}

It is a breaking change to add a type parameter without a default to a trait.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub trait Trait {}

///////////////////////////////////////////////////////////
// After
pub trait Trait<T> {}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Trait;
struct Foo;

impl Trait for Foo {}  // Error: missing generics
```

Mitigating strategies:
* See [adding a defaulted trait type parameter](#trait-new-parameter-default).

### Minor: adding a defaulted trait type parameter {#trait-new-parameter-default}

It is safe to add a type parameter to a trait as long as it has a default.
External implementors will use the default without needing to specify the
parameter.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub trait Trait {}

///////////////////////////////////////////////////////////
// After
pub trait Trait<T = i32> {}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
use updated_crate::Trait;
struct Foo;

impl Trait for Foo {}
```

### Possibly-breaking change: adding any inherent items {#impl-item-new}

Usually adding inherent items to an implementation should be safe because
inherent items take priority over trait items. However, in some cases the
collision can cause problems if the name is the same as an implemented trait
item with a different signature.

```rust,ignore
// Breaking change example

///////////////////////////////////////////////////////////
// Before
pub struct Foo;

///////////////////////////////////////////////////////////
// After
pub struct Foo;

impl Foo {
    pub fn foo(&self) {}
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Foo;

trait Trait {
    fn foo(&self, x: i32) {}
}

impl Trait for Foo {}

fn main() {
    let x = Foo;
    x.foo(1); // Error: this method takes 0 arguments but 1 argument was supplied
}
```

Note that if the signatures match, there would not be a compile-time error,
but possibly a silent change in runtime behavior (because it is now executing
a different function).

Mitigation strategies:
* Some projects may deem this acceptable breakage, particularly if the new
  item name is unlikely to collide with any existing code. Choose names
  carefully to help avoid these collisions. Additionally, it may be acceptable
  to require downstream users to add [disambiguation syntax] to select the
  correct function when updating the dependency.

### Major: tightening generic bounds {#generic-bounds-tighten}

It is a breaking change to tighten generic bounds on a type since this can
break users expecting the looser bounds.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Foo<A> {
    pub f1: A,
}

///////////////////////////////////////////////////////////
// After
pub struct Foo<A: Eq> {
    pub f1: A,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Foo;

fn main() {
    let s = Foo { f1: 1.23 }; // Error: the trait bound `{float}: Eq` is not satisfied
}
```

### Minor: loosening generic bounds {#generic-bounds-loosen}

It is safe to loosen the generic bounds on a type, as it only expands what is
allowed.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Foo<A: Clone> {
    pub f1: A,
}

///////////////////////////////////////////////////////////
// After
pub struct Foo<A> {
    pub f1: A,
}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
use updated_crate::Foo;

fn main() {
    let s = Foo { f1: 123 };
}
```

### Minor: adding defaulted type parameters {#generic-new-default}

It is safe to add a type parameter to a type as long as it has a default. All
existing references will use the default without needing to specify the
parameter.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
#[derive(Default)]
pub struct Foo {}

///////////////////////////////////////////////////////////
// After
#[derive(Default)]
pub struct Foo<A = i32> {
    f1: A,
}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
use updated_crate::Foo;

fn main() {
    let s: Foo = Default::default();
}
```

### Minor: generalizing a type to use generics (with identical types) {#generic-generalize-identical}

A struct or enum field can change from a concrete type to a generic type
parameter, provided that the change results in an identical type for all
existing use cases. For example, the following change is permitted:

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Foo(pub u8);

///////////////////////////////////////////////////////////
// After
pub struct Foo<T = u8>(pub T);

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
use updated_crate::Foo;

fn main() {
    let s: Foo = Foo(123);
}
```

because existing uses of `Foo` are shorthand for `Foo<u8>` which yields the
identical field type.

### Major: generalizing a type to use generics (with possibly different types) {#generic-generalize-different}

Changing a struct or enum field from a concrete type to a generic type
parameter can break if the type can change.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Foo<T = u8>(pub T, pub u8);

///////////////////////////////////////////////////////////
// After
pub struct Foo<T = u8>(pub T, pub T);

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::Foo;

fn main() {
    let s: Foo<f32> = Foo(3.14, 123); // Error: mismatched types
}
```

### Minor: changing a generic type to a more generic type {#generic-more-generic}

It is safe to change a generic type to a more generic one. For example, the
following adds a generic parameter that defaults to the original type, which
is safe because all existing users will be using the same type for both
fields, the defaulted parameter does not need to be specified.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Foo<T>(pub T, pub T);

///////////////////////////////////////////////////////////
// After
pub struct Foo<T, U = T>(pub T, pub U);

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
use updated_crate::Foo;

fn main() {
    let s: Foo<f32> = Foo(1.0, 2.0);
}
```

### Major: adding/removing function parameters {#fn-change-arity}

Changing the arity of a function is a breaking change.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub fn foo() {}

///////////////////////////////////////////////////////////
// After
pub fn foo(x: i32) {}

///////////////////////////////////////////////////////////
// Example usage that will break.
fn main() {
    updated_crate::foo(); // Error: this function takes 1 argument
}
```

Mitigating strategies:
* Introduce a new function with the new signature and possibly
  [deprecate][deprecated] the old one.
* Introduce functions that take a struct argument, where the struct is built
  with the builder pattern. This allows new fields to be added to the struct
  in the future.

### Possibly-breaking: introducing a new function type parameter {#fn-generic-new}

Usually, adding a non-defaulted type parameter is safe, but in some
cases it can be a breaking change:

```rust,ignore
// Breaking change example

///////////////////////////////////////////////////////////
// Before
pub fn foo<T>() {}

///////////////////////////////////////////////////////////
// After
pub fn foo<T, U>() {}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::foo;

fn main() {
    foo::<u8>(); // Error: function takes 2 generic arguments but 1 generic argument was supplied
}
```

However, such explicit calls are rare enough (and can usually be written in
other ways) that this breakage is usually acceptable. One should take into
account how likely it is that the function in question is being called with
explicit type arguments.

### Minor: generalizing a function to use generics (supporting original type) {#fn-generalize-compatible}

The type of a parameter to a function, or its return value, can be
*generalized* to use generics, including by introducing a new type parameter,
as long as it can be instantiated to the original type. For example, the
following changes are allowed:

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub fn foo(x: u8) -> u8 {
    x
}
pub fn bar<T: Iterator<Item = u8>>(t: T) {}

///////////////////////////////////////////////////////////
// After
use std::ops::Add;
pub fn foo<T: Add>(x: T) -> T {
    x
}
pub fn bar<T: IntoIterator<Item = u8>>(t: T) {}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
use updated_crate::{bar, foo};

fn main() {
    foo(1);
    bar(vec![1, 2, 3].into_iter());
}
```

because all existing uses are instantiations of the new signature.

Perhaps somewhat surprisingly, generalization applies to trait objects as
well, given that every trait implements itself:

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub trait Trait {}
pub fn foo(t: &dyn Trait) {}

///////////////////////////////////////////////////////////
// After
pub trait Trait {}
pub fn foo<T: Trait + ?Sized>(t: &T) {}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.
use updated_crate::{foo, Trait};

struct Foo;
impl Trait for Foo {}

fn main() {
    let obj = Foo;
    foo(&obj);
}
```

(The use of `?Sized` is essential; otherwise you couldn't recover the original
signature.)

Introducing generics in this way can potentially create type inference
failures. These are usually rare, and may be acceptable breakage for some
projects, as this can be fixed with additional type annotations.

```rust,ignore
// Breaking change example

///////////////////////////////////////////////////////////
// Before
pub fn foo() -> i32 {
    0
}

///////////////////////////////////////////////////////////
// After
pub fn foo<T: Default>() -> T {
    Default::default()
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::foo;

fn main() {
    let x = foo(); // Error: type annotations needed
}
```

### Major: generalizing a function to use generics with type mismatch {#fn-generalize-mismatch}

It is a breaking change to change a function parameter or return type if the
generic type constrains or changes the types previously allowed. For example,
the following adds a generic constraint that may not be satisfied by existing
code:

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub fn foo(x: Vec<u8>) {}

///////////////////////////////////////////////////////////
// After
pub fn foo<T: Copy + IntoIterator<Item = u8>>(x: T) {}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::foo;

fn main() {
    foo(vec![1, 2, 3]); // Error: `Copy` is not implemented for `Vec<u8>`
}
```

### Minor: making an `unsafe` function safe {#fn-unsafe-safe}

A previously `unsafe` function can be made safe without breaking code.

Note however that it may cause the [`unused_unsafe`][unused_unsafe] lint to
trigger as in the example below, which will cause local crates that have
specified `#![deny(warnings)]` to stop compiling. Per [introducing new
lints](#new-lints), it is allowed for updates to introduce new warnings.

Going the other way (making a safe function `unsafe`) is a breaking change.

```rust,ignore
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub unsafe fn foo() {}

///////////////////////////////////////////////////////////
// After
pub fn foo() {}

///////////////////////////////////////////////////////////
// Example use of the library that will trigger a lint.
use updated_crate::foo;

unsafe fn bar(f: unsafe fn()) {
    f()
}

fn main() {
    unsafe { foo() }; // The `unused_unsafe` lint will trigger here
    unsafe { bar(foo) };
}
```

Making a previously `unsafe` associated function or method on structs / enums
safe is also a minor change, while the same is not true for associated
function on traits (see [any change to trait item signatures](#trait-item-signature)).

### Major: switching from `no_std` support to requiring `std` {#attr-no-std-to-std}

If your library specifically supports a [`no_std`] environment, it is a
breaking change to make a new release that requires `std`.

```rust,ignore,skip
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
#![no_std]
pub fn foo() {}

///////////////////////////////////////////////////////////
// After
pub fn foo() {
    std::time::SystemTime::now();
}

///////////////////////////////////////////////////////////
// Example usage that will break.
// This will fail to link for no_std targets because they don't have a `std` crate.
#![no_std]
use updated_crate::foo;

fn example() {
    foo();
}
```

Mitigation strategies:
* A common idiom to avoid this is to include a `std` [Cargo feature] that
  optionally enables `std` support, and when the feature is off, the library
  can be used in a `no_std` environment.

### Major: adding `non_exhaustive` to an existing enum, variant, or struct with no private fields {#attr-adding-non-exhaustive}

Making items [`#[non_exhaustive]`][non_exhaustive] changes how they may
be used outside the crate where they are defined:

- Non-exhaustive structs and enum variants cannot be constructed
  using [struct literal] syntax, including [functional update syntax].
- Pattern matching on non-exhaustive structs requires `..` and
  matching on enums does not count towards exhaustiveness.
- Casting enum variants to their discriminant with `as` is not allowed.

Structs with private fields cannot be constructed using [struct literal] syntax
regardless of whether [`#[non_exhaustive]`][non_exhaustive] is used.
Adding [`#[non_exhaustive]`][non_exhaustive] to such a struct is not
a breaking change.

```rust,ignore
// MAJOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub struct Foo {
    pub bar: usize,
}

pub enum Bar {
    X,
    Y(usize),
    Z { a: usize },
}

pub enum Quux {
    Var,
}

///////////////////////////////////////////////////////////
// After
#[non_exhaustive]
pub struct Foo {
    pub bar: usize,
}

pub enum Bar {
    #[non_exhaustive]
    X,

    #[non_exhaustive]
    Y(usize),

    #[non_exhaustive]
    Z { a: usize },
}

#[non_exhaustive]
pub enum Quux {
    Var,
}

///////////////////////////////////////////////////////////
// Example usage that will break.
use updated_crate::{Bar, Foo, Quux};

fn main() {
    let foo = Foo { bar: 0 }; // Error: cannot create non-exhaustive struct using struct expression

    let bar_x = Bar::X; // Error: unit variant `X` is private
    let bar_y = Bar::Y(0); // Error: tuple variant `Y` is private
    let bar_z = Bar::Z { a: 0 }; // Error: cannot create non-exhaustive variant using struct expression

    let q = Quux::Var;
    match q {
        Quux::Var => 0,
        // Error: non-exhaustive patterns: `_` not covered
    };
}
```

Mitigation strategies:
* Mark structs, enums, and enum variants as
  [`#[non_exhaustive]`][non_exhaustive] when first introducing them,
  rather than adding [`#[non_exhaustive]`][non_exhaustive] later on.

## Tooling and environment compatibility

### Possibly-breaking: changing the minimum version of Rust required {#env-new-rust}

Introducing the use of new features in a new release of Rust can break
projects that are using older versions of Rust. This also includes using new
features in a new release of Cargo, and requiring the use of a nightly-only
feature in a crate that previously worked on stable.

It is generally recommended to treat this as a minor change, rather than as
a major change, for [various reasons][msrv-is-minor]. It
is usually relatively easy to update to a newer version of Rust. Rust also has
a rapid 6-week release cycle, and some projects will provide compatibility
within a window of releases (such as the current stable release plus N
previous releases). Just keep in mind that some large projects may not be able
to update their Rust toolchain rapidly.

Mitigation strategies:
* Use [Cargo features] to make the new features opt-in.
* Provide a large window of support for older releases.
* Copy the source of new standard library items if possible so that you
  can continue to use an older version but take advantage of the new feature.
* Provide a separate branch of older minor releases that can receive backports
  of important bugfixes.
* Keep an eye out for the [`[cfg(version(..))]`][cfg-version] and
  [`#[cfg(accessible(..))]`][cfg-accessible] features which provide an opt-in
  mechanism for new features. These are currently unstable and only available
  in the nightly channel.

### Possibly-breaking: changing the platform and environment requirements {#env-change-requirements}

There is a very wide range of assumptions a library makes about the
environment that it runs in, such as the host platform, operating system
version, available services, filesystem support, etc. It can be a breaking
change if you make a new release that restricts what was previously supported,
for example requiring a newer version of an operating system. These changes
can be difficult to track, since you may not always know if a change breaks in
an environment that is not automatically tested.

Some projects may deem this acceptable breakage, particularly if the breakage
is unlikely for most users, or the project doesn't have the resources to
support all environments. Another notable situation is when a vendor
discontinues support for some hardware or OS, the project may deem it
reasonable to also discontinue support.

Mitigation strategies:
* Document the platforms and environments you specifically support.
* Test your code on a wide range of environments in CI.

### Minor: introducing new lints {#new-lints}

Some changes to a library may cause new lints to be triggered in users of that library.
This should generally be considered a compatible change.

```rust,ignore,dont-deny
// MINOR CHANGE

///////////////////////////////////////////////////////////
// Before
pub fn foo() {}

///////////////////////////////////////////////////////////
// After
#[deprecated]
pub fn foo() {}

///////////////////////////////////////////////////////////
// Example use of the library that will safely work.

fn main() {
    updated_crate::foo(); // Warning: use of deprecated function
}
```

Beware that it may be possible for this to technically cause a project to fail if they have explicitly denied the warning, and the updated crate is a direct dependency.
Denying warnings should be done with care and the understanding that new lints may be introduced over time.
However, library authors should be cautious about introducing new warnings and may want to consider the potential impact on their users.

The following lints are examples of those that may be introduced when updating a dependency:

* [`deprecated`][deprecated-lint] --- Introduced when a dependency adds the [`#[deprecated]` attribute][deprecated] to an item you are using.
* [`unused_must_use`] --- Introduced when a dependency adds the [`#[must_use]` attribute][must-use-attr] to an item where you are not consuming the result.
* [`unused_unsafe`] --- Introduced when a dependency *removes* the `unsafe` qualifier from a function, and that is the only unsafe function called in an unsafe block.

Additionally, updating `rustc` to a new version may introduce new lints.

Transitive dependencies which introduce new lints should not usually cause a failure because Cargo uses [`--cap-lints`](../../rustc/lints/levels.html#capping-lints) to suppress all lints in dependencies.

Mitigating strategies:
* If you build with warnings denied, understand you may need to deal with resolving new warnings whenever you update your dependencies.
  If using RUSTFLAGS to pass `-Dwarnings`, also add the `-A` flag to allow lints that are likely to cause issues, such as `-Adeprecated`.
* Introduce deprecations behind a [feature][Cargo features].
  For example `#[cfg_attr(feature = "deprecated", deprecated="use bar instead")]`.
  Then, when you plan to remove an item in a future SemVer breaking change, you can communicate with your users that they should enable the `deprecated` feature *before* updating to remove the use of the deprecated items.
  This allows users to choose when to respond to deprecations without needing to immediately respond to them.
  A downside is that it can be difficult to communicate to users that they need to take these manual steps to prepare for a major update.

[`unused_must_use`]: ../../rustc/lints/listing/warn-by-default.html#unused-must-use
[deprecated-lint]: ../../rustc/lints/listing/warn-by-default.html#deprecated
[must-use-attr]: ../../reference/attributes/diagnostics.html#the-must_use-attribute
[`unused_unsafe`]: ../../rustc/lints/listing/warn-by-default.html#unused-unsafe

### Cargo

#### Minor: adding a new Cargo feature {#cargo-feature-add}

It is usually safe to add new [Cargo features]. If the feature introduces new
changes that cause a breaking change, this can cause difficulties for projects
that have stricter backwards-compatibility needs. In that scenario, avoid
adding the feature to the "default" list, and possibly document the
consequences of enabling the feature.

```toml
# MINOR CHANGE

###########################################################
# Before
[features]
# ..empty

###########################################################
# After
[features]
std = []
```

#### Major: removing a Cargo feature {#cargo-feature-remove}

It is usually a breaking change to remove [Cargo features]. This will cause
an error for any project that enabled the feature.

```toml
# MAJOR CHANGE

###########################################################
# Before
[features]
logging = []

###########################################################
# After
[dependencies]
# ..logging removed
```

Mitigation strategies:
* Clearly document your features. If there is an internal or experimental
  feature, mark it as such, so that users know the status of the feature.
* Leave the old feature in `Cargo.toml`, but otherwise remove its
  functionality. Document that the feature is deprecated, and remove it in a
  future major SemVer release.

#### Major: removing a feature from a feature list if that changes functionality or public items {#cargo-feature-remove-another}

If removing a feature from another feature, this can break existing users if
they are expecting that functionality to be available through that feature.

```toml
# Breaking change example

###########################################################
# Before
[features]
default = ["std"]
std = []

###########################################################
# After
[features]
default = []  # This may cause packages to fail if they are expecting std to be enabled.
std = []
```

#### Possibly-breaking: removing an optional dependency {#cargo-remove-opt-dep}

Removing an [optional dependency][opt-dep] can break a project using your library because
another project may be enabling that dependency via [Cargo features].

When there is an optional dependency, cargo implicitly defines a feature of
the same name to provide a mechanism to enable the dependency and to check
when it is enabled. This problem can be avoided by using the `dep:` syntax in
the `[features]` table, which disables this implicit feature. Using `dep:`
makes it possible to hide the existence of optional dependencies under more
semantically-relevant names which can be more safely modified.

```toml
# Breaking change example

###########################################################
# Before
[dependencies]
curl = { version = "0.4.31", optional = true }

###########################################################
# After
[dependencies]
# ..curl removed
```

```toml
# MINOR CHANGE
#
# This example shows how to avoid breaking changes with optional dependencies.

###########################################################
# Before
[dependencies]
curl = { version = "0.4.31", optional = true }

[features]
networking = ["dep:curl"]

###########################################################
# After
[dependencies]
# Here, one optional dependency was replaced with another.
hyper = { version = "0.14.27", optional = true }

[features]
networking = ["dep:hyper"]
```

Mitigation strategies:
* Use the `dep:` syntax in the `[features]` table to avoid exposing optional
  dependencies in the first place. See [optional dependencies][opt-dep] for
  more information.
* Clearly document your features. If the optional dependency is not included
  in the documented list of features, then you may decide to consider it safe
  to change undocumented entries.
* Leave the optional dependency, and just don't use it within your library.
* Replace the optional dependency with a [Cargo feature] that does nothing,
  and document that it is deprecated.
* Use high-level features which enable optional dependencies, and document
  those as the preferred way to enable the extended functionality. For
  example, if your library has optional support for something like
  "networking", create a generic feature name "networking" that enables the
  optional dependencies necessary to implement "networking". Then document the
  "networking" feature.

[opt-dep]: features.md#optional-dependencies

#### Minor: changing dependency features {#cargo-change-dep-feature}

It is usually safe to change the features on a dependency, as long as the
feature does not introduce a breaking change.

```toml
# MINOR CHANGE

###########################################################
# Before
[dependencies]
rand = { version = "0.7.3", features = ["small_rng"] }


###########################################################
# After
[dependencies]
rand = "0.7.3"
```

#### Minor: adding dependencies {#cargo-dep-add}

It is usually safe to add new dependencies, as long as the new dependency
does not introduce new requirements that result in a breaking change.
For example, adding a new dependency that requires nightly in a project
that previously worked on stable is a major change.

```toml
# MINOR CHANGE

###########################################################
# Before
[dependencies]
# ..empty

###########################################################
# After
[dependencies]
log = "0.4.11"
```

## Application compatibility

Cargo projects may also include executable binaries which have their own
interfaces (such as a CLI interface, OS-level interaction, etc.). Since these
are part of the Cargo package, they often use and share the same version as
the package. You will need to decide if and how you want to employ a SemVer
contract with your users in the changes you make to your application. The
potential breaking and compatible changes to an application are too numerous
to list, so you are encouraged to use the spirit of the [SemVer] spec to guide
your decisions on how to apply versioning to your application, or at least
document what your commitments are.

[`cfg` attribute]: ../../reference/conditional-compilation.md#the-cfg-attribute
[`no_std`]: ../../reference/names/preludes.html#the-no_std-attribute
[`pub use`]: ../../reference/items/use-declarations.html
[Cargo feature]: features.md
[Cargo features]: features.md
[cfg-accessible]: https://github.com/rust-lang/rust/issues/64797
[cfg-version]: https://github.com/rust-lang/rust/issues/64796
[conditional compilation]: ../../reference/conditional-compilation.md
[Default]: ../../std/default/trait.Default.html
[deprecated]: ../../reference/attributes/diagnostics.html#the-deprecated-attribute
[disambiguation syntax]: ../../reference/expressions/call-expr.html#disambiguating-function-calls
[functional update syntax]: ../../reference/expressions/struct-expr.html#functional-update-syntax
[inherent implementations]: ../../reference/items/implementations.html#inherent-implementations
[items]: ../../reference/items.html
[non_exhaustive]: ../../reference/attributes/type_system.html#the-non_exhaustive-attribute
[object safe]: ../../reference/items/traits.html#object-safety
[rust-feature]: https://doc.rust-lang.org/nightly/unstable-book/
[sealed trait]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
[SemVer]: https://semver.org/
[struct literal]: ../../reference/expressions/struct-expr.html
[wildcard patterns]: ../../reference/patterns.html#wildcard-pattern
[unused_unsafe]: ../../rustc/lints/listing/warn-by-default.html#unused-unsafe
[msrv-is-minor]: https://github.com/rust-lang/api-guidelines/discussions/231
