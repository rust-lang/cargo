use crate::core::compiler::{CompileMode, Kind};
use crate::core::{profiles::Profile, Package, Target};
use crate::util::hex::short_hash;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

/// All information needed to define a unit.
///
/// A unit is an object that has enough information so that cargo knows how to build it.
/// For example, if your package has dependencies, then every dependency will be built as a library
/// unit. If your package is a library, then it will be built as a library unit as well, or if it
/// is a binary with `main.rs`, then a binary will be output. There are also separate unit types
/// for `test`ing and `check`ing, amongst others.
///
/// The unit also holds information about all possible metadata about the package in `pkg`.
///
/// A unit needs to know extra information in addition to the type and root source file. For
/// example, it needs to know the target architecture (OS, chip arch etc.) and it needs to know
/// whether you want a debug or release build. There is enough information in this struct to figure
/// all that out.
#[derive(Clone, Copy, PartialOrd, Ord)]
pub struct Unit<'a> {
    inner: &'a UnitInner<'a>,
}

/// Internal fields of `Unit` which `Unit` will dereference to.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitInner<'a> {
    /// Information about available targets, which files to include/exclude, etc. Basically stuff in
    /// `Cargo.toml`.
    pub pkg: &'a Package,
    /// Information about the specific target to build, out of the possible targets in `pkg`. Not
    /// to be confused with *target-triple* (or *target architecture* ...), the target arch for a
    /// build.
    pub target: &'a Target,
    /// The profile contains information about *how* the build should be run, including debug
    /// level, etc.
    pub profile: Profile,
    /// Whether this compilation unit is for the host or target architecture.
    ///
    /// For example, when
    /// cross compiling and using a custom build script, the build script needs to be compiled for
    /// the host architecture so the host rustc can use it (when compiling to the target
    /// architecture).
    pub kind: Kind,
    /// The "mode" this unit is being compiled for. See [`CompileMode`] for more details.
    pub mode: CompileMode,
}

impl<'a> Unit<'a> {
    pub fn buildkey(&self) -> String {
        format!("{}-{}", self.pkg.name(), short_hash(self))
    }
}

// Just hash the pointer for fast hashing
impl<'a> Hash for Unit<'a> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        (self.inner as *const UnitInner<'a>).hash(hasher)
    }
}

// Just equate the pointer since these are interned
impl<'a> PartialEq for Unit<'a> {
    fn eq(&self, other: &Unit<'a>) -> bool {
        self.inner as *const UnitInner<'a> == other.inner as *const UnitInner<'a>
    }
}

impl<'a> Eq for Unit<'a> {}

impl<'a> Deref for Unit<'a> {
    type Target = UnitInner<'a>;

    fn deref(&self) -> &UnitInner<'a> {
        self.inner
    }
}

impl<'a> fmt::Debug for Unit<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Unit")
            .field("pkg", &self.pkg)
            .field("target", &self.target)
            .field("profile", &self.profile)
            .field("kind", &self.kind)
            .field("mode", &self.mode)
            .finish()
    }
}

pub struct UnitInterner<'a> {
    state: RefCell<InternerState<'a>>,
}

struct InternerState<'a> {
    cache: HashSet<Box<UnitInner<'a>>>,
}

impl<'a> UnitInterner<'a> {
    pub fn new() -> UnitInterner<'a> {
        UnitInterner {
            state: RefCell::new(InternerState {
                cache: HashSet::new(),
            }),
        }
    }

    pub fn intern(
        &'a self,
        pkg: &'a Package,
        target: &'a Target,
        profile: Profile,
        kind: Kind,
        mode: CompileMode,
    ) -> Unit<'a> {
        let inner = self.intern_inner(&UnitInner {
            pkg,
            target,
            profile,
            kind,
            mode,
        });
        Unit { inner }
    }

    fn intern_inner(&'a self, item: &UnitInner<'a>) -> &'a UnitInner<'a> {
        let mut me = self.state.borrow_mut();
        if let Some(item) = me.cache.get(item) {
            return unsafe { &*(&**item as *const UnitInner<'a>) };
        }
        me.cache.insert(Box::new(item.clone()));
        let item = me.cache.get(item).unwrap();
        return unsafe { &*(&**item as *const UnitInner<'a>) };
    }
}
