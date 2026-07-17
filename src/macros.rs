use std::fmt;

macro_rules! compact_debug {
    (
        impl fmt::Debug for $ty:ident {
            fn fmt(&$this:ident, f: &mut fmt::Formatter) -> fmt::Result {
                let (default, default_name) = $e:expr;
                [debug_the_fields($($field:ident)*)]
            }
        }
    ) => (

        impl fmt::Debug for $ty {
            fn fmt(&$this, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                // Try printing a pretty version where we collapse as many fields as
                // possible, indicating that they're equivalent to a function call
                // that's hopefully enough to indicate what each value is without
                // actually dumping everything so verbosely.
                let mut s = f.debug_struct(stringify!($ty));
                let (default, default_name) = $e;
                let mut any_default = false;

                // Exhaustively match so when fields are added we get a compile
                // failure
                let $ty { $($field),* } = $this;
                $(
                    if *$field == default.$field {
                        any_default = true;
                    } else {
                        s.field(stringify!($field), $field);
                    }
                )*

                if any_default {
                    s.field("..", &crate::macros::DisplayAsDebug(default_name));
                }
                s.finish()
            }
        }
    )
}

pub struct DisplayAsDebug<T>(pub T);

impl<T: fmt::Display> fmt::Debug for DisplayAsDebug<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
