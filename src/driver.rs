//! Bindings for the C Media Driver

use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;

use aeron_driver_sys::*;

/// Error code and message returned by the Media Driver
#[derive(Debug, PartialEq)]
pub struct DriverError {
    code: i32,
    msg: String,
}

/// Context used to set up the Media Driver
#[derive(Default)]
pub struct DriverContext {
    aeron_dir: Option<CString>,
}

impl DriverContext {
    /// Set the Aeron directory path that will be used for storing the files
    /// Aeron uses to communicate with clients.
    pub fn set_aeron_dir(mut self, path: &Path) -> Self {
        // UNWRAP: Fails only if the path is non-UTF8
        let path_bytes = path.to_str().unwrap().as_bytes();
        // UNWRAP: Fails only if there is a null byte in the provided path
        let c_string = CString::new(path_bytes).unwrap();
        self.aeron_dir = Some(c_string);
        self
    }
}

macro_rules! aeron_op {
    ($op:expr) => {
        if $op < 0 {
            let code = ::aeron_driver_sys::aeron_errcode();
            let msg = CStr::from_ptr(::aeron_driver_sys::aeron_errmsg())
                .to_str()
                .unwrap()
                .to_string();
            Err(DriverError { code, msg })
        } else {
            Ok(())
        }
    };
}

/// Holder object to interface with the Media Driver
#[derive(Debug)]
pub struct MediaDriver {
    c_context: *mut aeron_driver_context_t,
    c_driver: *mut aeron_driver_t,
}

impl MediaDriver {
    /// Set up a new Media Driver
    pub fn with_context(mut context: DriverContext) -> Result<Self, DriverError> {
        let mut driver = MediaDriver {
            c_context: ptr::null_mut(),
            c_driver: ptr::null_mut(),
        };

        unsafe { aeron_op!(aeron_driver_context_init(&mut driver.c_context)) }?;

        context.aeron_dir.take().map(|dir| unsafe {
            aeron_op!(aeron_driver_context_set_dir(
                driver.c_context,
                dir.into_raw()
            ))
        });

        unsafe { aeron_op!(aeron_driver_init(&mut driver.c_driver, driver.c_context)) }?;

        Ok(driver)
    }

    /// Set up a new Media Driver with default options
    pub fn new() -> Result<Self, DriverError> {
        Self::with_context(DriverContext::default())
    }

    /// Retrieve the C library version in (major, minor, patch) format
    pub fn driver_version() -> (u32, u32, u32) {
        unsafe {
            (
                aeron_version_major() as u32,
                aeron_version_minor() as u32,
                aeron_version_patch() as u32,
            )
        }
    }
}

impl Drop for MediaDriver {
    fn drop(&mut self) {
        if self.c_driver.is_null() {
            unsafe { aeron_op!(aeron_driver_close(self.c_driver)) }.unwrap();
        }
        if self.c_context.is_null() {
            unsafe { aeron_op!(aeron_driver_context_close(self.c_context)) }.unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::driver::{DriverContext, DriverError, MediaDriver};
    use std::ffi::CStr;
    use tempfile::tempdir;

    #[test]
    fn multiple_startup_failure() {
        // We immediately close `tempdir` because we just want the name; Aeron needs
        // to set up the directory itself.
        let dir = tempdir().unwrap();
        let dir_path = dir.as_ref().to_path_buf();
        dir.close().unwrap();

        let context = DriverContext::default().set_aeron_dir(&dir_path);
        let driver = MediaDriver::with_context(context).unwrap();

        assert_eq!(
            unsafe { CStr::from_ptr((*driver.c_context).aeron_dir) }.to_str(),
            Ok(dir_path.to_str().unwrap())
        );
        drop(driver);

        // Attempting to start a media driver twice in rapid succession is guaranteed
        // cause an issue because the new media driver must wait for a heartbeat timeout.
        let context = DriverContext::default().set_aeron_dir(&dir_path);
        let driver_res = MediaDriver::with_context(context);

        // TODO: Why is the error message behavior different on Windows?
        let expected_message = if cfg!(target_os = "windows") {
            String::new()
        } else {
            format!("could not recreate aeron dir {}: ", dir_path.display())
        };

        assert!(driver_res.is_err());
        assert_eq!(
            driver_res.unwrap_err(),
            DriverError {
                code: 0,
                msg: expected_message
            }
        );
    }
}
