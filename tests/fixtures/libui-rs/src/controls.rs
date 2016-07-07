//! Functions and types related to widgets.

use draw;
use ffi_utils::{self, Text};
use libc::{c_int, c_void};
use std::ffi::CString;
use std::mem;
use std::ptr;
use ui_sys::{self, uiArea, uiAreaDrawParams, uiAreaHandler, uiAreaKeyEvent, uiAreaMouseEvent};
use ui_sys::{uiBox, uiButton, uiCheckbox, uiColorButton, uiCombobox, uiControl, uiDateTimePicker};
use ui_sys::{uiEntry, uiFontButton, uiGroup, uiLabel, uiMultilineEntry, uiProgressBar};
use ui_sys::{uiRadioButtons, uiSeparator, uiSlider, uiSpinbox, uiTab};

pub use ui_sys::uiExtKey as ExtKey;

// Defines a new control, creating a Rust wrapper, a `Deref` implementation, and a destructor.
// An example of use:
//
//     define_control!(Slider, uiSlider, ui_slider)
#[macro_export]
macro_rules! define_control {
    ($rust_type:ident, $ui_type:ident, $ui_field:ident) => {
        pub struct $rust_type {
            $ui_field: *mut $ui_type,
        }

        impl ::std::ops::Deref for $rust_type {
            type Target = ::controls::Control;

            #[inline]
            fn deref(&self) -> &::controls::Control {
                // FIXME(pcwalton): $10 says this is undefined behavior. How do I make it not so?
                unsafe {
                    mem::transmute::<&$rust_type, &::controls::Control>(self)
                }
            }
        }

        impl Drop for $rust_type {
            #[inline]
            fn drop(&mut self) {
                // For now this does nothing, but in the future, when `libui` supports proper
                // memory management, this will likely need to twiddle reference counts.
            }
        }

        impl Clone for $rust_type {
            #[inline]
            fn clone(&self) -> $rust_type {
                $rust_type {
                    $ui_field: self.$ui_field,
                }
            }
        }

        impl Into<Control> for $rust_type {
            #[inline]
            fn into(self) -> Control {
                unsafe {
                    let control = Control::from_ui_control(self.$ui_field as *mut uiControl);
                    mem::forget(self);
                    control
                }
            }
        }

        impl $rust_type {
            #[inline]
            pub unsafe fn from_ui_control($ui_field: *mut $ui_type) -> $rust_type {
                $rust_type {
                    $ui_field: $ui_field
                }
            }
        }
    }
}

pub struct Control {
    ui_control: *mut uiControl,
}

impl Drop for Control {
    #[inline]
    fn drop(&mut self) {
        // For now this does nothing, but in the future, when `libui` supports proper memory
        // management, this will likely need to twiddle reference counts.
    }
}

impl Clone for Control {
    #[inline]
    fn clone(&self) -> Control {
        Control {
            ui_control: self.ui_control,
        }
    }
}

impl Control {
    /// Creates a new `Control` object from an existing `uiControl`.
    #[inline]
    pub unsafe fn from_ui_control(ui_control: *mut uiControl) -> Control {
        Control {
            ui_control: ui_control,
        }
    }

    #[inline]
    pub fn as_ui_control(&self) -> *mut uiControl {
        self.ui_control
    }

    /// Destroys a control. Any use of the control after this is use-after-free; therefore, this
    /// is marked unsafe.
    #[inline]
    pub unsafe fn destroy(&self) {
        // Don't check for initialization here since this can be run during deinitialization.
        ui_sys::uiControlDestroy(self.ui_control)
    }

