use crate::core::compiler::{CompileKind, CompileMode};
use std::marker;
use std::rc::Rc;
use crate::core::manifest::{LibKind, Target, TargetKind};
use crate::core::{profiles::Profile, InternedString, Package};
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
#[derive(Clone, PartialOrd, Ord)]
pub struct Unit<'a> {
    // inner: Rc<UnitInner>,
    // _wut: marker::PhantomData<&'a ()>,
    inner: &'a UnitInner,
}

/// Internal fields of `Unit` which `Unit` will dereference to.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitInner {
    /// Information about available targets, which files to include/exclude, etc. Basically stuff in
    /// `Cargo.toml`.
    pub pkg: Package,
    /// Information about the specific target to build, out of the possible targets in `pkg`. Not
    /// to be confused with *target-triple* (or *target architecture* ...), the target arch for a
    /// build.
    pub target: Target,
    /// The profile contains information about *how* the build should be run, including debug
    /// level, etc.
    pub profile: Profile,
    /// Whether this compilation unit is for the host or target architecture.
    ///
    /// For example, when
    /// cross compiling and using a custom build script, the build script needs to be compiled for
    /// the host architecture so the host rustc can use it (when compiling to the target
    /// architecture).
    pub kind: CompileKind,
    /// The "mode" this unit is being compiled for. See [`CompileMode`] for more details.
    pub mode: CompileMode,
    /// The `cfg` features to enable for this unit.
    /// This must be sorted.
    pub features: Vec<InternedString>,
    /// Whether this is a standard library unit.
    pub is_std: bool,
}

impl UnitInner {
    /// Returns whether compilation of this unit requires all upstream artifacts
    /// to be available.
    ///
    /// This effectively means that this unit is a synchronization point (if the
    /// return value is `true`) that all previously pipelined units need to
    /// finish in their entirety before this one is started.
    pub fn requires_upstream_objects(&self) -> bool {
        self.mode.is_any_test() || self.target.kind().requires_upstream_objects()
    }
}

impl<'a> Unit<'a> {
    pub fn buildkey(&self) -> String {
        format!("{}-{}", self.pkg.name(), short_hash(self))
    }
}

// Just hash the pointer for fast hashing
impl<'a> Hash for Unit<'a> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        (&*self.inner as *const UnitInner).hash(hasher)
    }
}

// Just equate the pointer since these are interned
impl<'a> PartialEq for Unit<'a> {
    fn eq(&self, other: &Unit<'a>) -> bool {
        &*self.inner as *const UnitInner == &*other.inner as *const UnitInner
    }
}

impl<'a> Eq for Unit<'a> {}

impl<'a> Deref for Unit<'a> {
    type Target = UnitInner;

    fn deref(&self) -> &UnitInner {
        &*self.inner
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
            .field("features", &self.features)
            .finish()
    }
}

/// A small structure used to "intern" `Unit` values.
///
/// A `Unit` is just a thin pointer to an internal `UnitInner`. This is done to
/// ensure that `Unit` itself is quite small as well as enabling a very
/// efficient hash/equality implementation for `Unit`. All units are
/// manufactured through an interner which guarantees that each equivalent value
/// is only produced once.
pub struct UnitInterner {
    state: RefCell<InternerState>,
}

struct InternerState {
    // cache: HashSet<Rc<UnitInner>>,
    cache: HashSet<Box<UnitInner>>,
}

impl UnitInterner {
    /// Creates a new blank interner
    pub fn new() -> UnitInterner {
        UnitInterner {
            state: RefCell::new(InternerState {
                cache: HashSet::new(),
            }),
        }
    }

    /// Creates a new `unit` from its components. The returned `Unit`'s fields
    /// will all be equivalent to the provided arguments, although they may not
    /// be the exact same instance.
    pub fn intern(
        &self,
        pkg: &Package,
        target: &Target,
        profile: Profile,
        kind: CompileKind,
        mode: CompileMode,
        features: Vec<InternedString>,
        is_std: bool,
    ) -> Unit<'_> {
        let target = match (is_std, target.kind()) {
            // This is a horrible hack to support build-std. `libstd` declares
            // itself with both rlib and dylib. We don't want the dylib for a
            // few reasons:
            //
            // - dylibs don't have a hash in the filename. If you do something
            //   (like switch rustc versions), it will stomp on the dylib
            //   file, invalidating the entire cache (because std is a dep of
            //   everything).
            // - We don't want to publicize the presence of dylib for the
            //   standard library.
            //
            // At some point in the future, it would be nice to have a
            // first-class way of overriding or specifying crate-types.
            (true, TargetKind::Lib(crate_types)) if crate_types.contains(&LibKind::Dylib) => {
                let mut new_target = Target::clone(target);
                new_target.set_kind(TargetKind::Lib(vec![LibKind::Rlib]));
                new_target
            }
            _ => target.clone(),
        };
        let inner = self.intern_inner(&UnitInner {
            pkg: pkg.clone(),
            target,
            profile,
            kind,
            mode,
            features,
            is_std,
        });
        // Unit { inner, _wut: marker::PhantomData }
        Unit { inner }
    }

    // fn intern_inner(&self, item: &UnitInner) -> Rc<UnitInner> {
    //     let mut me = self.state.borrow_mut();
    //     if let Some(item) = me.cache.get(item) {
    //         return item.clone();
    //     }
    //     let item = Rc::new(item.clone());
    //     me.cache.insert(item.clone());
    //     return item;
    // }

    fn intern_inner<'a>(&'a self, item: &UnitInner) -> &'a UnitInner {
        let mut me = self.state.borrow_mut();
        if let Some(item) = me.cache.get(item) {
            // note that `item` has type `&Box<UnitInner<'a>`. Use `&**` to
            // convert that to `&UnitInner<'a>`, then do some trickery to extend
            // the lifetime to the `'a` on the function here.
            return unsafe { &*(&**item as *const UnitInner) };
        }
        me.cache.insert(Box::new(item.clone()));
        let item = me.cache.get(item).unwrap();
        unsafe { &*(&**item as *const UnitInner) }
    }
}
