# What constitutes a breaking change in Rust

Rust's ecosystem has adopted [semver][semver], a
technique for versioning platforms/libraries partly in terms of the effect on
the code that uses them. In a nutshell, the versioning scheme has three components::

1. **Major**: must be incremented for changes that break downstream code.
2. **Minor**: incremented for backwards-compatible feature additions.
3. **Patch**: incremented for backwards-compatible bug fixes.

> **Note** As well as the rule that versions **<1.0.0** can change anything.

Typically, a **breaking change** is a change that, *strictly speaking*, can cause downstream
code to fail to compile

In Rust today, almost any change is technically a
breaking change. For example, given the way that globs currently work, *adding
any public item* to a library can break its clients (more on that later). But
not all breaking changes are equal. This has led to Rust's ecosystem adopting the following:

**All major changes are breaking, but not all breaking
changes are major.**

> **Note**: This belief was codified for the standard library in [RFC 1105][rfc]

## Principles of the policy

The basic policy is that **the same code should be able to run
against different minor revisions**. Furthermore, minor changes should require
at most a few local *annotations* to the code you are developing, and in
principle no changes to your dependencies.

In more detail:

* Minor changes should require at most minor amounts of work upon upgrade. For
  example, changes that may require occasional type annotations or use of [UFCS][ufcs]
  to disambiguate are not automatically "major" changes. (But in such cases, one
  must evaluate how widespread these "minor" changes are).

* In principle, it should be possible to produce a version of dependency code
  that *will not break* when upgrading other dependencies, or Rust itself, to a
  new minor revision. This goes hand-in-hand with the above bullet; as we will
  see, it's possible to save a fully "elaborated" version of upstream code that
  does not require any disambiguation. The "in principle" refers to the fact
  that getting there may require some additional tooling or language support,
  which this RFC outlines.

That means that any breakage in a minor release must be very "shallow": it must
always be possible to locally fix the problem through some kind of
disambiguation *that could have been done in advance* (by using more explicit
forms) or other annotation (like disabling a lint). It means that minor changes in upstream dependencies
can never leave a downstream dependency in a state that requires breaking changes to their own code.

**Although this general policy allows some (very limited) breakage in minor
releases, it is not a license to make these changes blindly**. The breakage that
this policy permits, aside from being very simple to fix, are also unlikely to occur
often in practice.

## Policy by language feature

Most of the policy is simplest to lay out with reference to specific language
features and the way that APIs using them can, and cannot, evolve in a minor
release.

**Breaking changes are assumed to be major changes unless otherwise stated**.
The RFC covers many, but not all breaking changes that are major; it covers
*all* breaking changes that are considered minor.

### Crates

#### Major change: going from stable to nightly

Changing a crate from working on stable Rust to *requiring* a nightly is
considered a breaking change. That includes using `#[feature]` directly, or
using a dependency that does so. Crate authors should consider using Cargo
["features"][features] for their crate to make such use opt-in.

#### Minor change: altering the use of Cargo features

Cargo packages can provide
[opt-in features][opt-in_features],
which enable `#[cfg]` options. When a common dependency is compiled, it is done
so with the *union* of all features opted into by any packages using the
dependency. That means that adding or removing a feature could technically break
other, unrelated code.

However, such breakage always represents a bug: packages are supposed to support
any combination of features, and if another client of the package depends on a
given feature, that client should specify the opt-in themselves.

### Modules

#### Major change: renaming/moving/removing any public items.

Although renaming an item might seem like a minor change, according to the
general policy design this is not a permitted form of breakage: it's not
possible to annotate code in advance to avoid the breakage, nor is it possible
to prevent the breakage from affecting dependencies.

Of course, much of the effect of renaming/moving/removing can be achieved by
instead using deprecation and `pub use`, and the standard library should not be
afraid to do so! In the long run, we should consider hiding at least some old
deprecated items from the docs, and could even consider putting out a major
version solely as a kind of "garbage collection" for long-deprecated APIs.

#### Minor change: adding new public items.