    #[inline]
    pub fn handle(&self) -> usize {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiControlHandle(self.ui_control)
        }
    }

    #[inline]
    pub fn parent(&self) -> Option<Control> {
        ffi_utils::ensure_initialized();
        unsafe {
            let ui_control = ui_sys::uiControlParent(self.ui_control);
            if ui_control.is_null() {
                None
            } else {
                Some(Control::from_ui_control(ui_control))
            }
        }
    }

    #[inline]
    pub unsafe fn set_parent(&self, parent: Option<&Control>) {
        ffi_utils::ensure_initialized();
        ui_sys::uiControlSetParent(self.ui_control,
                                match parent {
                                    None => ptr::null_mut(),
                                    Some(parent) => parent.ui_control,
                                })
    }

    #[inline]
    pub fn toplevel(&self) -> bool {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiControlToplevel(self.ui_control) != 0
        }
    }

    #[inline]
    pub fn visible(&self) -> bool {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiControlVisible(self.ui_control) != 0
        }
    }

    #[inline]
    pub fn show(&self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiControlShow(self.ui_control)
        }
    }

    #[inline]
    pub fn hide(&self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiControlHide(self.ui_control)
        }
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiControlEnabled(self.ui_control) != 0
        }
    }

    #[inline]
    pub fn enable(&self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiControlEnable(self.ui_control)
        }
    }

    #[inline]
    pub fn disable(&self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiControlDisable(self.ui_control)
        }
    }
}

define_control!(Button, uiButton, ui_button);

impl Button {
    #[inline]
    pub fn text(&self) -> Text {
        ffi_utils::ensure_initialized();
        unsafe {
            Text::new(ui_sys::uiButtonText(self.ui_button))
        }
    }

    #[inline]
    pub fn set_text(&self, text: &str) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(text.as_bytes().to_vec()).unwrap();
            ui_sys::uiButtonSetText(self.ui_button, c_string.as_ptr())
        }
    }

    #[inline]
    pub fn on_clicked(&self, callback: Box<FnMut(&Button)>) {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut data: Box<Box<FnMut(&Button)>> = Box::new(callback);
            ui_sys::uiButtonOnClicked(self.ui_button,
                                      c_callback,
                                      &mut *data as *mut Box<FnMut(&Button)> as *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(button: *mut uiButton, data: *mut c_void) {
            unsafe {
                let button = Button {
                    ui_button: button,
                };
                mem::transmute::<*mut c_void, &mut Box<FnMut(&Button)>>(data)(&button)
            }
        }
    }

    #[inline]
    pub fn new(text: &str) -> Button {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(text.as_bytes().to_vec()).unwrap();
            Button::from_ui_control(ui_sys::uiNewButton(c_string.as_ptr()))
        }
    }
}

define_control!(BoxControl, uiBox, ui_box);

impl BoxControl {
    #[inline]
    pub fn append(&self, child: Control, stretchy: bool) {
        ffi_utils::ensure_initialized();
        unsafe {
            assert!(child.parent().is_none());
            ui_sys::uiBoxAppend(self.ui_box, child.ui_control, stretchy as c_int)
        }
    }

    /// FIXME(pcwalton): This will leak the deleted control! We have no way of actually getting it
    /// to decrement its reference count per `libui`'s UI as of today, unless we maintain a
    /// separate list of children ourselves…
    #[inline]
    pub fn delete(&self, index: u64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiBoxDelete(self.ui_box, index)
        }
    }

    #[inline]
    pub fn padded(&self) -> bool {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiBoxPadded(self.ui_box) != 0
        }
    }

    #[inline]
    pub fn set_padded(&self, padded: bool) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiBoxSetPadded(self.ui_box, padded as c_int)
        }
    }

    #[inline]
    pub fn new_horizontal() -> BoxControl {
        ffi_utils::ensure_initialized();
        unsafe {
            BoxControl::from_ui_control(ui_sys::uiNewHorizontalBox())
        }
    }

    #[inline]
    pub fn new_vertical() -> BoxControl {
        ffi_utils::ensure_initialized();
        unsafe {
            BoxControl::from_ui_control(ui_sys::uiNewVerticalBox())
        }
    }
}

define_control!(Entry, uiEntry, ui_entry);

impl Entry {
    #[inline]
    pub fn text(&self) -> Text {
        ffi_utils::ensure_initialized();
        unsafe {
            Text::new(ui_sys::uiEntryText(self.ui_entry))
        }
    }

