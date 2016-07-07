//! Rust bindings to `libui`.
//!
//! Main C source repository: https://github.com/andlabs/libui
//!
//! Copyright © 2016 Mozilla Foundation

#[macro_use]
extern crate bitflags;
extern crate libc;
extern crate ui_sys;

pub use controls::{Area, AreaDrawParams, AreaHandler, BoxControl, Button, Checkbox, ColorButton};
pub use controls::{Combobox, Control, DateTimePicker, Entry, FontButton, Group, Label};
pub use controls::{MultilineEntry, ProgressBar, RadioButtons, Separator, Slider, Spinbox, Tab};
pub use ffi_utils::Text;
pub use menus::{Menu, MenuItem};
pub use ui::{InitError, InitOptions, init, main, msg_box, msg_box_error, on_should_quit};
pub use ui::{open_file, queue_main, quit, save_file, uninit};
pub use windows::Window;

#[macro_use] 
mod controls;
pub mod draw; 
pub mod ffi_utils;
mod menus;
mod ui;
mod windows;

