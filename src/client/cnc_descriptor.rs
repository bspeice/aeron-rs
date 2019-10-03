//! Description of the command and control file used to communicate between the Media Driver
//! and its clients.
//!
//! File layout:
//!
//! ```text
//! +-----------------------------+
//! |          Meta Data          |
//! +-----------------------------+
//! |      to-driver Buffer       |
//! +-----------------------------+
//! |      to-clients Buffer      |
//! +-----------------------------+
//! |   Counters Metadata Buffer  |
//! +-----------------------------+
//! |    Counters Values Buffer   |
//! +-----------------------------+
//! |          Error Log          |
//! +-----------------------------+
//! ```

/// The CnC file metadata header. Layout:
///
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                      Aeron CnC Version                        |
/// +---------------------------------------------------------------+
/// |                   to-driver buffer length                     |
/// +---------------------------------------------------------------+
/// |                  to-clients buffer length                     |
/// +---------------------------------------------------------------+
/// |               Counters Metadata buffer length                 |
/// +---------------------------------------------------------------+
/// |                Counters Values buffer length                  |
/// +---------------------------------------------------------------+
/// |                   Error Log buffer length                     |
/// +---------------------------------------------------------------+
/// |                   Client Liveness Timeout                     |
/// |                                                               |
/// +---------------------------------------------------------------+
/// |                    Driver Start Timestamp                     |
/// |                                                               |
/// +---------------------------------------------------------------+
/// |                         Driver PID                            |
/// |                                                               |
/// +---------------------------------------------------------------+
/// ```
#[repr(C, align(4))]
pub struct MetaDataDefinition {
    cnc_version: i32,
    _to_driver_buffer_length: i32,
    _to_client_buffer_length: i32,
    _counter_metadata_buffer_length: i32,
    _counter_values_buffer_length: i32,
    _error_log_buffer_length: i32,
    _client_liveness_timeout: i64,
    _start_timestamp: i64,
    _pid: i64,
}

/// Version code for the Aeron CnC file format
pub const CNC_VERSION: i32 = crate::sematic_version_compose(0, 0, 16);

/// Filename for the CnC file located in the Aeron directory
pub const CNC_FILE: &str = "cnc.dat";

#[cfg(test)]
mod tests {
    use crate::client::cnc_descriptor::{MetaDataDefinition, CNC_FILE, CNC_VERSION};
    use crate::driver::{DriverContext, MediaDriver};
    use memmap::MmapOptions;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn read_cnc_version() {
        let dir = tempdir().unwrap();
        let dir_path = dir.as_ref().to_path_buf();
        dir.close().unwrap();

        let context = DriverContext::default().set_aeron_dir(&dir_path);
        let _driver = MediaDriver::with_context(context).unwrap();

        // Open the CnC location
        let cnc_path = dir_path.join(CNC_FILE);
        let cnc_file = File::open(&cnc_path).expect("Unable to open CnC file");
        let mmap = unsafe {
            MmapOptions::default()
                .map(&cnc_file)
                .expect("Unable to memory map CnC file")
        };

        let metadata: &MetaDataDefinition = unsafe { &*(mmap.as_ptr().cast()) };
        assert_eq!(metadata.cnc_version, CNC_VERSION);
    }
}