    #[inline]
    pub fn set_text(&self, text: &str) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(text.as_bytes().to_vec()).unwrap();
            ui_sys::uiEntrySetText(self.ui_entry, c_string.as_ptr())
        }
    }

    #[inline]
    pub fn on_changed(&self, callback: Box<FnMut(&Entry)>) {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut data: Box<Box<FnMut(&Entry)>> = Box::new(callback);
            ui_sys::uiEntryOnChanged(self.ui_entry,
                                     c_callback,
                                     &mut *data as *mut Box<FnMut(&Entry)> as *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(entry: *mut uiEntry, data: *mut c_void) {
            unsafe {
                let entry = Entry::from_ui_control(entry);
                mem::transmute::<*mut c_void, &mut Box<FnMut(&Entry)>>(data)(&entry);
                mem::forget(entry);
            }
        }
    }

    #[inline]
    pub fn read_only(&self) -> bool {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiEntryReadOnly(self.ui_entry) != 0
        }
    }

    #[inline]
    pub fn set_read_only(&self, readonly: bool) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiEntrySetReadOnly(self.ui_entry, readonly as c_int)
        }
    }

    #[inline]
    pub fn new() -> Entry {
        ffi_utils::ensure_initialized();
        unsafe {
            Entry::from_ui_control(ui_sys::uiNewEntry())
        }
    }
}

define_control!(Checkbox, uiCheckbox, ui_checkbox);

impl Checkbox {
    #[inline]
    pub fn text(&self) -> Text {
        ffi_utils::ensure_initialized();
        unsafe {
            Text::new(ui_sys::uiCheckboxText(self.ui_checkbox))
        }
    }

    #[inline]
    pub fn set_text(&self, text: &str) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(text.as_bytes().to_vec()).unwrap();
            ui_sys::uiCheckboxSetText(self.ui_checkbox, c_string.as_ptr())
        }
    }

    #[inline]
    pub fn on_toggled(&self, callback: Box<FnMut(&Checkbox)>) {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut data: Box<Box<FnMut(&Checkbox)>> = Box::new(callback);
            ui_sys::uiCheckboxOnToggled(self.ui_checkbox,
                                        c_callback,
                                        &mut *data as *mut Box<FnMut(&Checkbox)> as *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(checkbox: *mut uiCheckbox, data: *mut c_void) {
            unsafe {
                let checkbox = Checkbox::from_ui_control(checkbox);
                mem::transmute::<*mut c_void, &mut Box<FnMut(&Checkbox)>>(data)(&checkbox);
                mem::forget(checkbox)
            }
        }
    }

    #[inline]
    pub fn checked(&self) -> bool {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiCheckboxChecked(self.ui_checkbox) != 0
        }
    }

    #[inline]
    pub fn set_checked(&self, checked: bool) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiCheckboxSetChecked(self.ui_checkbox, checked as c_int)
        }
    }

    #[inline]
    pub fn new(text: &str) -> Checkbox {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(text.as_bytes().to_vec()).unwrap();
            Checkbox::from_ui_control(ui_sys::uiNewCheckbox(c_string.as_ptr()))
        }
    }
}

define_control!(Label, uiLabel, ui_label);

impl Label {
    #[inline]
    pub fn text(&self) -> Text {
        ffi_utils::ensure_initialized();
        unsafe {
            Text::new(ui_sys::uiLabelText(self.ui_label))
        }
    }

    #[inline]
    pub fn set_text(&self, text: &str) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(text.as_bytes().to_vec()).unwrap();
            ui_sys::uiLabelSetText(self.ui_label, c_string.as_ptr())
        }
    }

    #[inline]
    pub fn new(text: &str) -> Label {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(text.as_bytes().to_vec()).unwrap();
            Label::from_ui_control(ui_sys::uiNewLabel(c_string.as_ptr()))
        }
    }
}

define_control!(Tab, uiTab, ui_tab);

impl Tab {
    #[inline]
    pub fn append(&self, name: &str, control: Control) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(name.as_bytes().to_vec()).unwrap();
            ui_sys::uiTabAppend(self.ui_tab, c_string.as_ptr(), control.ui_control)
        }
    }

    #[inline]
    pub fn insert_at(&self, name: &str, before: u64, control: Control) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(name.as_bytes().to_vec()).unwrap();
            ui_sys::uiTabInsertAt(self.ui_tab, c_string.as_ptr(), before, control.ui_control)
        }
    }

    /// FIXME(pcwalton): This will leak the deleted control! We have no way of actually getting it
    /// to decrement its reference count per `libui`'s UI as of today, unless we maintain a
    /// separate list of children ourselves…
    #[inline]
    pub fn delete(&self, index: u64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiTabDelete(self.ui_tab, index)
        }
    }

    #[inline]
    pub fn margined(&self, page: u64) -> bool {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiTabMargined(self.ui_tab, page) != 0
        }
    }

    #[inline]
    pub fn set_margined(&self, page: u64, margined: bool) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiTabSetMargined(self.ui_tab, page, margined as c_int)
        }
    }

    #[inline]
    pub fn new() -> Tab {
        ffi_utils::ensure_initialized();
        unsafe {
            Tab::from_ui_control(ui_sys::uiNewTab())
        }
    }
}

