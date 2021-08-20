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
[rust-semverver]: https://github.com/rust-dev-tools/rust-semverver
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
    * Attributes
        * [Major: switching from `no_std` support to requiring `std`](#attr-no-std-to-std)
* Tooling and environment compatibility
    * [Possibly-breaking: changing the minimum version of Rust required](#env-new-rust)
    * [Possibly-breaking: changing the platform and environment requirements](#env-change-requirements)
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

<a id="item-remove"></a>
### Major: renaming/moving/removing any public items

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

<a id="item-new"></a>
### Minor: adding new public items

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

<a id="struct-add-private-field-when-public"></a>
### Major: adding a private struct field when all current fields are public

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

<a id="struct-add-public-field-when-no-private"></a>
### Major: adding a public field when no private field exists

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

<a id="struct-private-fields-with-private"></a>
### Minor: adding or removing private fields when at least one already exists

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

<a id="struct-tuple-normal-with-private"></a>
### Minor: going from a tuple struct with all private fields (with at least one field) to a normal struct, or vice versa

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

<a id="enum-variant-new"></a>
### Major: adding new enum variants (without `non_exhaustive`)

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
    match x { // Error: `Variant2` not covered
        E::Variant1 => {}
    }
}
```

Mitigation strategies:
* When introducing the enum, mark it as [`#[non_exhaustive]`][non_exhaustive]
  to force users to use [wildcard patterns] to catch new variants.

<a id="enum-fields-new"></a>
### Major: adding new fields to an enum variant

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

<a id="trait-new-item-no-default"></a>
### Major: adding a non-defaulted trait item

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

<a id="trait-item-signature"></a>
### Major: any change to trait item signatures

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

<a id="trait-new-default-item"></a>
### Possibly-breaking: adding a defaulted trait item

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

<a id="trait-object-safety"></a>
### Major: adding a trait item that makes the trait non-object safe

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

<a id="trait-new-parameter-no-default"></a>
### Major: adding a type parameter without a default

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

<a id="trait-new-parameter-default"></a>
### Minor: adding a defaulted trait type parameter

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

<a id="impl-item-new"></a>
### Possibly-breaking change: adding any inherent items

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
    x.foo(1); // Error: this function takes 0 arguments
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

<a id="generic-bounds-tighten"></a>
### Major: tightening generic bounds

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

<a id="generic-bounds-loosen"></a>
### Minor: loosening generic bounds

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

<a id="generic-new-default"></a>
### Minor: adding defaulted type parameters

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

<a id="generic-generalize-identical"></a>
### Minor: generalizing a type to use generics (with identical types)

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

<a id="generic-generalize-different"></a>
### Major: generalizing a type to use generics (with possibly different types)

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

<a id="generic-more-generic"></a>
### Minor: changing a generic type to a more generic type

It is safe to change a generic type to a more generic one. For example, the
following adds a generic parameter that defaults to the original type, which
is safe because all existing users will be using the same type for both
fields, the the defaulted parameter does not need to be specified.

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

<a id="fn-change-arity"></a>
### Major: adding/removing function parameters

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

<a id="fn-generic-new"></a>
### Possibly-breaking: introducing a new function type parameter

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
    foo::<u8>(); // Error: this function takes 2 generic arguments but 1 generic argument was supplied
}
```

However, such explicit calls are rare enough (and can usually be written in
other ways) that this breakage is usually acceptable. One should take into
account how likely it is that the function in question is being called with
explicit type arguments.

<a id="fn-generalize-compatible"></a>
### Minor: generalizing a function to use generics (supporting original type)

The type of an parameter to a function, or its return value, can be
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

<a id="fn-generalize-mismatch"></a>
### Major: generalizing a function to use generics with type mismatch

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

<a id="attr-no-std-to-std"></a>
### Major: switching from `no_std` support to requiring `std`

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

## Tooling and environment compatibility

<a id="env-new-rust"></a>
### Possibly-breaking: changing the minimum version of Rust required

Introducing the use of new features in a new release of Rust can break
projects that are using older versions of Rust. This also includes using new
features in a new release of Cargo, and requiring the use of a nightly-only
feature in a crate that previously worked on stable.

Some projects choose to allow this in a minor release for various reasons. It
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

<a id="env-change-requirements"></a>
### Possibly-breaking: changing the platform and environment requirements

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

### Cargo

<a id="cargo-feature-add"></a>
#### Minor: adding a new Cargo feature

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

<a id="cargo-feature-remove"></a>
#### Major: removing a Cargo feature

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

<a id="cargo-feature-remove-another"></a>
#### Major: removing a feature from a feature list if that changes functionality or public items

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

<a id="cargo-remove-opt-dep"></a>
#### Possibly-breaking: removing an optional dependency

Removing an optional dependency can break a project using your library because
another project may be enabling that dependency via [Cargo features].

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

Mitigation strategies:
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

<a id="cargo-change-dep-feature"></a>
#### Minor: changing dependency features

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

<a id="cargo-dep-add"></a>
#### Minor: adding dependencies

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
[inherent implementations]: ../../reference/items/implementations.html#inherent-implementations
[items]: ../../reference/items.html
[non_exhaustive]: ../../reference/attributes/type_system.html#the-non_exhaustive-attribute
[object safe]: ../../reference/items/traits.html#object-safety
[rust-feature]: https://doc.rust-lang.org/nightly/unstable-book/
[sealed trait]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
[SemVer]: https://semver.org/
[struct literal]: ../../reference/expressions/struct-expr.html
[wildcard patterns]: ../../reference/patterns.html#wildcard-pattern
