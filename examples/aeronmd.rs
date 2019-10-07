//! A version of the `aeronmd` runner program demonstrating the Rust wrappers
//! around Media Driver functionality.
use aeron_rs::driver::DriverContext;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(false);

fn main() {
    let driver = DriverContext::default()
        .build()
        .expect("Unable to create media driver");

    let driver = driver.start().expect("Unable to start media driver");
    RUNNING.store(true, Ordering::SeqCst);

    println!("Press Ctrl-C to quit");

    while RUNNING.load(Ordering::SeqCst) {
        // TODO: Termination hook
        driver.do_work();
    }
}