define_control!(Group, uiGroup, ui_group);

impl Group {
    #[inline]
    pub fn title(&self) -> Text {
        ffi_utils::ensure_initialized();
        unsafe {
            Text::new(ui_sys::uiGroupTitle(self.ui_group))
        }
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(title.as_bytes().to_vec()).unwrap();
            ui_sys::uiGroupSetTitle(self.ui_group, c_string.as_ptr())
        }
    }

    #[inline]
    pub fn set_child(&self, child: Control) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiGroupSetChild(self.ui_group, child.ui_control)
        }
    }

    #[inline]
    pub fn margined(&self) -> bool {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiGroupMargined(self.ui_group) != 0
        }
    }

    #[inline]
    pub fn set_margined(&self, margined: bool) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiGroupSetMargined(self.ui_group, margined as c_int)
        }
    }

    #[inline]
    pub fn new(title: &str) -> Group {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(title.as_bytes().to_vec()).unwrap();
            Group::from_ui_control(ui_sys::uiNewGroup(c_string.as_ptr()))
        }
    }
}

define_control!(Spinbox, uiSpinbox, ui_spinbox);

impl Spinbox {
    #[inline]
    pub fn value(&self) -> i64 {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiSpinboxValue(self.ui_spinbox)
        }
    }

    #[inline]
    pub fn set_value(&self, value: i64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiSpinboxSetValue(self.ui_spinbox, value)
        }
    }

    #[inline]
    pub fn on_changed(&self, callback: Box<FnMut(&Spinbox)>) {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut data: Box<Box<FnMut(&Spinbox)>> = Box::new(callback);
            ui_sys::uiSpinboxOnChanged(self.ui_spinbox,
                                       c_callback,
                                       &mut *data as *mut Box<FnMut(&Spinbox)> as *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(spinbox: *mut uiSpinbox, data: *mut c_void) {
            unsafe {
                let spinbox = Spinbox::from_ui_control(spinbox);
                mem::transmute::<*mut c_void, &mut Box<FnMut(&Spinbox)>>(data)(&spinbox);
                mem::forget(spinbox);
            }
        }
    }

    #[inline]
    pub fn new(min: i64, max: i64) -> Spinbox {
        ffi_utils::ensure_initialized();
        unsafe {
            Spinbox::from_ui_control(ui_sys::uiNewSpinbox(min, max))
        }
    }
}

define_control!(ProgressBar, uiProgressBar, ui_progress_bar);

impl ProgressBar {
    #[inline]
    pub fn set_value(&self, n: i32) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiProgressBarSetValue(self.ui_progress_bar, n)
        }
    }

    #[inline]
    pub fn new() -> ProgressBar {
        ffi_utils::ensure_initialized();
        unsafe {
            ProgressBar::from_ui_control(ui_sys::uiNewProgressBar())
        }
    }
}

define_control!(Slider, uiSlider, ui_slider);

impl Slider {
    #[inline]
    pub fn value(&self) -> i64 {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiSliderValue(self.ui_slider)
        }
    }

    #[inline]
    pub fn set_value(&self, value: i64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiSliderSetValue(self.ui_slider, value)
        }
    }

    #[inline]
    pub fn on_changed(&self, callback: Box<FnMut(&Slider)>) {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut data: Box<Box<FnMut(&Slider)>> = Box::new(callback);
            ui_sys::uiSliderOnChanged(self.ui_slider,
                                      c_callback,
                                      &mut *data as *mut Box<FnMut(&Slider)> as *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(slider: *mut uiSlider, data: *mut c_void) {
            unsafe {
                let slider = Slider::from_ui_control(slider);
                mem::transmute::<*mut c_void, &mut Box<FnMut(&Slider)>>(data)(&slider);
                mem::forget(slider);
            }
        }
    }

    #[inline]
    pub fn new(min: i64, max: i64) -> Slider {
        ffi_utils::ensure_initialized();
        unsafe {
            Slider::from_ui_control(ui_sys::uiNewSlider(min, max))
        }
    }
}

