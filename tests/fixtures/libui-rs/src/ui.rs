//! General functions.

use ffi_utils::{self, Text};
use libc::{c_char, c_void};
use std::fmt::{self, Debug, Formatter};
use std::ffi::{CStr, CString};
use std::mem;
use std::ops::Deref;
use ui_sys::{self, uiInitOptions};
use windows::Window;

#[derive(Clone)]
pub struct InitOptions;

#[inline]
pub fn init(_: InitOptions) -> Result<(),InitError> {
    unsafe {
        let mut init_options = uiInitOptions {
            Size: mem::size_of::<uiInitOptions>(),
        };
        let err = ui_sys::uiInit(&mut init_options);
        if err.is_null() {
            ffi_utils::set_initialized();
            Ok(())
        } else {
            Err(InitError {
                ui_init_error: err,
            })
        }
    }
}

#[inline]
pub fn uninit() {
    unsafe {
        ffi_utils::unset_initialized();
        Window::destroy_all_windows();
        ui_sys::uiUninit();
    }
}

#[inline]
pub fn main() {
    unsafe {
        ui_sys::uiMain()
    }
}

#[inline]
pub fn quit() {
    unsafe {
        ui_sys::uiQuit()
    }
}

pub struct InitError {
    ui_init_error: *const c_char,
}

impl Drop for InitError {
    fn drop(&mut self) {
        unsafe {
            ui_sys::uiFreeInitError(self.ui_init_error)
        }
    }
}

impl Debug for InitError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        (**self).fmt(f)
    }
}

impl Deref for InitError {
    type Target = str;
    fn deref(&self) -> &str {
        unsafe {
            CStr::from_ptr(self.ui_init_error).to_str().unwrap_or("")
        }
    }
}

#[inline]
pub fn queue_main(callback: Box<FnMut()>) {
    unsafe {
        let mut data: Box<Box<FnMut()>> = Box::new(callback);
        ui_sys::uiQueueMain(ffi_utils::void_void_callback,
                            &mut *data as *mut Box<FnMut()> as *mut c_void);
        mem::forget(data);
    }
}

#[inline]
pub fn on_should_quit(callback: Box<FnMut()>) {
    unsafe {
        let mut data: Box<Box<FnMut()>> = Box::new(callback);
        ui_sys::uiOnShouldQuit(ffi_utils::void_void_callback,
                               &mut *data as *mut Box<FnMut()> as *mut c_void);
        mem::forget(data);
    }
}

#[inline]
pub fn open_file(parent: &Window) -> Option<Text> {
    unsafe {
        Text::optional(ui_sys::uiOpenFile(parent.as_ui_window()))
    }
}

#[inline]
pub fn save_file(parent: &Window) -> Option<Text> {
    unsafe {
        Text::optional(ui_sys::uiSaveFile(parent.as_ui_window()))
    }
}

#[inline]
pub fn msg_box(parent: &Window, title: &str, description: &str) {
    unsafe {
        let c_title = CString::new(title.as_bytes().to_vec()).unwrap();
        let c_description = CString::new(description.as_bytes().to_vec()).unwrap();
        ui_sys::uiMsgBox(parent.as_ui_window(), c_title.as_ptr(), c_description.as_ptr())
    }
}

#[inline]
pub fn msg_box_error(parent: &Window, title: &str, description: &str) {
    unsafe {
        let c_title = CString::new(title.as_bytes().to_vec()).unwrap();
        let c_description = CString::new(description.as_bytes().to_vec()).unwrap();
        ui_sys::uiMsgBoxError(parent.as_ui_window(), c_title.as_ptr(), c_description.as_ptr())
    }
}

