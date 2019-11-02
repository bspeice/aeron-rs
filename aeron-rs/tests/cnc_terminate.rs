use aeron_driver_sys::*;
use aeron_rs::client::cnc_descriptor;
use aeron_rs::client::concurrent::AtomicBuffer;
use aeron_rs::client::concurrent::ringbuffer::ManyToOneRingBuffer;
use aeron_rs::util::IndexT;
use memmap::MmapOptions;
use std::ffi::{c_void, CString};
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{ptr, thread};
use tempfile::tempdir;

static RUNNING: AtomicBool = AtomicBool::new(false);

unsafe extern "C" fn termination_hook(_state: *mut c_void) {
    RUNNING.store(false, Ordering::SeqCst);
}

unsafe extern "C" fn termination_validation(
    _state: *mut c_void,
    _data: *mut u8,
    _length: i32,
) -> bool {
    true
}

fn driver_thread(aeron_dir: PathBuf) {
    // Code largely ripped from `aeronmd`. Extra bits for termination and
    // termination validation added as necessary, and far coarser error handling.
    let mut context: *mut aeron_driver_context_t = ptr::null_mut();
    let mut driver: *mut aeron_driver_t = ptr::null_mut();

    let context_init = unsafe { aeron_driver_context_init(&mut context) };
    assert!(context_init >= 0);

    let path_bytes = aeron_dir.to_str().unwrap().as_bytes();
    let c_string = CString::new(path_bytes).unwrap();

    let aeron_dir = unsafe { aeron_driver_context_set_dir(context, c_string.into_raw()) };
    assert!(aeron_dir >= 0);

    let term_hook = unsafe {
        aeron_driver_context_set_driver_termination_hook(
            context,
            Some(termination_hook),
            ptr::null_mut(),
        )
    };
    assert!(term_hook >= 0);

    let term_validation_hook = unsafe {
        aeron_driver_context_set_driver_termination_validator(
            context,
            Some(termination_validation),
            ptr::null_mut(),
        )
    };
    assert!(term_validation_hook >= 0);

    let delete_dir = unsafe { aeron_driver_context_set_dir_delete_on_start(context, true) };
    assert!(delete_dir >= 0);

    let driver_init = unsafe { aeron_driver_init(&mut driver, context) };
    assert!(driver_init >= 0);

    let driver_start = unsafe { aeron_driver_start(driver, true) };
    assert!(driver_start >= 0);

    RUNNING.store(true, Ordering::SeqCst);
    while RUNNING.load(Ordering::SeqCst) {
        unsafe { aeron_driver_main_idle_strategy(driver, aeron_driver_main_do_work(driver)) };
    }

    unsafe { aeron_driver_close(driver) };
    unsafe { aeron_driver_context_close(context) };
}

/*
#[test]
fn cnc_terminate() {
    let temp_dir = tempdir().unwrap();
    let dir = temp_dir.path().to_path_buf();
    temp_dir.close().unwrap();

    // Start up the media driver
    let driver_dir = dir.clone();
    let driver_thread = thread::Builder::new()
        .name("cnc_terminate__driver_thread".to_string())
        .spawn(|| driver_thread(driver_dir))
        .unwrap();

    // Sleep a moment to let the media driver get set up
    thread::sleep(Duration::from_millis(500));
    assert_eq!(RUNNING.load(Ordering::SeqCst), true);

    // Write to the CnC file to attempt termination
    let cnc = dir.join(cnc_descriptor::CNC_FILE);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&cnc)
        .expect("Unable to open CnC file");
    let mut mmap =
        unsafe { MmapOptions::default().map_mut(&file) }.expect("Unable to mmap CnC file");

    // When creating the buffer, we need to offset by the CnC metadata
    let cnc_metadata_len = cnc_descriptor::META_DATA_LENGTH;

    // Read metadata to get buffer length
    let buffer_len = {
        let atomic_buffer = AtomicBuffer::wrap(&mut mmap);
        let metadata = atomic_buffer
            .overlay::<cnc_descriptor::MetaDataDefinition>(0)
            .unwrap();
        metadata.to_driver_buffer_length
    };

    let buffer_end = cnc_metadata_len + buffer_len as usize;
    let atomic_buffer = AtomicBuffer::wrap(&mut mmap[cnc_metadata_len..buffer_end]);
    let mut ring_buffer =
        ManyToOneRingBuffer::wrap(atomic_buffer).expect("Improperly sized buffer");

    // 20 bytes: Client ID (8), correlation ID (8), token length (4)
    let mut terminate_bytes = vec![0u8; 20];
    let terminate_len = terminate_bytes.len();
    let mut source_buffer = AtomicBuffer::wrap(&mut terminate_bytes);
    let client_id = ring_buffer.next_correlation_id();
    source_buffer.put_i64_ordered(0, client_id).unwrap();
    source_buffer.put_i64_ordered(8, -1).unwrap();

    let term_id: i32 = 0x0E;
    ring_buffer
        .write(term_id, &source_buffer, 0, terminate_len as IndexT)
        .unwrap();

    // Wait for the driver to finish
    // TODO: Timeout, and then set `RUNNING` manually
    driver_thread
        .join()
        .expect("Driver thread panicked during execution");
    assert_eq!(RUNNING.load(Ordering::SeqCst), false);
}
*/