define_control!(Separator, uiSeparator, ui_separator);

impl Separator {
    #[inline]
    pub fn new_horizontal() -> Separator {
        ffi_utils::ensure_initialized();
        unsafe {
            Separator::from_ui_control(ui_sys::uiNewHorizontalSeparator())
        }
    }
}

define_control!(Combobox, uiCombobox, ui_combobox);

impl Combobox {
    #[inline]
    pub fn append(&self, name: &str) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(name.as_bytes().to_vec()).unwrap();
            ui_sys::uiComboboxAppend(self.ui_combobox, c_string.as_ptr())
        }
    }

    #[inline]
    pub fn selected(&self) -> i64 {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiComboboxSelected(self.ui_combobox)
        }
    }

    #[inline]
    pub fn set_selected(&self, n: i64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiComboboxSetSelected(self.ui_combobox, n)
        }
    }

    #[inline]
    pub fn on_selected(&self, callback: Box<FnMut(&Combobox)>) {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut data: Box<Box<FnMut(&Combobox)>> = Box::new(callback);
            ui_sys::uiComboboxOnSelected(self.ui_combobox,
                                         c_callback,
                                         &mut *data as *mut Box<FnMut(&Combobox)> as *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(combobox: *mut uiCombobox, data: *mut c_void) {
            unsafe {
                let combobox = Combobox::from_ui_control(combobox);
                mem::transmute::<*mut c_void, &mut Box<FnMut(&Combobox)>>(data)(&combobox);
                mem::forget(combobox);
            }
        }
    }

    #[inline]
    pub fn new() -> Combobox {
        ffi_utils::ensure_initialized();
        unsafe {
            Combobox::from_ui_control(ui_sys::uiNewCombobox())
        }
    }

    #[inline]
    pub fn new_editable() -> Combobox {
        ffi_utils::ensure_initialized();
        unsafe {
            Combobox::from_ui_control(ui_sys::uiNewEditableCombobox())
        }
    }
}

// FIXME(pcwalton): Are these supposed to be a subclass of something? They don't seem very usable
// with just the `uiRadioButtons*` methods…
define_control!(RadioButtons, uiRadioButtons, ui_radio_buttons);

impl RadioButtons {
    #[inline]
    pub fn append(&self, name: &str) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(name.as_bytes().to_vec()).unwrap();
            ui_sys::uiRadioButtonsAppend(self.ui_radio_buttons, c_string.as_ptr())
        }
    }

    #[inline]
    pub fn new() -> RadioButtons {
        ffi_utils::ensure_initialized();
        unsafe {
            RadioButtons::from_ui_control(ui_sys::uiNewRadioButtons())
        }
    }
}

// FIXME(pcwalton): Are these supposed to be a subclass of something? They don't seem very usable
// with just the `uiDatetimePicker*` methods…
define_control!(DateTimePicker, uiDateTimePicker, ui_date_time_picker);

impl DateTimePicker {
    pub fn new_date_time_picker() -> DateTimePicker {
        ffi_utils::ensure_initialized();
        unsafe {
            DateTimePicker::from_ui_control(ui_sys::uiNewDateTimePicker())
        }
    }

    pub fn new_date_picker() -> DateTimePicker {
        ffi_utils::ensure_initialized();
        unsafe {
            DateTimePicker::from_ui_control(ui_sys::uiNewDatePicker())
        }
    }

    pub fn new_time_picker() -> DateTimePicker {
        ffi_utils::ensure_initialized();
        unsafe {
            DateTimePicker::from_ui_control(ui_sys::uiNewTimePicker())
        }
    }
}

define_control!(MultilineEntry, uiMultilineEntry, ui_multiline_entry);

impl MultilineEntry {
    #[inline]
    pub fn text(&self) -> Text {
        ffi_utils::ensure_initialized();
        unsafe {
            Text::new(ui_sys::uiMultilineEntryText(self.ui_multiline_entry))
        }
    }