Note that adding new public items is currently a breaking change, due to glob
imports. For example, the following snippet of code will break if the `foo`
module introduces a public item called `bar`:

<!-- ignore, fake imports -->
```rust,ignore
use foo::*;
fn bar() { ... }
```

The problem here is that glob imports currently do not allow any of their
imports to be shadowed by an explicitly-defined item.

This is considered a minor change because under the principles of this policy: the
glob imports could have been written as more explicit (expanded) `use`
statements. It is also plausible to do this expansion automatically for a
crate's dependencies, to prevent breakage in the first place.

### Structs

See "[Signatures in type definitions](#signatures-in-type-definitions)" for some
general remarks about changes to the actual types in a `struct` definition.

#### Major change: adding a private field when all current fields are public.

This change has the effect of making external struct literals impossible to
write, which can break code irreparably.

#### Major change: adding a public field when no private field exists.

This change retains the ability to use struct literals, but it breaks existing
uses of such literals; it likewise breaks exhaustive matches against the struct.

#### Minor change: adding or removing private fields when at least one already exists (before and after the change).

No existing code could be relying on struct literals for the struct, nor on
exhaustively matching its contents, and client code will likewise be oblivious
to the addition of further private fields.

For tuple structs, this is only a minor change if furthermore *all* fields are
currently private. (Tuple structs with mixtures of public and private fields are
bad practice in any case.)

#### Minor change: going from a tuple struct with all private fields (with at least one field) to a normal struct, or vice versa.

This is technically a breaking change:

<!-- ignore, fake imports -->
```rust,ignore
// in some other module:
pub struct Foo(SomeType);

// in downstream code
let Foo(_) = foo;
```

Changing `Foo` to a normal struct can break code that matches on it -- but there
is never any real reason to match on it in that circumstance, since you cannot
extract any fields or learn anything of interest about the struct.

### Enums

See "[Signatures in type definitions](#signatures-in-type-definitions)" for some
general remarks about changes to the actual types in an `enum` definition.

#### Major change: adding new variants.

Exhaustiveness checking means that a `match` that explicitly checks all the
variants for an `enum` will break if a new variant is added. It is not currently
possible to defend against this breakage in advance.

#### Major change: adding new fields to a variant.

If the enum is public, so is the full contents of all of its variants. As per
the rules for structs, this means it is not allowed to add any new fields (which
will automatically be public).

If you wish to allow for this kind of extensibility, consider introducing a new,
explicit struct for the variant up front.

### Traits

#### Major change: adding a non-defaulted item.

Adding any item without a default will immediately break all trait implementations.

#### Major change: any non-trivial change to item signatures.

Because traits have both implementors and consumers, any change to the signature
of e.g. a method will affect at least one of the two parties. So, for example,
abstracting a concrete method to use generics instead might work fine for
clients of the trait, but would break existing implementors. (Note, as above,
the potential for "sealed" traits to alter this dynamic.)

#### Minor change: adding a defaulted item.

Adding a defaulted item is technically a breaking change:

```rust
trait Trait1 {}
trait Trait2 {
    fn foo(&self);
}

fn use_both<T: Trait1 + Trait2>(t: &T) {
    t.foo()
}
```

If a `foo` method is added to `Trait1`, even with a default, it would cause a
dispatch ambiguity in `use_both`, since the call to `foo` could be referring to
either trait.

(Note, however, that existing *implementations* of the trait are fine.)

According to the basic principles of this RFC, such a change is minor: it is
always possible to annotate the call `t.foo()` to be more explicit *in advance*
using UFCS: `Trait2::foo(t)`.

While the scenario of adding a defaulted method to a trait may seem somewhat
obscure, the exact same hazards arise with *implementing existing traits* (see
below), which is clearly vital to allow; we apply a similar policy to both.

All that said, it is incumbent on library authors to ensure that such "minor"
changes are in fact minor in practice: if a conflict like `t.foo()` is likely to
arise at all often in downstream code, it would be advisable to explore a
different choice of names. More guidelines for the standard library are given
later on.

There are two circumstances when adding a defaulted item is still a major change:

* The new item would change the trait from object safe to non-object safe.
* The trait has a defaulted associated type and the item being added is a
  defaulted function/method. In this case, existing impls that override the
  associated type will break, since the function/method default will not
  apply. (See
  [the associated item RFC](https://github.com/rust-lang/rfcs/blob/master/text/0195-associated-items.md#defaults)).
* Adding a default to an existing associated type is likewise a major change if
  the trait has defaulted methods, since it will invalidate use of those
  defaults for the methods in existing trait impls.

#### Minor change: adding a defaulted type parameter.

As with "[Signatures in type definitions](#signatures-in-type-definitions)",
traits are permitted to add new type parameters as long as defaults are provided
(which is backwards compatible).

### Trait implementations

#### Major change: implementing any "fundamental" trait.

A [recent RFC](https://github.com/rust-lang/rfcs/pull/1023) introduced the idea
of "fundamental" traits which are so basic that *not* implementing such a trait
right off the bat is considered a promise that you will *never* implement the
trait. The `Sized` and `Fn` traits are examples.

The coherence rules take advantage of fundamental traits in such a way that
*adding a new implementation of a fundamental trait to an existing type can
cause downstream breakage*. Thus, such impls are considered major changes.

#### Minor change: implementing any non-fundamental trait.

Unfortunately, implementing any existing trait can cause breakage:

<!-- ignore, fake imports -->
```rust,ignore
// Crate A
    pub trait Trait1 {
        fn foo(&self);
    }

    pub struct Foo; // does not implement Trait1

// Crate B
    use crateA::Trait1;

    trait Trait2 {
        fn foo(&self);
    }

    impl Trait2 for crateA::Foo { .. }

    fn use_foo(f: &crateA::Foo) {
        f.foo()
    }
```

If crate A adds an implementation of `Trait1` for `Foo`, the call to `f.foo()`
in crate B will yield a dispatch ambiguity (much like the one we saw for
defaulted items). Thus *technically implementing any existing trait is a
breaking change!* Completely prohibiting such a change is clearly a non-starter.

However, as before, this kind of breakage is considered "minor" by the
principles of this RFC (see "Adding a defaulted item" above).

### Inherent implementations

#### Minor change: adding any inherent items.

Adding an inherent item cannot lead to dispatch ambiguity, because inherent
items trump any trait items with the same name.

However, introducing an inherent item *can* lead to breakage if the signature of
the item does not match that of an in scope, implemented trait:

<!-- ignore, fake imports -->
```rust,ignore
// Crate A
    pub struct Foo;

// Crate B
    trait Trait {
        fn foo(&self);
    }

    impl Trait for crateA::Foo { .. }

    fn use_foo(f: &crateA::Foo) {
        f.foo()
   }
```

If crate A adds a method:

<!-- ignore, fake changes -->
```rust,ignore
impl Foo {
    fn foo(&self, x: u8) { /* ... */ }
}
```

then crate B would no longer compile, since dispatch would prefer the inherent
impl, which has the wrong type.

Once more, this is considered a minor change, since UFCS can disambiguate (see
"Adding a defaulted item" above).

It's worth noting, however, that if the signatures *did* happen to match then
the change would no longer cause a compilation error, but might silently change
runtime behavior. The case where the same method for the same type has
meaningfully different behavior is considered unlikely enough that the RFC is
willing to permit it to be labeled as a minor change -- and otherwise, inherent
methods could never be added after the fact.


#### Minor change: loosening bounds.

Loosening bounds, on the other hand, cannot break code because when you
reference `Foo<A>`, you *do not learn anything about the bounds on `A`*. (This
is why you have to repeat any relevant bounds in `impl` blocks for `Foo`, for
example.) So the following is a minor change:

<!-- ignore, fake changes -->
```rust,ignore
// MINOR CHANGE

// Before
struct Foo<A: Clone> { .. }

// After
struct Foo<A> { .. }
```

#### Minor change: adding defaulted type parameters.

All existing references to a type/trait definition continue to compile and work
correctly after a new defaulted type parameter is added. So the following is
a minor change:

<!-- ignore, fake changes -->
```rust,ignore
// MINOR CHANGE

// Before
struct Foo { .. }

// After
struct Foo<A = u8> { .. }
```

#### Minor change: generalizing to generics.

A struct or enum field can change from a concrete type to a generic type
parameter, provided that the change results in an identical type for all
existing use cases. For example, the following change is permitted:

<!-- ignore, fake changes -->
```rust,ignore
// MINOR CHANGE

// Before
struct Foo(pub u8);

// After
struct Foo<T = u8>(pub T);
```

because existing uses of `Foo` are shorthand for `Foo<u8>` which yields the
identical field type.

On the other hand, the following is not permitted:

<!-- ignore, fake changes -->
```rust,ignore
// MAJOR CHANGE

// Before
struct Foo<T = u8>(pub T, pub u8);

// After
struct Foo<T = u8>(pub T, pub T);
```

since there may be existing uses of `Foo` with a non-default type parameter
which would break as a result of the change.

It's also permitted to change from a generic type to a more-generic one in a
minor revision:

<!-- ignore, fake changes -->
```rust,ignore
// MINOR CHANGE

// Before
struct Foo<T>(pub T, pub T);

// After
struct Foo<T, U = T>(pub T, pub U);
```

since, again, all existing uses of the type `Foo<T>` will yield the same field
types as before.

### Signatures in functions

All of the changes mentioned below are considered major changes in the context
of trait methods, since they can break implementors.

#### Major change: adding/removing arguments.

At the moment, Rust does not provide defaulted arguments, so any change in arity
is a breaking change.

#### Minor change: introducing a new type parameter.

Technically, adding a (non-defaulted) type parameter can break code:

<!-- ignore, fake changes -->
```rust,ignore
// MINOR CHANGE (but causes breakage)

// Before
fn foo<T>(...) { ... }

// After
fn foo<T, U>(...) { ... }
```

will break any calls like `foo::<u8>`. However, such explicit calls are rare
enough (and can usually be written in other ways) that this breakage is
considered minor. (However, one should take into account how likely it is that
the function in question is being called with explicit type arguments).

Such changes are an important ingredient of abstracting to use generics, as
described next.

#### Minor change: generalizing to generics.

The type of an argument to a function, or its return value, can be *generalized*
to use generics, including by introducing a new type parameter (as long as it
can be instantiated to the original type). For example, the following change is
allowed:

<!-- ignore, fake changes -->
```rust,ignore
// MINOR CHANGE

// Before
fn foo(x: u8) -> u8;
fn bar<T: Iterator<Item = u8>>(t: T);

// After
fn foo<T: Add>(x: T) -> T;
fn bar<T: IntoIterator<Item = u8>>(t: T);
```

because all existing uses are instantiations of the new signature. On the other
hand, the following isn't allowed in a minor revision:

<!-- ignore, fake changes -->
```rust,ignore
// MAJOR CHANGE

// Before
fn foo(x: Vec<u8>);

// After
fn foo<T: Copy + IntoIterator<Item = u8>>(x: T);
```

because the generics include a constraint not satisfied by the original type.

Introducing generics in this way can potentially create type inference failures,
but these are considered acceptable as they only
require local annotations that could have been inserted in advance.

Perhaps somewhat surprisingly, generalization applies to trait objects as well,
given that every trait implements itself:

<!-- ignore, fake changes -->
```rust,ignore
// MINOR CHANGE

// Before
fn foo(t: &Trait);

// After
fn foo<T: Trait + ?Sized>(t: &T);
```

(The use of `?Sized` is essential; otherwise you couldn't recover the original
signature).

[rfc]: https://github.com/rust-lang/rfcs/pull/1105
[ufcs]: https://doc.rust-lang.org/1.7.0/book/ufcs.html
[semver]: https://semver.org/
[features]: http://doc.crates.io/manifest.html#the-[features]-section
[opt-in_features]: http://doc.crates.io/manifest.html#the-[features]-section
[postponed_RFC]: https://github.com/rust-lang/rfcs/pull/757
