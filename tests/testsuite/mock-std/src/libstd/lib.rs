pub use std::*;

pub fn custom_api() {
    registry_dep_using_core::custom_api();
    registry_dep_using_alloc::custom_api();
}