    #[inline]
    pub fn set_text(&self, text: &str) {
        ffi_utils::ensure_initialized();
        unsafe {
            let c_string = CString::new(text.as_bytes().to_vec()).unwrap();
            ui_sys::uiMultilineEntrySetText(self.ui_multiline_entry, c_string.as_ptr())
        }
    }

    #[inline]
    pub fn on_changed(&self, callback: Box<FnMut(&MultilineEntry)>) {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut data: Box<Box<FnMut(&MultilineEntry)>> = Box::new(callback);
            ui_sys::uiMultilineEntryOnChanged(self.ui_multiline_entry,
                                              c_callback,
                                              &mut *data as *mut Box<FnMut(&MultilineEntry)> as
                                              *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(multiline_entry: *mut uiMultilineEntry, data: *mut c_void) {
            unsafe {
                let multiline_entry = MultilineEntry::from_ui_control(multiline_entry);
                mem::transmute::<*mut c_void,
                                 &mut Box<FnMut(&MultilineEntry)>>(data)(&multiline_entry);
                mem::forget(multiline_entry);
            }
        }
    }

    #[inline]
    pub fn read_only(&self) -> bool {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiMultilineEntryReadOnly(self.ui_multiline_entry) != 0
        }
    }

    #[inline]
    pub fn set_read_only(&self, readonly: bool) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiMultilineEntrySetReadOnly(self.ui_multiline_entry, readonly as c_int)
        }
    }

    #[inline]
    pub fn new() -> MultilineEntry {
        ffi_utils::ensure_initialized();
        unsafe {
            MultilineEntry::from_ui_control(ui_sys::uiNewMultilineEntry())
        }
    }
}

pub trait AreaHandler {
    fn draw(&mut self, _area: &Area, _area_draw_params: &AreaDrawParams) {}
    fn mouse_event(&mut self, _area: &Area, _area_mouse_event: &AreaMouseEvent) {}
    fn mouse_crossed(&mut self, _area: &Area, _left: bool) {}
    fn drag_broken(&mut self, _area: &Area) {}
    fn key_event(&mut self, _area: &Area, _area_key_event: &AreaKeyEvent) -> bool {
        true
    }
}

#[repr(C)]
struct RustAreaHandler {
    ui_area_handler: uiAreaHandler,
    trait_object: Box<AreaHandler>,
}

impl RustAreaHandler {
    #[inline]
    fn new(trait_object: Box<AreaHandler>) -> Box<RustAreaHandler> {
        ffi_utils::ensure_initialized();
        return Box::new(RustAreaHandler {
            ui_area_handler: uiAreaHandler {
                Draw: draw,
                MouseEvent: mouse_event,
                MouseCrossed: mouse_crossed,
                DragBroken: drag_broken,
                KeyEvent: key_event,
            },
            trait_object: trait_object,
        });

        extern "C" fn draw(ui_area_handler: *mut uiAreaHandler,
                           ui_area: *mut uiArea,
                           ui_area_draw_params: *mut uiAreaDrawParams) {
            unsafe {
                let area = Area::from_ui_area(ui_area);
                let area_draw_params =
                    AreaDrawParams::from_ui_area_draw_params(&*ui_area_draw_params);
                (*(ui_area_handler as *mut RustAreaHandler)).trait_object.draw(&area,
                                                                               &area_draw_params);
                mem::forget(area_draw_params);
                mem::forget(area);
            }
        }

        extern "C" fn mouse_event(ui_area_handler: *mut uiAreaHandler,
                                  ui_area: *mut uiArea,
                                  ui_area_mouse_event: *mut uiAreaMouseEvent) {
            unsafe {
                let area = Area::from_ui_area(ui_area);
                let area_mouse_event =
                    AreaMouseEvent::from_ui_area_mouse_event(&*ui_area_mouse_event);
                (*(ui_area_handler as *mut RustAreaHandler)).trait_object
                                                            .mouse_event(&area, &area_mouse_event);
                mem::forget(area_mouse_event);
                mem::forget(area);
            }
        }

        extern "C" fn mouse_crossed(ui_area_handler: *mut uiAreaHandler,
                                    ui_area: *mut uiArea,
                                    left: c_int) {
            unsafe {
                let area = Area::from_ui_area(ui_area);
                (*(ui_area_handler as *mut RustAreaHandler)).trait_object.mouse_crossed(&area,
                                                                                        left != 0);
                mem::forget(area);
            }
        }

        extern "C" fn drag_broken(ui_area_handler: *mut uiAreaHandler, ui_area: *mut uiArea) {
            unsafe {
                let area = Area::from_ui_area(ui_area);
                (*(ui_area_handler as *mut RustAreaHandler)).trait_object.drag_broken(&area);
                mem::forget(area);
            }
        }

        extern "C" fn key_event(ui_area_handler: *mut uiAreaHandler,
                                ui_area: *mut uiArea,
                                ui_area_key_event: *mut uiAreaKeyEvent)
                                -> c_int {
            unsafe {
                let area = Area::from_ui_area(ui_area);
                let area_key_event = AreaKeyEvent::from_ui_area_key_event(&*ui_area_key_event);
                let result =
                    (*(ui_area_handler as *mut RustAreaHandler)).trait_object
                                                                .key_event(&area, &area_key_event);
                mem::forget(area_key_event);
                mem::forget(area);
                result as c_int
            }
        }
    }
}

