//! Media driver startup example based on
//! [aeronmd.c](https://github.com/real-logic/aeron/blob/master/aeron-driver/src/main/c/aeronmd.c)
//! This example demonstrates direct usage of the -sys bindings for the Media Driver API.

use aeron_driver_sys::*;
use clap;
use ctrlc;
use std::ffi::CStr;
use std::os::raw::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);

unsafe extern "C" fn termination_hook(_clientd: *mut c_void) {
    println!("Terminated");
    RUNNING.store(false, Ordering::SeqCst);
}

fn main() {
    let version = unsafe { CStr::from_ptr(aeron_version_full()) };
    let _cmdline = clap::App::new("aeronmd")
        .version(version.to_str().unwrap())
        .get_matches();

    // TODO: Handle -D switches

    ctrlc::set_handler(move || {
        // TODO: Actually understand atomic ordering
        RUNNING.store(false, Ordering::SeqCst);
    })
    .unwrap();

    let mut init_success = true;
    let mut context: *mut aeron_driver_context_t = ptr::null_mut();
    let mut driver: *mut aeron_driver_t = ptr::null_mut();

    if init_success {
        let context_init = unsafe { aeron_driver_context_init(&mut context) };
        if context_init < 0 {
            let err_code = unsafe { aeron_errcode() };
            let err_str = unsafe { CStr::from_ptr(aeron_errmsg()) }.to_str().unwrap();
            eprintln!("ERROR: context init ({}) {}", err_code, err_str);
            init_success = false;
        }
    }

    if init_success {
        let term_hook = unsafe {
            aeron_driver_context_set_driver_termination_hook(
                context,
                Some(termination_hook),
                ptr::null_mut(),
            )
        };
        if term_hook < 0 {
            let err_code = unsafe { aeron_errcode() };
            let err_str = unsafe { CStr::from_ptr(aeron_errmsg()) }.to_str().unwrap();
            eprintln!(
                "ERROR: context set termination hook ({}) {}",
                err_code, err_str
            );
            init_success = false;
        }
    }

    if init_success {
        let driver_init = unsafe { aeron_driver_init(&mut driver, context) };
        if driver_init < 0 {
            let err_code = unsafe { aeron_errcode() };
            let err_str = unsafe { CStr::from_ptr(aeron_errmsg()) }.to_str().unwrap();
            eprintln!("ERROR: driver init ({}) {}", err_code, err_str);
            init_success = false;
        }
    }

    if init_success {
        let driver_start = unsafe { aeron_driver_start(driver, true) };
        if driver_start < 0 {
            let err_code = unsafe { aeron_errcode() };
            let err_str = unsafe { CStr::from_ptr(aeron_errmsg()) }.to_str().unwrap();
            eprintln!("ERROR: driver start ({}) {}", err_code, err_str);
            init_success = false;
        }
    }

    if init_success {
        println!("Press Ctrl-C to exit.");

        while RUNNING.load(Ordering::SeqCst) {
            unsafe { aeron_driver_main_idle_strategy(driver, aeron_driver_main_do_work(driver)) };
        }
    }

    unsafe { aeron_driver_close(driver) };
    unsafe { aeron_driver_context_close(context) };
}
