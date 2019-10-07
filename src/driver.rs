//! Bindings for the C Media Driver

use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;

use aeron_driver_sys::*;
use std::marker::PhantomData;
use std::mem::replace;

/// Error code and message returned by the Media Driver
#[derive(Debug, PartialEq)]
pub struct DriverError {
    code: i32,
    msg: String,
}

type Result<S> = std::result::Result<S, DriverError>;

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

/// Context used to set up the Media Driver
#[derive(Default)]
pub struct DriverContext {
    aeron_dir: Option<CString>,
    dir_delete_on_start: Option<bool>,
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

    /// Set whether Aeron should attempt to delete the `aeron_dir` on startup
    /// if it already exists. Aeron will attempt to remove the directory if true.
    /// If `aeron_dir` is not set in the `DriverContext`, Aeron will still attempt
    /// to remove the default Aeron directory.
    pub fn set_dir_delete_on_start(mut self, delete: bool) -> Self {
        self.dir_delete_on_start = Some(delete);
        self
    }

    /// Construct a Media Driver given the context options
    pub fn build(mut self) -> Result<MediaDriver<DriverInitialized>> {
        let mut driver = MediaDriver {
            c_context: ptr::null_mut(),
            c_driver: ptr::null_mut(),
            _state: PhantomData,
        };

        unsafe { aeron_op!(aeron_driver_context_init(&mut driver.c_context)) }?;

        self.aeron_dir.take().map(|dir| unsafe {
            aeron_op!(aeron_driver_context_set_dir(
                driver.c_context,
                dir.into_raw()
            ))
        });

        self.dir_delete_on_start.take().map(|delete| unsafe {
            aeron_op!(aeron_driver_context_set_dir_delete_on_start(
                driver.c_context,
                delete
            ))
        });

        unsafe { aeron_op!(aeron_driver_init(&mut driver.c_driver, driver.c_context)) }?;

        Ok(driver)
    }
}

/// Holder object to interface with the Media Driver
#[derive(Debug)]
pub struct MediaDriver<S> {
    c_context: *mut aeron_driver_context_t,
    c_driver: *mut aeron_driver_t,
    _state: PhantomData<S>,
}

/// Marker type for a MediaDriver that has yet to be started
#[derive(Debug)]
pub struct DriverInitialized;

/// Marker type for a MediaDriver that has been started
#[derive(Debug)]
pub struct DriverStarted;

impl<S> MediaDriver<S> {
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

impl MediaDriver<DriverInitialized> {
    /// Set up a new Media Driver with default options
    pub fn new() -> Result<Self> {
        DriverContext::default().build()
    }

    /// Start the Media Driver threads; does not take control of the current thread
    pub fn start(mut self) -> Result<MediaDriver<DriverStarted>> {
        unsafe { aeron_op!(aeron_driver_start(self.c_driver, true)) }?;

        // Move the driver and context references so the drop of `self` can't trigger issues
        // when the new media driver is also eventually dropped
        let c_driver = replace(&mut self.c_driver, ptr::null_mut());
        let c_context = replace(&mut self.c_context, ptr::null_mut());

        Ok(MediaDriver {
            c_driver,
            c_context,
            _state: PhantomData,
        })
    }
}

impl MediaDriver<DriverStarted> {
    /// Perform a single idle cycle of the Media Driver; does not take control of
    /// the current thread
    pub fn do_work(&self) {
        unsafe {
            aeron_driver_main_idle_strategy(self.c_driver, aeron_driver_main_do_work(self.c_driver))
        };
    }
}

impl<S> Drop for MediaDriver<S> {
    fn drop(&mut self) {
        if !self.c_driver.is_null() {
            unsafe { aeron_op!(aeron_driver_close(self.c_driver)) }.unwrap();
        }
        if !self.c_context.is_null() {
            unsafe { aeron_op!(aeron_driver_context_close(self.c_context)) }.unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::driver::{DriverContext, DriverError};
    use std::ffi::CStr;
    use tempfile::tempdir;

    #[test]
    fn multiple_startup_failure() {
        let temp_dir = tempdir().unwrap();
        let dir = temp_dir.path().to_path_buf();
        temp_dir.close();

        let driver = DriverContext::default()
            .set_aeron_dir(&dir)
            .build()
            .unwrap();

        assert_eq!(
            unsafe { CStr::from_ptr((*driver.c_context).aeron_dir) }.to_str(),
            Ok(dir.to_str().unwrap())
        );
        drop(driver);

        // Attempting to start a media driver twice in rapid succession is guaranteed
        // cause an issue because the new media driver must wait for a heartbeat timeout.
        let driver_res = DriverContext::default().set_aeron_dir(&dir).build();

        // TODO: Why is the error message behavior different on Windows?
        let expected_message = if cfg!(target_os = "windows") {
            String::new()
        } else {
            format!("could not recreate aeron dir {}: ", dir.display())
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

    #[test]
    fn single_duty_cycle() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        temp_dir.close();

        let driver = DriverContext::default()
            .set_aeron_dir(&path)
            .build()
            .expect("Unable to create media driver")
            .start()
            .expect("Unable to start driver");
        driver.do_work();
    }
}