define_control!(Area, uiArea, ui_area);

impl Area {
    #[inline]
    pub unsafe fn from_ui_area(ui_area: *mut uiArea) -> Area {
        Area {
            ui_area: ui_area,
        }
    }

    #[inline]
    pub fn set_size(&self, width: i64, height: i64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiAreaSetSize(self.ui_area, width, height)
        }
    }

    #[inline]
    pub fn queue_redraw_all(&self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiAreaQueueRedrawAll(self.ui_area)
        }
    }

    #[inline]
    pub fn scroll_to(&self, x: f64, y: f64, width: f64, height: f64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiAreaScrollTo(self.ui_area, x, y, width, height)
        }
    }

    #[inline]
    pub fn new(area_handler: Box<AreaHandler>) -> Area {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut rust_area_handler = RustAreaHandler::new(area_handler);
            let area =
                Area::from_ui_control(ui_sys::uiNewArea(&mut *rust_area_handler as
                                                        *mut RustAreaHandler as
                                                        *mut uiAreaHandler));
            mem::forget(rust_area_handler);
            area
        }
    }

    #[inline]
    pub fn new_scrolling(area_handler: Box<AreaHandler>, width: i64, height: i64) -> Area {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut rust_area_handler = RustAreaHandler::new(area_handler);
            let area =
                Area::from_ui_control(ui_sys::uiNewScrollingArea(&mut *rust_area_handler as
                                                                 *mut RustAreaHandler as
                                                                 *mut uiAreaHandler,
                                                                       width,
                                                                       height));
            mem::forget(rust_area_handler);
            area
        }
    }
}

pub struct AreaDrawParams {
    pub context: draw::Context,

    pub area_width: f64,
    pub area_height: f64,

    pub clip_x: f64,
    pub clip_y: f64,
    pub clip_width: f64,
    pub clip_height: f64,
}

impl AreaDrawParams {
    #[inline]
    unsafe fn from_ui_area_draw_params(ui_area_draw_params: &uiAreaDrawParams) -> AreaDrawParams {
        ffi_utils::ensure_initialized();
        AreaDrawParams {
            context: draw::Context::from_ui_draw_context(ui_area_draw_params.Context),
            area_width: ui_area_draw_params.AreaWidth,
            area_height: ui_area_draw_params.AreaHeight,
            clip_x: ui_area_draw_params.ClipX,
            clip_y: ui_area_draw_params.ClipY,
            clip_width: ui_area_draw_params.ClipWidth,
            clip_height: ui_area_draw_params.ClipHeight,
        }
    }
}

