//! Functions and types related to menus.

use libc::{c_int, c_void};
use std::ffi::CString;
use std::mem;
use ui_sys::{self, uiMenu, uiMenuItem, uiWindow};
use windows::Window;

// NB: If there ever becomes a way to destroy menus and/or menu items, we'll need to reference
// count these for memory safety.
#[derive(Clone)]
pub struct MenuItem {
    ui_menu_item: *mut uiMenuItem,
}

impl MenuItem {
    #[inline]
    pub fn enable(&self) {
        unsafe {
            ui_sys::uiMenuItemEnable(self.ui_menu_item)
        }
    }

    #[inline]
    pub fn disable(&self) {
        unsafe {
            ui_sys::uiMenuItemDisable(self.ui_menu_item)
        }
    }

    #[inline]
    pub fn on_clicked(&self, callback: Box<FnMut(&MenuItem, &Window)>) {
        unsafe {
            let mut data: Box<Box<FnMut(&MenuItem, &Window)>> = Box::new(callback);
            ui_sys::uiMenuItemOnClicked(self.ui_menu_item,
                                        c_callback,
                                        &mut *data as *mut Box<FnMut(&MenuItem,
                                                                     &Window)> as *mut c_void);
            mem::forget(data);
        }

        extern "C" fn c_callback(menu_item: *mut uiMenuItem,
                                 window: *mut uiWindow,
                                 data: *mut c_void) {
            unsafe {
                let menu_item = MenuItem {
                    ui_menu_item: menu_item,
                };
                let window = Window::from_ui_window(window);
                mem::transmute::<*mut c_void,
                                 &mut Box<FnMut(&MenuItem, &Window)>>(data)(&menu_item, &window);
                mem::forget(window);
            }
        }
    }

    #[inline]
    pub fn checked(&self) -> bool {
        unsafe {
            ui_sys::uiMenuItemChecked(self.ui_menu_item) != 0
        }
    }

    #[inline]
    pub fn set_checked(&self, checked: bool) {
        unsafe {
            ui_sys::uiMenuItemSetChecked(self.ui_menu_item, checked as c_int)
        }
    }
}

// NB: If there ever becomes a way to destroy menus, we'll need to reference count these for memory
// safety.
#[derive(Clone)]
pub struct Menu {
    ui_menu: *mut uiMenu,
}

impl Menu {
    #[inline]
    pub fn append_item(&self, name: &str) -> MenuItem {
        unsafe {
            let c_string = CString::new(name.as_bytes().to_vec()).unwrap();
            MenuItem {
                ui_menu_item: ui_sys::uiMenuAppendItem(self.ui_menu, c_string.as_ptr()),
            }
        }
    }

    #[inline]
    pub fn append_check_item(&self, name: &str) -> MenuItem {
        unsafe {
            let c_string = CString::new(name.as_bytes().to_vec()).unwrap();
            MenuItem {
                ui_menu_item: ui_sys::uiMenuAppendCheckItem(self.ui_menu, c_string.as_ptr()),
            }
        }
    }

    #[inline]
    pub fn append_quit_item(&self) -> MenuItem {
        unsafe {
            MenuItem {
                ui_menu_item: ui_sys::uiMenuAppendQuitItem(self.ui_menu),
            }
        }
    }

    #[inline]
    pub fn append_preferences_item(&self) -> MenuItem {
        unsafe {
            MenuItem {
                ui_menu_item: ui_sys::uiMenuAppendPreferencesItem(self.ui_menu),
            }
        }
    }

    #[inline]
    pub fn append_about_item(&self) -> MenuItem {
        unsafe {
            MenuItem {
                ui_menu_item: ui_sys::uiMenuAppendAboutItem(self.ui_menu),
            }
        }
    }

    #[inline]
    pub fn append_separator(&self) {
        unsafe {
            ui_sys::uiMenuAppendSeparator(self.ui_menu)
        }
    }

    #[inline]
    pub fn new(name: &str) -> Menu {
        unsafe {
            let c_string = CString::new(name.as_bytes().to_vec()).unwrap();
            Menu {
                ui_menu: ui_sys::uiNewMenu(c_string.as_ptr()),
            }
        }
    }
}

