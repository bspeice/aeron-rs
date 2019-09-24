//! [Aeron](https://github.com/real-logic/aeron) client for Rust
#![deny(missing_docs)]

pub mod driver;

/// Retrieve the C library version in (major, minor, patch) format
pub fn aeron_version() -> (u32, u32, u32) {
    unsafe {
        (
            aeron_driver_sys::aeron_version_major() as u32,
            aeron_driver_sys::aeron_version_minor() as u32,
            aeron_driver_sys::aeron_version_patch() as u32,
        )
    }
}