bitflags! {
    pub flags Modifiers: u8 {
        const MODIFIER_CTRL = 1 << 0,
        const MODIFIER_ALT = 1 << 1,
        const MODIFIER_SHIFT = 1 << 2,
        const MODIFIER_SUPER = 1 << 3,
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AreaMouseEvent {
    pub x: f64,
    pub y: f64,

    pub area_width: f64,
    pub area_height: f64,

    pub down: u64,
    pub up: u64,

    pub count: u64,

    pub modifiers: Modifiers,

    pub held_1_to_64: u64,
}

impl AreaMouseEvent {
    #[inline]
    pub fn from_ui_area_mouse_event(ui_area_mouse_event: &uiAreaMouseEvent) -> AreaMouseEvent {
        ffi_utils::ensure_initialized();
        AreaMouseEvent {
            x: ui_area_mouse_event.X,
            y: ui_area_mouse_event.Y,
            area_width: ui_area_mouse_event.AreaWidth,
            area_height: ui_area_mouse_event.AreaHeight,
            down: ui_area_mouse_event.Down,
            up: ui_area_mouse_event.Up,
            count: ui_area_mouse_event.Count,
            modifiers: Modifiers::from_bits(ui_area_mouse_event.Modifiers as u8).unwrap(),
            held_1_to_64: ui_area_mouse_event.Held1To64,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AreaKeyEvent {
    pub key: u8,
    pub ext_key: ExtKey,
    pub modifier: Modifiers,
    pub modifiers: Modifiers,
    pub up: bool,
}

impl AreaKeyEvent {
    #[inline]
    pub fn from_ui_area_key_event(ui_area_key_event: &uiAreaKeyEvent) -> AreaKeyEvent {
        ffi_utils::ensure_initialized();
        AreaKeyEvent {
            key: ui_area_key_event.Key as u8,
            ext_key: ui_area_key_event.ExtKey,
            modifier: Modifiers::from_bits(ui_area_key_event.Modifier as u8).unwrap(),
            modifiers: Modifiers::from_bits(ui_area_key_event.Modifiers as u8).unwrap(),
            up: ui_area_key_event.Up != 0,
        }
    }
}

define_control!(FontButton, uiFontButton, ui_font_button);

impl FontButton {
    /// Returns a new font.
    #[inline]
    pub fn font(&self) -> draw::text::Font {
        ffi_utils::ensure_initialized();
        unsafe {
            draw::text::Font::from_ui_draw_text_font(ui_sys::uiFontButtonFont(self.ui_font_button))
        }
    }

    #[inline]
    pub fn on_changed(&self, callback: Box<FnMut(&FontButton)>) {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut data: Box<Box<FnMut(&FontButton)>> = Box::new(callback);
            ui_sys::uiFontButtonOnChanged(self.ui_font_button,
                                          c_callback,
                                          &mut *data as *mut Box<FnMut(&FontButton)> as
                                          *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(ui_font_button: *mut uiFontButton, data: *mut c_void) {
            unsafe {
                let font_button = FontButton::from_ui_control(ui_font_button);
                mem::transmute::<*mut c_void, &mut Box<FnMut(&FontButton)>>(data)(&font_button);
                mem::forget(font_button);
            }
        }
    }

    #[inline]
    pub fn new() -> FontButton {
        ffi_utils::ensure_initialized();
        unsafe {
            FontButton::from_ui_control(ui_sys::uiNewFontButton())
        }
    }
}

define_control!(ColorButton, uiColorButton, ui_color_button);

impl ColorButton {
    #[inline]
    pub fn color(&self) -> Color {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut color: Color = mem::uninitialized();
            ui_sys::uiColorButtonColor(self.ui_color_button,
                                       &mut color.r,
                                       &mut color.g,
                                       &mut color.b,
                                       &mut color.a);
            color
        }
    }

    #[inline]
    pub fn set_color(&self, color: &Color) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiColorButtonSetColor(self.ui_color_button, color.r, color.g, color.b, color.a)
        }
    }

    #[inline]
    pub fn on_changed(&self, callback: Box<FnMut(&ColorButton)>) {
        ffi_utils::ensure_initialized();
        unsafe {
            let mut data: Box<Box<FnMut(&ColorButton)>> = Box::new(callback);
            ui_sys::uiColorButtonOnChanged(self.ui_color_button,
                                           c_callback,
                                           &mut *data as *mut Box<FnMut(&ColorButton)> as
                                           *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(ui_color_button: *mut uiColorButton, data: *mut c_void) {
            unsafe {
                let color_button = ColorButton::from_ui_control(ui_color_button);
                mem::transmute::<*mut c_void, &mut Box<FnMut(&ColorButton)>>(data)(&color_button);
                mem::forget(color_button)
            }
        }
    }

    #[inline]
    pub fn new() -> ColorButton {
        ffi_utils::ensure_initialized();
        unsafe {
            ColorButton::from_ui_control(ui_sys::uiNewColorButton())
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Color {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

