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

use crate::util::bit;
use std::mem::size_of;

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
    /// Size of the buffer containing data going to the media driver
    pub to_driver_buffer_length: i32,
    _to_client_buffer_length: i32,
    _counter_metadata_buffer_length: i32,
    _counter_values_buffer_length: i32,
    _error_log_buffer_length: i32,
    _client_liveness_timeout: i64,
    _start_timestamp: i64,
    _pid: i64,
}

/// Length of the metadata block in a CnC file. Note that it's not equivalent
/// to the actual struct length.
pub const META_DATA_LENGTH: usize =
    bit::align(size_of::<MetaDataDefinition>(), bit::CACHE_LINE_LENGTH * 2);

/// Version code for the Aeron CnC file format
pub const CNC_VERSION: i32 = crate::sematic_version_compose(0, 0, 16);

/// Filename for the CnC file located in the Aeron directory
pub const CNC_FILE: &str = "cnc.dat";

#[cfg(test)]
mod tests {
    use crate::cnc_descriptor::{MetaDataDefinition, CNC_FILE, CNC_VERSION};
    use crate::driver::DriverContext;
    use memmap::MmapOptions;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn read_cnc_version() {
        let temp_dir = tempdir().unwrap();
        let dir = temp_dir.path().to_path_buf();
        temp_dir.close().unwrap();

        let _driver = DriverContext::default()
            .set_aeron_dir(&dir)
            .build()
            .unwrap();

        // Open the CnC location
        let cnc_path = dir.join(CNC_FILE);
        let cnc_file = File::open(&cnc_path).expect("Unable to open CnC file");
        let mmap = unsafe {
            MmapOptions::default()
                .map(&cnc_file)
                .expect("Unable to memory map CnC file")
        };

        let metadata: &MetaDataDefinition =
            unsafe { &*(mmap.as_ptr() as *const MetaDataDefinition) };
        assert_eq!(metadata.cnc_version, CNC_VERSION);
    }
}
