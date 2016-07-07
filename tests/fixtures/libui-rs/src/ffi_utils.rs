//! Useful utility functions for calling the `libui` C bindings.

use libc::{c_char, c_void};
use std::ffi::CStr;
use std::mem;
use std::ops::Deref;
use std::sync::atomic::{ATOMIC_BOOL_INIT, AtomicBool, Ordering};
use ui_sys;

static INITIALIZED: AtomicBool = ATOMIC_BOOL_INIT;

#[inline]
pub unsafe fn set_initialized() {
    assert!(!INITIALIZED.swap(true, Ordering::SeqCst));
}

#[inline]
pub unsafe fn unset_initialized() {
    INITIALIZED.store(false, Ordering::SeqCst);
}

#[inline]
pub fn ensure_initialized() {
    assert!(INITIALIZED.load(Ordering::SeqCst));
}

pub struct Text {
    ui_text: *mut c_char,
}

impl Drop for Text {
    fn drop(&mut self) {
        unsafe {
            ui_sys::uiFreeText(self.ui_text)
        }
    }
}

impl Deref for Text {
    type Target = str;
    fn deref(&self) -> &str {
        unsafe {
            CStr::from_ptr(self.ui_text).to_str().unwrap_or("")
        }
    }
}

impl Text {
    #[inline]
    pub unsafe fn new(text: *mut c_char) -> Text {
        debug_assert!(!text.is_null());
        Text {
            ui_text: text,
        }
    }

    #[inline]
    pub unsafe fn optional(text: *mut c_char) -> Option<Text> {
        if text.is_null() {
            None
        } else {
            Some(Text {
                ui_text: text,
            })
        }
    }
}

pub extern "C" fn void_void_callback(data: *mut c_void) {
    unsafe {
        mem::transmute::<*mut c_void, Box<Box<FnMut()>>>(data)()
    }
}

